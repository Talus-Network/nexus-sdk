//! TAP unit testing with the Nexus bytecode published on Sui.

use {
    super::{
        tap_test_overlay::add_test_functions,
        tap_test_vm::{SuiVMTestSetup, MAX_UNIT_TEST_INSTRUCTIONS},
        *,
    },
    anyhow::Context as _,
    move_binary_format::CompiledModule,
    move_compiler::{
        compiled_unit::{AnnotatedCompiledUnit, NamedCompiledModule},
        diagnostics,
        parser::ast::{Definition, ModuleDefinition, ModuleMember, Program},
        shared::Identifier as _,
        unit_test::{plan_builder::construct_test_plan, TestPlan},
        PASS_CFGIR,
        PASS_PARSER,
    },
    move_package_alt::RootPackage,
    move_package_alt_compilation::{
        build_config::BuildConfig,
        build_plan::BuildPlan,
        compiled_package::BuildNamedAddresses,
    },
    move_unit_test::UnitTestingConfig,
    nexus_sdk::sui::traits::FieldMaskUtil as _,
    std::{
        collections::{BTreeMap, BTreeSet},
        fmt::Debug,
        path::Path,
    },
    sui_package_alt::{mainnet_environment, testnet_environment, SuiFlavor},
};

const NEXUS_PACKAGE_NAMES: [&str; 7] = [
    "nexus_interface",
    "nexus_kernel",
    "nexus_primitives",
    "nexus_registry",
    "nexus_scheduler",
    "nexus_tool",
    "nexus_workflow",
];

/// Network whose published Nexus modules execute in the local test VM.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum TapTestEnvironment {
    /// Use the Nexus packages published on Sui Testnet.
    #[default]
    Testnet,
    /// Use the Nexus packages published on Sui Mainnet.
    Mainnet,
}

impl std::fmt::Display for TapTestEnvironment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Testnet => formatter.write_str("testnet"),
            Self::Mainnet => formatter.write_str("mainnet"),
        }
    }
}

impl TapTestEnvironment {
    fn move_environment(self) -> move_package_alt::schema::Environment {
        match self {
            Self::Testnet => testnet_environment(),
            Self::Mainnet => mainnet_environment(),
        }
    }

    fn rpc_url(self) -> &'static str {
        match self {
            Self::Testnet => TESTNET_NEXUS_RPC_URL,
            Self::Mainnet => MAINNET_NEXUS_RPC_URL,
        }
    }
}

/// Runs the tests in `path` against the selected published Nexus packages.
pub(crate) async fn test_tap_package(
    path: PathBuf,
    build_env: TapTestEnvironment,
    filter: Option<String>,
    list: bool,
    threads: usize,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Testing TAP package '{}' with Nexus {build_env} bytecode",
        path.display()
    );

    if threads == 0 {
        return Err(NexusCliError::Any(anyhow!(
            "unit test thread count must be greater than zero"
        )));
    }

    let package_path = resolve_package_path(&path).map_err(NexusCliError::Any)?;
    let passed = run_tap_tests(&package_path, build_env, filter, list, threads)
        .await
        .map_err(NexusCliError::Any)?;

    if !passed {
        return Err(NexusCliError::Any(anyhow!("TAP unit tests failed")));
    }

    if !list {
        notify_success!("TAP unit tests passed");
    }
    Ok(())
}

fn resolve_package_path(path: &Path) -> AnyResult<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("TAP package path '{}' does not exist", path.display()))?;
    let package_path = if path.is_file() {
        if path.file_name().and_then(|name| name.to_str()) != Some("Move.toml") {
            bail!(
                "TAP package path '{}' is a file other than Move.toml",
                path.display()
            );
        }
        path.parent()
            .expect("Move.toml has a parent directory")
            .to_path_buf()
    } else {
        path
    };

    let manifest = package_path.join("Move.toml");
    if !manifest.is_file() {
        bail!("TAP package '{}' has no Move.toml", package_path.display());
    }
    Ok(package_path)
}

async fn run_tap_tests(
    package_path: &Path,
    build_env: TapTestEnvironment,
    filter: Option<String>,
    list: bool,
    threads: usize,
) -> AnyResult<bool> {
    let environment = build_env.move_environment();
    let flavor = SuiFlavor::new();
    let mut build_config = BuildConfig {
        test_mode: true,
        default_flavor: Some(move_compiler::editions::Flavor::Sui),
        environment: Some(environment.name.clone()),
        ..BuildConfig::default()
    };
    // A lockfile may have been produced by a newer standalone Sui CLI. Resolve
    // fresh pins with this binary's package resolver so the compiled test
    // interfaces and framework modules remain compatible with its embedded VM.
    // `load` does not write the resulting pins back to the TAP package.
    let root_package: RootPackage<SuiFlavor> = build_config
        .package_loader(package_path, &environment, flavor.clone())
        .force_repin(true)
        .load()
        .await
        .context("could not resolve the TAP Move dependency graph")?;

    let fetch_handle = loading!("Loading published Nexus bytecode...");
    let bytecode_dependencies = fetch_nexus_bytecode(&root_package, build_env.rpc_url()).await?;
    fetch_handle.success();

    let mut test_plan = None;
    build_config.test_mode = true;
    let root_package_name = root_package.name().as_str().into();
    let mut named_addresses: Vec<_> = {
        let addresses: BuildNamedAddresses = root_package.package_info().named_addresses()?.into();
        addresses
            .inner
            .into_iter()
            .map(|(name, address)| (name.to_string(), address))
            .collect()
    };
    named_addresses.sort_by(|left, right| left.0.cmp(&right.0));

    let build_plan = BuildPlan::create(&root_package, &build_config)?;
    build_plan.compile_with_driver(&mut std::io::stdout(), |compiler| {
        let (files, parser_result) = compiler.run::<PASS_PARSER>()?;
        let parser = diagnostics::unwrap_or_report_pass_diagnostics(&files, parser_result);
        let (compiler, parser_ast) = parser.into_ast();
        let extension_locations = extension_function_locations(&parser_ast);
        let cfgir_result = compiler.at_parser(parser_ast).run::<PASS_CFGIR>();
        let compiler = diagnostics::unwrap_or_report_pass_diagnostics(&files, cfgir_result);
        let (compiler, cfgir) = compiler.into_ast();
        let compilation_environment = compiler.compilation_env();
        let tests = construct_test_plan(compilation_environment, Some(root_package_name), &cfgir);
        let mapped_files = compilation_environment.mapped_files().clone();

        let compilation_result = compiler.at_cfgir(cfgir).build();
        let (units, warnings) =
            diagnostics::unwrap_or_report_pass_diagnostics(&files, compilation_result);
        diagnostics::report_warnings(&files, warnings);
        let extension_functions = compiled_extension_functions(&units, &extension_locations)?;
        let named_units = units
            .clone()
            .into_iter()
            .map(|unit| unit.named_module)
            .collect();
        let (named_units, bytecode_dependencies) = link_nexus_test_modules(
            named_units,
            bytecode_dependencies.clone(),
            &extension_functions,
        )?;
        test_plan = Some((tests, mapped_files, named_units, bytecode_dependencies));
        Ok((files, units))
    })?;

    let (tests, mapped_files, units, bytecode_dependencies) =
        test_plan.expect("the compiler driver creates a test plan");
    let tests = tests.context("could not construct a TAP unit test plan")?;
    let test_plan = TestPlan::new(tests, mapped_files, units, bytecode_dependencies);

    let mut test_config = UnitTestingConfig::default_with_bound(Some(*MAX_UNIT_TEST_INSTRUCTIONS));
    test_config.filter = filter;
    test_config.list = list;
    test_config.num_threads = threads;
    test_config.named_address_values = named_addresses;
    test_config.report_stacktrace_on_abort = true;

    let (_, passed) = test_config.run_and_report_unit_tests(
        test_plan,
        SuiVMTestSetup::new(),
        std::io::stdout(),
    )?;
    Ok(passed)
}

fn extension_function_locations(program: &Program) -> BTreeSet<String> {
    fn collect_module(module: &ModuleDefinition, locations: &mut BTreeSet<String>) {
        if !module.is_extension {
            return;
        }
        for member in &module.members {
            if let ModuleMember::Function(function) = member {
                let location = function.name.loc();
                locations.insert(source_location_key(
                    location.file_hash(),
                    location.start(),
                    location.end(),
                ));
            }
        }
    }

    let mut locations = BTreeSet::new();
    for package in program
        .source_definitions
        .iter()
        .chain(&program.lib_definitions)
    {
        match &package.def {
            Definition::Module(module) => collect_module(module, &mut locations),
            Definition::Address(address) => {
                for module in &address.modules {
                    collect_module(module, &mut locations);
                }
            }
        }
    }
    locations
}

fn compiled_extension_functions(
    units: &[AnnotatedCompiledUnit],
    locations: &BTreeSet<String>,
) -> AnyResult<BTreeMap<String, BTreeSet<String>>> {
    let mut functions = BTreeMap::<String, BTreeSet<String>>::new();
    for unit in units {
        for (position, definition) in unit.named_module.module.function_defs.iter().enumerate() {
            let position = u16::try_from(position).context("too many Move function definitions")?;
            let source_map = unit.named_module.source_map.get_function_source_map(
                move_binary_format::file_format::FunctionDefinitionIndex::new(position),
            )?;
            let location = source_map.definition_location;
            if !locations.contains(&source_location_key(
                location.file_hash(),
                location.start(),
                location.end(),
            )) {
                continue;
            }

            let handle = unit
                .named_module
                .module
                .function_handles
                .get(definition.function.0 as usize)
                .context("compiled extension contains an invalid function handle")?;
            let name = unit
                .named_module
                .module
                .identifiers
                .get(handle.name.0 as usize)
                .context("compiled extension contains an invalid function name")?
                .to_string();
            functions
                .entry(unit.named_module.module.self_id().to_string())
                .or_default()
                .insert(name);
        }
    }
    Ok(functions)
}

fn link_nexus_test_modules(
    units: Vec<NamedCompiledModule>,
    published_modules: Vec<CompiledModule>,
    extension_functions: &BTreeMap<String, BTreeSet<String>>,
) -> AnyResult<(Vec<NamedCompiledModule>, Vec<CompiledModule>)> {
    let mut published = BTreeMap::new();
    for module in published_modules {
        let module_id = module.self_id().to_string();
        if published.insert(module_id.clone(), module).is_some() {
            bail!("received published Nexus module '{module_id}' more than once");
        }
    }
    let mut unmatched_extensions = extension_functions.keys().cloned().collect::<BTreeSet<_>>();
    let mut source_units = Vec::new();
    let mut test_dependencies = Vec::new();

    for unit in units {
        let module_id = unit.module.self_id().to_string();
        let Some(published_module) = published.remove(&module_id) else {
            if unit
                .package_name
                .as_ref()
                .is_some_and(|name| NEXUS_PACKAGE_NAMES.contains(&name.as_str()))
            {
                bail!("Nexus interface module '{module_id}' is missing from the published package");
            }
            source_units.push(unit);
            continue;
        };
        unmatched_extensions.remove(&module_id);
        let function_names = extension_functions
            .get(&module_id)
            .cloned()
            .unwrap_or_default();
        let module = if function_names.is_empty() {
            published_module
        } else {
            add_test_functions(published_module, &unit.module, &function_names)?
        };
        test_dependencies.push(module);
    }

    if !unmatched_extensions.is_empty() {
        bail!(
            "test extensions target modules that are not published by Nexus: {}",
            unmatched_extensions
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    test_dependencies.extend(published.into_values());
    test_dependencies.sort_by_key(|module| module.self_id().to_string());
    Ok((source_units, test_dependencies))
}

fn source_location_key(file_hash: impl Debug, start: u32, end: u32) -> String {
    format!("{file_hash:?}:{start}:{end}")
}

async fn fetch_nexus_bytecode(
    root_package: &RootPackage<SuiFlavor>,
    rpc_url: &str,
) -> AnyResult<Vec<CompiledModule>> {
    let resolved_packages = root_package.packages();
    if !resolved_packages
        .iter()
        .any(|package| !package.is_root() && NEXUS_PACKAGE_NAMES.contains(&package.name().as_str()))
    {
        bail!(
            "the TAP package has no supported Nexus dependency; add a @talus/nexus-* MVR dependency to Move.toml"
        );
    }

    let mut packages = resolved_packages
        .into_iter()
        .filter(|package| {
            !package.is_root() && NEXUS_PACKAGE_NAMES.contains(&package.name().as_str())
        })
        .map(|package| {
            let published = package.published().ok_or_else(|| {
                anyhow!(
                    "Nexus dependency '{}' has no published address for this environment",
                    package.name()
                )
            })?;
            Ok((
                package.name().to_string(),
                published.published_at.0.to_hex_literal(),
                published.original_id.0,
            ))
        })
        .collect::<AnyResult<Vec<_>>>()?;
    packages.sort();

    let mut seen_addresses = BTreeSet::new();
    let mut service = nexus_sdk::sui::grpc::Client::new(rpc_url)
        .context("could not create the Sui gRPC client")?
        .ledger_client();
    let mut modules = Vec::new();

    for (package_name, address, original_id) in packages {
        if !seen_addresses.insert(address.clone()) {
            continue;
        }
        let package_id = address
            .parse::<nexus_sdk::sui::types::Address>()
            .with_context(|| format!("invalid published address for '{package_name}'"))?;
        let request = nexus_sdk::sui::grpc::GetObjectRequest::default()
            .with_object_id(package_id)
            .with_read_mask(nexus_sdk::sui::grpc::FieldMask::from_paths([
                "object_id",
                "object_type",
                "package.modules.name",
                "package.modules.contents",
            ]));
        let response = service
            .get_object(request)
            .await
            .with_context(|| {
                format!("could not fetch Nexus dependency '{package_name}' at '{package_id}'")
            })?
            .into_inner();
        let object = response.object.ok_or_else(|| {
            anyhow!("Sui returned no object for Nexus dependency '{package_name}'")
        })?;
        if object.object_id_opt().and_then(|id| id.parse().ok()) != Some(package_id)
            || object.object_type_opt() != Some("package")
        {
            bail!(
                "Sui returned inconsistent package identity for Nexus dependency '{package_name}'"
            );
        }
        let package = object.package.ok_or_else(|| {
            anyhow!("Sui returned no package for Nexus dependency '{package_name}'")
        })?;

        for bytes in module_contents(&package_name, package)? {
            let module = CompiledModule::deserialize_with_defaults(&bytes).with_context(|| {
                format!("published package '{package_name}' contains invalid Move bytecode")
            })?;
            if module.self_id().address() != &original_id {
                bail!(
                    "published package '{package_name}' module '{}' has an unexpected original address",
                    module.self_id().name()
                );
            }
            modules.push(module);
        }
    }

    Ok(modules)
}

fn module_contents(
    package_name: &str,
    package: nexus_sdk::sui::grpc::Package,
) -> AnyResult<Vec<Vec<u8>>> {
    if package.modules.is_empty() {
        bail!("published package '{package_name}' contains no modules");
    }

    package
        .modules
        .into_iter()
        .map(|module| {
            let module_name = module.name.as_deref().unwrap_or("<unnamed>");
            module.contents.map(|bytes| bytes.to_vec()).ok_or_else(|| {
                anyhow!("published package '{package_name}' module '{module_name}' has no bytecode")
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_path_accepts_a_directory_or_manifest() {
        let temp = tempfile::tempdir().expect("temporary package");
        std::fs::write(temp.path().join("Move.toml"), "[package]\nname = \"tap\"\n")
            .expect("write manifest");

        let from_directory = resolve_package_path(temp.path()).expect("directory accepted");
        let from_manifest =
            resolve_package_path(&temp.path().join("Move.toml")).expect("manifest accepted");

        assert_eq!(from_directory, from_manifest);
    }

    #[test]
    fn package_path_rejects_a_directory_without_a_manifest() {
        let temp = tempfile::tempdir().expect("temporary package");
        let error = resolve_package_path(temp.path()).expect_err("manifest is required");

        assert!(error.to_string().contains("has no Move.toml"));
    }

    #[test]
    fn module_contents_requires_bytecode_for_every_module() {
        let mut module = nexus_sdk::sui::grpc::Module::default();
        module.name = Some("data".to_owned());
        let mut package = nexus_sdk::sui::grpc::Package::default();
        package.modules.push(module);

        let error = module_contents("nexus_primitives", package)
            .expect_err("missing module contents must fail");
        assert!(error.to_string().contains("module 'data' has no bytecode"));
    }

    #[test]
    fn module_contents_rejects_an_empty_package() {
        let error = module_contents("nexus_primitives", Default::default())
            .expect_err("an empty package must fail");

        assert!(error.to_string().contains("contains no modules"));
    }

    #[test]
    fn environments_select_matching_rpc_urls() {
        assert_eq!(TapTestEnvironment::Testnet.rpc_url(), TESTNET_NEXUS_RPC_URL);
        assert_eq!(TapTestEnvironment::Mainnet.rpc_url(), MAINNET_NEXUS_RPC_URL);
    }

    #[test]
    fn duplicate_published_modules_are_rejected() {
        let module = move_binary_format::file_format::empty_module();
        let error = link_nexus_test_modules(vec![], vec![module.clone(), module], &BTreeMap::new())
            .expect_err("duplicate modules must fail");

        assert!(error.to_string().contains("more than once"));
    }

    #[test]
    fn every_test_extension_requires_a_published_module() {
        let extensions = BTreeMap::from([(
            "0x42::missing".to_owned(),
            BTreeSet::from(["fixture_for_testing".to_owned()]),
        )]);
        let error = link_nexus_test_modules(vec![], vec![], &extensions)
            .expect_err("an unmatched extension must fail");

        assert!(error
            .to_string()
            .contains("modules that are not published by Nexus"));
    }
}
