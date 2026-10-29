//! Regenerate committed Move binding IR with [`talus_sui_move_codegen`].

use {
    anyhow::{anyhow, bail, Context, Result},
    std::{
        collections::BTreeMap,
        env,
        fmt::Write as _,
        fs,
        path::{Path, PathBuf},
        str::FromStr,
    },
    sui_sdk_types::Address,
    talus_sui_move_codegen::{
        apply_function_parameter_names_from_sources,
        fetch_package,
        ir::NormalizedPackage,
        GrpcClient,
    },
};

const DEFAULT_GRPC_URL: &str = "http://127.0.0.1:9000";
const IR_DIR: &str = "src/move_bindings/ir";
const PROTOCOL_LIMITS_FILE: &str = "src/move_bindings/protocol_limits.toml";
const CANONICAL_PACKAGE_VERSION: u64 = 1;
const NEXUS_PACKAGES: &[(&str, &str)] = &[
    ("primitives", "0xa1"),
    ("interface", "0xa2"),
    ("tool", "0xa7"),
    ("registry", "0xa3"),
    ("workflow", "0xa4"),
    ("scheduler", "0xa5"),
];
const KERNEL_PACKAGE: (&str, &str) = ("kernel", "0xa0");
const TALUS_PACKAGE: (&str, &str) = ("talus", "0xa6");
const SUI_FRAMEWORK_PACKAGE: (&str, &str) = ("sui_framework", "0x2");
const SUI_FRAMEWORK_MODULES: &[&str] = &[
    "accumulator",
    "bag",
    "balance",
    "clock",
    "coin",
    "funds_accumulator",
    "linked_table",
    "object",
    "object_bag",
    "object_table",
    "priority_queue",
    "sui",
    "table",
    "table_vec",
    "transfer",
    "url",
    "vec_map",
    "vec_set",
    "versioned",
];
const PROTOCOL_LIMIT_MODULES: &[(&str, &str, &str, &[&str])] = &[
    (
        "interface",
        "meta_schema",
        "interface/sources/meta_schema.move",
        &[
            "MAX_IDENTIFIER_BYTES",
            "MAX_INPUT_PORTS",
            "MAX_META_SCHEMA_BYTES",
            "MAX_OUTPUT_VARIANTS",
            "MAX_PORTS_PER_OUTPUT_VARIANT",
            "MAX_RAW_OUTPUT_BYTES",
        ],
    ),
    (
        "interface",
        "payment",
        "interface/sources/payment.move",
        &["MAX_PRIORITY_FEE_PERCENTAGE"],
    ),
    (
        "primitives",
        "data",
        "primitives/sources/data.move",
        &[
            "MAX_INLINE_DATA_BYTES",
            "MAX_MANY_VALUES",
            "MAX_NEXUS_DATA_BYTES",
        ],
    ),
    (
        "scheduler",
        "scheduler",
        "scheduler/sources/scheduler.move",
        &["LEADER_SUBMISSION_WINDOW_MS"],
    ),
];

type ProtocolLimits = BTreeMap<String, BTreeMap<String, BTreeMap<String, u64>>>;

#[derive(Debug, PartialEq, Eq)]
struct Inputs {
    metadata_file: PathBuf,
    grpc_url: String,
    source_root: Option<PathBuf>,
    output_root: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct OutputPaths {
    ir_dir: PathBuf,
    protocol_limits_file: PathBuf,
}

impl OutputPaths {
    fn from_sdk_crate_root(root: &Path) -> Self {
        Self {
            ir_dir: root.join(IR_DIR),
            protocol_limits_file: root.join(PROTOCOL_LIMITS_FILE),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    regenerate(Inputs::from_args(env::args().skip(1))?).await
}

async fn regenerate(inputs: Inputs) -> Result<()> {
    let source_root = inputs
        .source_root
        .as_deref()
        .or_else(|| {
            inputs
                .metadata_file
                .is_dir()
                .then_some(inputs.metadata_file.as_path())
        })
        .ok_or_else(|| {
            anyhow!(
                "Nexus Move source root is required to regenerate authoritative protocol limits"
            )
        })?;
    let metadata_file = if inputs.metadata_file.is_dir() {
        inputs
            .metadata_file
            .join("bin/target/deployment.localnet.json")
    } else {
        inputs.metadata_file.clone()
    };
    let package_ids = packages_from_metadata_file(&metadata_file)?;
    let output_paths = OutputPaths::from_sdk_crate_root(&inputs.output_root);
    fs::create_dir_all(&output_paths.ir_dir)
        .with_context(|| format!("create {}", output_paths.ir_dir.display()))?;
    let protocol_limits = extract_protocol_limits(source_root)?;

    let mut client = GrpcClient::new(&inputs.grpc_url)
        .map_err(|err| anyhow!("gRPC client for {}: {err}", inputs.grpc_url))?;

    let mut packages = Vec::with_capacity(package_ids.len());
    for (name, package_id) in package_ids {
        let mut package = fetch_package(&mut client, package_id)
            .await
            .with_context(|| format!("fetch {name} ({package_id})"))?;
        retain_supported_modules(&mut package, &name);
        apply_source_names(&mut package, &name, Some(source_root))?;
        packages.push((name, package));
    }

    canonicalize_sdk_ir(&mut packages);
    write_protocol_limits(&output_paths.protocol_limits_file, &protocol_limits)?;
    for (name, package) in packages {
        if name == KERNEL_PACKAGE.0 {
            continue;
        }
        let module_count = package.modules.len();
        let path = write_package_ir(&output_paths.ir_dir, &name, &package)?;
        println!("wrote {} ({} modules)", path.display(), module_count);
    }

    Ok(())
}

fn extract_protocol_limits(source_root: &Path) -> Result<ProtocolLimits> {
    let mut limits = ProtocolLimits::new();
    for (package, module, relative_path, required) in PROTOCOL_LIMIT_MODULES {
        let path = source_root.join(relative_path);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read protocol limit source {}", path.display()))?;
        let module_limits =
            extract_required_u64_constants(&source, &path.display().to_string(), required)?;
        limits
            .entry((*package).to_string())
            .or_default()
            .insert((*module).to_string(), module_limits);
    }
    Ok(limits)
}

fn extract_required_u64_constants(
    source: &str,
    source_name: &str,
    required: &[&str],
) -> Result<BTreeMap<String, u64>> {
    let mut constants = BTreeMap::new();
    for (line_index, line) in source.lines().enumerate() {
        let declaration = line.split("//").next().unwrap_or_default().trim();
        let Some(declaration) = declaration.strip_prefix("const ") else {
            continue;
        };
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if !required.contains(&name) {
            continue;
        }
        let value = value
            .trim()
            .strip_prefix("u64")
            .map(str::trim_start)
            .and_then(|value| value.strip_prefix('='))
            .map(str::trim)
            .and_then(|value| value.strip_suffix(';'))
            .ok_or_else(|| {
                anyhow!(
                    "{source_name}:{} must declare '{name}' as a literal u64 constant",
                    line_index + 1
                )
            })?;
        let normalized = value.trim().replace('_', "");
        let parsed = if let Some(hex) = normalized.strip_prefix("0x") {
            u64::from_str_radix(hex, 16)
        } else {
            normalized.parse()
        }
        .with_context(|| {
            format!(
                "{source_name}:{} contains an invalid u64 value for '{name}'",
                line_index + 1
            )
        })?;
        if parsed > i64::MAX as u64 {
            bail!(
                "{source_name}:{} value for '{name}' exceeds the generated TOML integer range",
                line_index + 1
            );
        }
        if constants.insert(name.to_string(), parsed).is_some() {
            bail!("{source_name} declares required constant '{name}' more than once");
        }
    }

    for name in required {
        if !constants.contains_key(*name) {
            bail!("{source_name} is missing required u64 constant '{name}'");
        }
    }
    Ok(constants)
}

fn render_protocol_limits(limits: &ProtocolLimits) -> String {
    let mut output = String::from(
        "# @generated by regenerate_bindings from authoritative Nexus Move source; do not edit.\n",
    );
    for (package, modules) in limits {
        for (module, constants) in modules {
            writeln!(output, "\n[{package}.{module}]").expect("writing to String cannot fail");
            for (name, value) in constants {
                writeln!(output, "{name} = {value}").expect("writing to String cannot fail");
            }
        }
    }
    output
}

fn write_protocol_limits(path: &Path, limits: &ProtocolLimits) -> Result<()> {
    fs::write(path, render_protocol_limits(limits))
        .with_context(|| format!("write {}", path.display()))
}

fn retain_supported_modules(package: &mut NormalizedPackage, package_name: &str) {
    if package_name == SUI_FRAMEWORK_PACKAGE.0 {
        package
            .modules
            .retain(|module, _| SUI_FRAMEWORK_MODULES.contains(&module.as_str()));
    }
}

fn apply_source_names(
    package: &mut NormalizedPackage,
    package_name: &str,
    source_root: Option<&Path>,
) -> Result<()> {
    let Some(source_root) = source_root else {
        return Ok(());
    };
    if !source_root.is_dir() {
        bail!("source root is not a directory: {}", source_root.display());
    }

    let source_dir = source_root.join(package_name).join("sources");
    if !source_dir.is_dir() {
        if NEXUS_PACKAGES.iter().any(|(name, _)| *name == package_name) {
            bail!(
                "source directory for {package_name} does not exist: {}",
                source_dir.display()
            );
        }
        return Ok(());
    }
    apply_function_parameter_names_from_sources(package, &source_dir)
        .with_context(|| format!("apply parameter names from {}", source_dir.display()))
}

fn canonicalize_sdk_ir(packages: &mut [(String, NormalizedPackage)]) {
    let mut replacements = Vec::new();
    for (name, package) in packages.iter_mut() {
        package.version = CANONICAL_PACKAGE_VERSION;
        let canonical_id = NEXUS_PACKAGES
            .iter()
            .find(|(package_name, _)| package_name == name)
            .map(|(_, canonical_id)| *canonical_id)
            .or_else(|| (name == KERNEL_PACKAGE.0).then_some(KERNEL_PACKAGE.1))
            .or_else(|| (name == TALUS_PACKAGE.0).then_some(TALUS_PACKAGE.1));
        let Some(canonical_id) = canonical_id else {
            continue;
        };

        replacements.push((package.storage_id.clone(), canonical_id.to_string()));
        if let Some(original_id) = &package.original_id {
            replacements.push((original_id.clone(), canonical_id.to_string()));
        }
    }

    for (_, package) in packages {
        package.replace_addresses(&replacements);
    }
}

impl Inputs {
    fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut positional = Vec::new();
        let mut source_root = None;
        let mut output_root = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--source-root" => {
                    if source_root.is_some() {
                        bail!("--source-root may only be provided once");
                    }
                    let path = args
                        .next()
                        .ok_or_else(|| anyhow!("--source-root requires a path"))?;
                    source_root = Some(PathBuf::from(path));
                }
                "--output-root" => {
                    if output_root.is_some() {
                        bail!("--output-root may only be provided once");
                    }
                    let path = args
                        .next()
                        .filter(|path| !path.is_empty())
                        .ok_or_else(|| anyhow!("--output-root requires a non-empty path"))?;
                    output_root = Some(PathBuf::from(path));
                }
                option if option.starts_with('-') => bail!("unknown option: {option}"),
                _ => positional.push(arg),
            }
        }

        let output_root = output_root.ok_or_else(|| anyhow!("--output-root is required"))?;
        match positional.as_slice() {
            [metadata_file] => Ok(Self {
                metadata_file: PathBuf::from(metadata_file),
                grpc_url: DEFAULT_GRPC_URL.to_string(),
                source_root,
                output_root,
            }),
            [metadata_file, grpc_url] => Ok(Self {
                metadata_file: PathBuf::from(metadata_file),
                grpc_url: grpc_url.to_string(),
                source_root,
                output_root,
            }),
            _ => bail!(
                "expected: regenerate_bindings <deployment_or_candidate_json> [grpc_url] \
                 [--source-root <path>] --output-root <sdk_crate_root>"
            ),
        }
    }
}

fn packages_from_metadata_file(path: &Path) -> Result<Vec<(String, Address)>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let metadata: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    packages_from_metadata(&metadata, path)
}

fn packages_from_metadata(
    metadata: &serde_json::Value,
    path: &Path,
) -> Result<Vec<(String, Address)>> {
    let source = path.display();
    let kind = metadata
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("{source} is missing kind"))?;
    let mut deployment = metadata.clone();
    if kind == "candidate" {
        let base = metadata
            .get("base_deployment")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("{source} is missing base_deployment"))?;
        let base_path = PathBuf::from(base);
        let base_path = if base_path.is_absolute() {
            base_path
        } else {
            path.parent()
                .unwrap_or_else(|| Path::new("."))
                .join(base_path)
        };
        let text = fs::read_to_string(&base_path)
            .with_context(|| format!("read {}", base_path.display()))?;
        deployment = serde_json::from_str(&text)
            .with_context(|| format!("parse {}", base_path.display()))?;
        if deployment.get("kind").and_then(serde_json::Value::as_str) != Some("deployment") {
            bail!("{} is not a deployment document", base_path.display());
        }
        let changed = metadata
            .get("packages")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| anyhow!("{source} is missing packages"))?;
        let packages = deployment
            .get_mut("packages")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| anyhow!("{} is missing packages", base_path.display()))?;
        for (name, package) in changed {
            packages.insert(name.clone(), package.clone());
        }
    } else if kind != "deployment" {
        bail!("{source} has unsupported metadata kind '{kind}'");
    }

    let mut packages = Vec::new();
    for (package, _) in NEXUS_PACKAGES {
        let id = deployment
            .get("packages")
            .and_then(|value| value.get(package))
            .and_then(|value| value.get("storage_id"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("{source} is missing packages.{package}.storage_id"))?;
        packages.push(((*package).to_string(), parse_address(id)?));
    }
    let kernel_id = deployment
        .get("kernel")
        .and_then(|value| value.get("package_id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("{source} is missing kernel.package_id"))?;
    packages.push((KERNEL_PACKAGE.0.to_string(), parse_address(kernel_id)?));
    let talus_id = deployment
        .get("external_us")
        .and_then(|value| value.get("package_id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("{source} is missing external_us.package_id"))?;
    packages.push((TALUS_PACKAGE.0.to_string(), parse_address(talus_id)?));
    packages.push((
        SUI_FRAMEWORK_PACKAGE.0.to_string(),
        parse_address(SUI_FRAMEWORK_PACKAGE.1)?,
    ));
    Ok(packages)
}

fn parse_address(input: &str) -> Result<Address> {
    Address::from_str(input).with_context(|| format!("invalid package id {input}"))
}

fn write_package_ir(out_dir: &Path, name: &str, package: &NormalizedPackage) -> Result<PathBuf> {
    let json = package
        .to_json_string()
        .with_context(|| format!("serialize IR JSON for {name}"))?;
    let path = out_dir.join(format!("{name}.json"));
    fs::write(&path, format!("{json}\n")).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{collections::BTreeMap, process::Command},
        talus_sui_move_codegen::ir::{
            Datatype,
            DatatypeKind,
            Function,
            FunctionParam,
            NormalizedModule,
            TypeName,
            TypeRef,
            Visibility,
        },
    };

    #[test]
    fn extracts_required_move_u64_constants() {
        let source = r#"
            const IGNORED: u64 = 9;
            const MAX_FIRST: u64 = 65_536;
            const MAX_SECOND: u64 = 0x80; // source comment
        "#;

        let constants =
            extract_required_u64_constants(source, "limits.move", &["MAX_FIRST", "MAX_SECOND"])
                .unwrap();

        assert_eq!(
            constants,
            BTreeMap::from([
                ("MAX_FIRST".to_string(), 65_536),
                ("MAX_SECOND".to_string(), 128),
            ])
        );
    }

    #[test]
    fn rejects_missing_or_duplicate_move_limits() {
        let missing = extract_required_u64_constants(
            "const MAX_FIRST: u64 = 1;",
            "limits.move",
            &["MAX_FIRST", "MAX_SECOND"],
        )
        .unwrap_err()
        .to_string();
        assert!(missing.contains("missing required u64 constant 'MAX_SECOND'"));

        let duplicate = extract_required_u64_constants(
            "const MAX_FIRST: u64 = 1;\nconst MAX_FIRST: u64 = 2;",
            "limits.move",
            &["MAX_FIRST"],
        )
        .unwrap_err()
        .to_string();
        assert!(duplicate.contains("more than once"));
    }

    #[test]
    fn rejects_malformed_or_out_of_range_move_limits() {
        let malformed = extract_required_u64_constants(
            "const MAX_FIRST: u64 = 1 << 10;",
            "limits.move",
            &["MAX_FIRST"],
        )
        .unwrap_err()
        .to_string();
        assert!(malformed.contains("invalid u64 value"));

        let out_of_range = extract_required_u64_constants(
            "const MAX_FIRST: u64 = 18_446_744_073_709_551_615;",
            "limits.move",
            &["MAX_FIRST"],
        )
        .unwrap_err()
        .to_string();
        assert!(out_of_range.contains("exceeds the generated TOML integer range"));
    }

    #[test]
    fn renders_protocol_limits_deterministically() {
        let limits = BTreeMap::from([
            (
                "primitives".to_string(),
                BTreeMap::from([(
                    "data".to_string(),
                    BTreeMap::from([("MAX_DATA".to_string(), 64)]),
                )]),
            ),
            (
                "interface".to_string(),
                BTreeMap::from([(
                    "meta_schema".to_string(),
                    BTreeMap::from([("MAX_PORTS".to_string(), 32)]),
                )]),
            ),
        ]);

        assert_eq!(
            render_protocol_limits(&limits),
            concat!(
                "# @generated by regenerate_bindings from authoritative Nexus Move source; do not edit.\n",
                "\n[interface.meta_schema]\n",
                "MAX_PORTS = 32\n",
                "\n[primitives.data]\n",
                "MAX_DATA = 64\n",
            )
        );
    }

    #[test]
    fn parses_required_rebind_packages() {
        let deployment = serde_json::json!({
            "kind": "deployment",
            "packages": {
                "primitives": {"storage_id": "0x11"},
                "interface": {"storage_id": "0x12"},
                "registry": {"storage_id": "0x13"},
                "workflow": {"storage_id": "0x14"},
                "scheduler": {"storage_id": "0x15"},
                "tool": {"storage_id": "0x17"}
            },
            "kernel": {"package_id": "0x18"},
            "external_us": {"package_id": "0x16"}
        });
        let packages = packages_from_metadata(&deployment, Path::new("deployment.json"))
            .expect("deployment JSON parses");

        assert_eq!(
            packages,
            vec![
                ("primitives".to_string(), Address::from_str("0x11").unwrap()),
                ("interface".to_string(), Address::from_str("0x12").unwrap()),
                ("tool".to_string(), Address::from_str("0x17").unwrap()),
                ("registry".to_string(), Address::from_str("0x13").unwrap()),
                ("workflow".to_string(), Address::from_str("0x14").unwrap()),
                ("scheduler".to_string(), Address::from_str("0x15").unwrap()),
                ("kernel".to_string(), Address::from_str("0x18").unwrap()),
                ("talus".to_string(), Address::from_str("0x16").unwrap()),
                (
                    "sui_framework".to_string(),
                    Address::from_str("0x2").unwrap()
                ),
            ]
        );
    }

    #[test]
    fn retains_only_supported_sui_framework_modules() {
        let mut package = NormalizedPackage {
            storage_id: "0x2".to_string(),
            original_id: Some("0x2".to_string()),
            version: 1,
            modules: BTreeMap::from([
                (
                    "object".to_string(),
                    NormalizedModule {
                        name: "object".to_string(),
                        datatypes: vec![],
                        functions: vec![],
                    },
                ),
                (
                    "unsupported".to_string(),
                    NormalizedModule {
                        name: "unsupported".to_string(),
                        datatypes: vec![],
                        functions: vec![],
                    },
                ),
            ]),
        };

        retain_supported_modules(&mut package, SUI_FRAMEWORK_PACKAGE.0);

        assert_eq!(package.modules.keys().collect::<Vec<_>>(), ["object"]);
    }

    #[test]
    fn does_not_filter_nexus_modules() {
        let mut package = NormalizedPackage {
            storage_id: "0x11".to_string(),
            original_id: Some("0x11".to_string()),
            version: 1,
            modules: BTreeMap::from([(
                "unsupported".to_string(),
                NormalizedModule {
                    name: "unsupported".to_string(),
                    datatypes: vec![],
                    functions: vec![],
                },
            )]),
        };

        retain_supported_modules(&mut package, "primitives");

        assert!(package.modules.contains_key("unsupported"));
    }

    #[test]
    fn rejects_deployment_without_talus_package() {
        let deployment = serde_json::json!({
            "kind": "deployment",
            "packages": {
                "primitives": {"storage_id": "0x11"},
                "interface": {"storage_id": "0x12"},
                "registry": {"storage_id": "0x13"},
                "workflow": {"storage_id": "0x14"},
                "scheduler": {"storage_id": "0x15"},
                "tool": {"storage_id": "0x17"}
            },
            "kernel": {"package_id": "0x18"}
        });

        let error = packages_from_metadata(&deployment, Path::new("deployment.json"))
            .expect_err("Talus package metadata is required");

        assert!(error.to_string().contains("external_us.package_id"));
    }

    #[test]
    fn accepts_metadata_file_and_optional_grpc_url() {
        assert_eq!(
            Inputs::from_args([
                "objects.toml".to_string(),
                "--output-root".to_string(),
                "/selected/sdk".to_string(),
            ])
            .unwrap(),
            Inputs {
                metadata_file: PathBuf::from("objects.toml"),
                grpc_url: DEFAULT_GRPC_URL.to_string(),
                source_root: None,
                output_root: PathBuf::from("/selected/sdk"),
            }
        );

        assert_eq!(
            Inputs::from_args([
                "objects.toml".to_string(),
                "http://localhost:9000".to_string(),
                "--output-root".to_string(),
                "/selected/sdk".to_string(),
            ])
            .unwrap(),
            Inputs {
                metadata_file: PathBuf::from("objects.toml"),
                grpc_url: "http://localhost:9000".to_string(),
                source_root: None,
                output_root: PathBuf::from("/selected/sdk"),
            }
        );
    }

    #[test]
    fn accepts_explicit_source_root() {
        assert_eq!(
            Inputs::from_args([
                "objects.toml".to_string(),
                "--source-root".to_string(),
                "../nexus/sui".to_string(),
                "--output-root".to_string(),
                "/selected/sdk".to_string(),
            ])
            .unwrap(),
            Inputs {
                metadata_file: PathBuf::from("objects.toml"),
                grpc_url: DEFAULT_GRPC_URL.to_string(),
                source_root: Some(PathBuf::from("../nexus/sui")),
                output_root: PathBuf::from("/selected/sdk"),
            }
        );

        assert_eq!(
            Inputs::from_args([
                "objects.toml".to_string(),
                "http://localhost:9000".to_string(),
                "--source-root".to_string(),
                "../nexus/sui".to_string(),
                "--output-root".to_string(),
                "/selected/sdk".to_string(),
            ])
            .unwrap(),
            Inputs {
                metadata_file: PathBuf::from("objects.toml"),
                grpc_url: "http://localhost:9000".to_string(),
                source_root: Some(PathBuf::from("../nexus/sui")),
                output_root: PathBuf::from("/selected/sdk"),
            }
        );
    }

    #[test]
    fn runtime_output_root_routes_ir_and_protocol_limits_to_selected_checkout() {
        let tempdir = tempfile::tempdir().expect("create temporary directory");
        let sdk_root = tempdir.path().join("current-checkout").join("sdk");
        let compile_time_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert_ne!(sdk_root, compile_time_root);

        let inputs = Inputs::from_args([
            "objects.toml".to_string(),
            "http://localhost:9000".to_string(),
            "--source-root".to_string(),
            "../nexus/sui".to_string(),
            "--output-root".to_string(),
            sdk_root.display().to_string(),
        ])
        .expect("parse selected checkout output root");
        assert_eq!(inputs.output_root, sdk_root);
        assert_eq!(inputs.source_root, Some(PathBuf::from("../nexus/sui")));
        assert_eq!(inputs.grpc_url, "http://localhost:9000");

        let output_paths = OutputPaths::from_sdk_crate_root(&inputs.output_root);
        assert_eq!(output_paths.ir_dir, sdk_root.join(IR_DIR));
        assert_eq!(
            output_paths.protocol_limits_file,
            sdk_root.join(PROTOCOL_LIMITS_FILE)
        );
        assert!(!output_paths.ir_dir.starts_with(&compile_time_root));
        assert!(!output_paths
            .protocol_limits_file
            .starts_with(&compile_time_root));

        fs::create_dir_all(&output_paths.ir_dir).expect("create selected output directory");
        let package = NormalizedPackage {
            storage_id: "0xa1".to_string(),
            original_id: None,
            version: 1,
            modules: BTreeMap::new(),
        };
        let ir_path = write_package_ir(&output_paths.ir_dir, "primitives", &package)
            .expect("write IR into selected checkout");
        let limits = BTreeMap::from([(
            "interface".to_string(),
            BTreeMap::from([(
                "payment".to_string(),
                BTreeMap::from([("MAX_PRIORITY_FEE_PERCENTAGE".to_string(), 10)]),
            )]),
        )]);
        write_protocol_limits(&output_paths.protocol_limits_file, &limits)
            .expect("write protocol limits into selected checkout");

        assert_eq!(ir_path, output_paths.ir_dir.join("primitives.json"));
        assert!(ir_path.starts_with(&sdk_root));
        assert!(output_paths.protocol_limits_file.starts_with(&sdk_root));
        let written_ir = fs::read_to_string(&ir_path).expect("read generated IR");
        assert_eq!(
            NormalizedPackage::from_json_str(&written_ir)
                .expect("decode generated IR")
                .storage_id,
            "0xa1"
        );
        let written_limits = fs::read_to_string(&output_paths.protocol_limits_file)
            .expect("read generated protocol limits");
        assert!(written_limits.contains("MAX_PRIORITY_FEE_PERCENTAGE = 10"));
    }

    #[test]
    fn rebind_recipe_anchors_outputs_to_sdk_checkout_and_inputs_to_caller() {
        let sdk_crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let sdk_checkout_root = sdk_crate_root
            .parent()
            .expect("SDK crate is inside the repository checkout")
            .to_path_buf();
        let justfile = sdk_checkout_root.join("justfile");
        let external_caller = tempfile::tempdir().expect("create external caller directory");
        let callers = [
            (sdk_checkout_root.clone(), false),
            (sdk_crate_root.clone(), false),
            (external_caller.path().to_path_buf(), true),
        ];

        for (caller, explicit_justfile) in callers {
            let mut command = Command::new("just");
            command.current_dir(&caller).args(["--color", "never"]);
            if explicit_justfile {
                command.args(["--justfile", justfile.to_str().unwrap()]);
            }
            let output = command
                .args([
                    "--dry-run",
                    "sdk",
                    "rebind",
                    "deployment.json",
                    "http://127.0.0.1:9000",
                    "../nexus/sui",
                ])
                .output()
                .expect("run Just rebind dry run");
            assert!(
                output.status.success(),
                "Just rebind dry run failed in {}: {}",
                caller.display(),
                String::from_utf8_lossy(&output.stderr)
            );

            let script = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                script.contains(&format!("output_root=\"{}\"", sdk_crate_root.display())),
                "SDK output root did not follow the selected justfile from {}:\n{}",
                caller.display(),
                script
            );
            assert!(script.contains("objects_file=\"deployment.json\""));
            assert!(
                script.contains(&format!(
                    "*) objects_file=\"{}/$objects_file\" ;;",
                    caller.display()
                )),
                "relative metadata path was not anchored to its caller in {}:\n{}",
                caller.display(),
                script
            );
            assert!(script.contains("source_root=\"../nexus/sui\""));
            assert!(
                script.contains(&format!(
                    "*) source_root=\"{}/$source_root\" ;;",
                    caller.display()
                )),
                "relative Move source path was not anchored to its caller in {}:\n{}",
                caller.display(),
                script
            );
            assert!(script.contains("--output-root \"$output_root\""));
        }
    }

    #[test]
    fn rejects_missing_empty_or_duplicate_output_roots() {
        let omitted = Inputs::from_args(["objects.toml".to_string()])
            .expect_err("output root is required")
            .to_string();
        assert!(omitted.contains("--output-root is required"));

        let missing = Inputs::from_args(["objects.toml".to_string(), "--output-root".to_string()])
            .expect_err("output root requires a path")
            .to_string();
        assert!(missing.contains("--output-root requires a non-empty path"));

        let empty = Inputs::from_args([
            "objects.toml".to_string(),
            "--output-root".to_string(),
            "".to_string(),
        ])
        .expect_err("empty output root is invalid")
        .to_string();
        assert!(empty.contains("--output-root requires a non-empty path"));

        let duplicate = Inputs::from_args([
            "objects.toml".to_string(),
            "--output-root".to_string(),
            "/current/sdk".to_string(),
            "--output-root".to_string(),
            "/other/sdk".to_string(),
        ])
        .expect_err("output root can only be supplied once")
        .to_string();
        assert!(duplicate.contains("--output-root may only be provided once"));
    }

    #[test]
    fn writes_ir_without_changing_addresses_modules_or_functions() {
        let out_dir = tempfile::tempdir().expect("create temporary directory");
        let package = NormalizedPackage {
            storage_id: "0x111".to_string(),
            original_id: Some("0x222".to_string()),
            version: 1,
            modules: BTreeMap::from([
                (
                    "m".to_string(),
                    NormalizedModule {
                        name: "m".to_string(),
                        datatypes: vec![Datatype {
                            type_name: TypeName::parse("0x111::m::Obj").unwrap(),
                            module: "m".to_string(),
                            name: "Obj".to_string(),
                            abilities: vec![],
                            type_parameters: vec![],
                            kind: DatatypeKind::Struct { fields: vec![] },
                        }],
                        functions: vec![Function {
                            name: "keep_me".to_string(),
                            visibility: Visibility::Public,
                            is_entry: true,
                            type_parameters: vec![],
                            parameters: vec![FunctionParam {
                                name: "arg0".to_string(),
                                ty: TypeRef::U64,
                            }],
                            return_types: vec![],
                        }],
                    },
                ),
                (
                    "module_that_sdk_used_to_filter".to_string(),
                    NormalizedModule {
                        name: "module_that_sdk_used_to_filter".to_string(),
                        datatypes: vec![],
                        functions: vec![],
                    },
                ),
            ]),
        };

        let path = write_package_ir(out_dir.path(), "primitives", &package).expect("write IR");
        let written = fs::read_to_string(path).expect("read IR");
        let decoded = NormalizedPackage::from_json_str(&written).expect("decode IR");

        assert_eq!(decoded.storage_id, "0x111");
        assert_eq!(decoded.original_id.as_deref(), Some("0x222"));
        assert!(decoded
            .modules
            .contains_key("module_that_sdk_used_to_filter"));
        assert_eq!(decoded.modules["m"].functions[0].name, "keep_me");
    }

    #[test]
    fn canonicalizes_same_abi_across_deployments() {
        fn package(storage_id: &str, original_id: &str, dependency_id: &str) -> NormalizedPackage {
            NormalizedPackage {
                storage_id: storage_id.to_string(),
                original_id: Some(original_id.to_string()),
                version: 1,
                modules: BTreeMap::from([(
                    "m".to_string(),
                    NormalizedModule {
                        name: "m".to_string(),
                        datatypes: vec![],
                        functions: vec![Function {
                            name: "use_dependency".to_string(),
                            visibility: Visibility::Public,
                            is_entry: true,
                            type_parameters: vec![],
                            parameters: vec![FunctionParam {
                                name: "arg0".to_string(),
                                ty: TypeRef::Datatype {
                                    type_name: TypeName::parse(&format!("{dependency_id}::m::Obj"))
                                        .unwrap(),
                                    type_arguments: vec![],
                                },
                            }],
                            return_types: vec![],
                        }],
                    },
                )]),
            }
        }

        let mut first = vec![
            ("kernel".to_string(), package("0x01", "0x01", "0x20")),
            ("primitives".to_string(), package("0x11", "0x10", "0x20")),
            ("interface".to_string(), package("0x21", "0x20", "0x10")),
            ("talus".to_string(), package("0x31", "0x30", "0x10")),
        ];
        let mut second = vec![
            ("kernel".to_string(), package("0x001", "0x001", "0x220")),
            ("primitives".to_string(), package("0x111", "0x110", "0x220")),
            ("interface".to_string(), package("0x221", "0x220", "0x110")),
            ("talus".to_string(), package("0x331", "0x330", "0x110")),
        ];
        second[0].1.version = 5;
        second[1].1.version = 7;
        second[2].1.version = 9;
        second[3].1.version = 11;

        canonicalize_sdk_ir(&mut first);
        canonicalize_sdk_ir(&mut second);

        assert_eq!(first, second);
        assert_eq!(first[0].1.storage_id, "0xa0");
        assert_eq!(first[0].1.original_id.as_deref(), Some("0xa0"));
        assert_eq!(first[1].1.storage_id, "0xa1");
        assert_eq!(first[1].1.original_id.as_deref(), Some("0xa1"));
        assert_eq!(first[3].1.storage_id, "0xa6");
        assert_eq!(first[3].1.original_id.as_deref(), Some("0xa6"));
        assert_eq!(
            first[1].1.modules["m"].functions[0].parameters[0].ty,
            TypeRef::Datatype {
                type_name: TypeName::parse("0xa2::m::Obj").unwrap(),
                type_arguments: vec![],
            }
        );
    }

    #[test]
    fn overlays_parameter_names_from_explicit_source_root() {
        let source_root = tempfile::tempdir().expect("create temporary directory");
        let source_dir = source_root.path().join("primitives/sources");
        fs::create_dir_all(&source_dir).expect("create source directory");
        fs::write(
            source_dir.join("m.move"),
            "module nexus_primitives::m; public fun keep_me(amount: u64) {}",
        )
        .expect("write Move source");

        let mut package = NormalizedPackage {
            storage_id: "0x111".to_string(),
            original_id: Some("0x111".to_string()),
            version: 1,
            modules: BTreeMap::from([(
                "m".to_string(),
                NormalizedModule {
                    name: "m".to_string(),
                    datatypes: vec![],
                    functions: vec![Function {
                        name: "keep_me".to_string(),
                        visibility: Visibility::Public,
                        is_entry: true,
                        type_parameters: vec![],
                        parameters: vec![FunctionParam {
                            name: "arg0".to_string(),
                            ty: TypeRef::U64,
                        }],
                        return_types: vec![],
                    }],
                },
            )]),
        };

        apply_source_names(&mut package, "primitives", None).expect("keep network names");
        assert_eq!(package.modules["m"].functions[0].parameters[0].name, "arg0");

        apply_source_names(&mut package, "primitives", Some(source_root.path()))
            .expect("apply source names");

        assert_eq!(
            package.modules["m"].functions[0].parameters[0].name,
            "amount"
        );
    }

    #[test]
    fn rejects_missing_source_root() {
        let parent = tempfile::tempdir().expect("create temporary directory");
        let missing = parent.path().join("missing");
        let mut package = NormalizedPackage {
            storage_id: "0x111".to_string(),
            original_id: Some("0x111".to_string()),
            version: 1,
            modules: BTreeMap::new(),
        };

        let error = apply_source_names(&mut package, "primitives", Some(&missing))
            .expect_err("reject missing source root");

        assert!(error.to_string().contains("source root is not a directory"));
    }

    #[test]
    fn rejects_missing_nexus_package_source_directory() {
        let source_root = tempfile::tempdir().expect("create temporary directory");
        let mut package = NormalizedPackage {
            storage_id: "0x111".to_string(),
            original_id: Some("0x111".to_string()),
            version: 1,
            modules: BTreeMap::new(),
        };

        let error = apply_source_names(&mut package, "primitives", Some(source_root.path()))
            .expect_err("reject missing Nexus package source directory");

        assert!(error
            .to_string()
            .contains("source directory for primitives does not exist"));
    }
}
