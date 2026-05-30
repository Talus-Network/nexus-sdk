//! Regenerate committed Move binding IR with [`sui_move_codegen`].

use {
    anyhow::{anyhow, bail, Context, Result},
    std::{
        env,
        fs,
        path::{Path, PathBuf},
        str::FromStr,
    },
    sui_move_codegen::{
        apply_function_parameter_names_from_sources,
        fetch_package,
        ir::NormalizedPackage,
        GrpcClient,
    },
    sui_sdk_types::Address,
};

const DEFAULT_GRPC_URL: &str = "http://127.0.0.1:9000";
const IR_DIR: &str = "src/move_bindings/ir";
const CANONICAL_PACKAGE_VERSION: u64 = 1;
const NEXUS_PACKAGES: &[(&str, &str)] = &[
    ("primitives", "0xa1"),
    ("interface", "0xa2"),
    ("registry", "0xa3"),
    ("workflow", "0xa4"),
    ("scheduler", "0xa5"),
];
const TALUS_PACKAGE: (&str, &str) = ("talus", "0xa6");

#[derive(Debug, PartialEq, Eq)]
struct Inputs {
    objects_file: PathBuf,
    grpc_url: String,
    source_root: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    regenerate(Inputs::from_args(env::args().skip(1))?).await
}

async fn regenerate(inputs: Inputs) -> Result<()> {
    let source_root = inputs.source_root.as_deref().or_else(|| {
        inputs
            .objects_file
            .is_dir()
            .then_some(inputs.objects_file.as_path())
    });
    let objects_file = if inputs.objects_file.is_dir() {
        inputs.objects_file.join("bin/target/objects.localnet.toml")
    } else {
        inputs.objects_file.clone()
    };
    let package_ids = packages_from_objects_file(&objects_file)?;
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(IR_DIR);
    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir.display()))?;

    let mut client = GrpcClient::new(&inputs.grpc_url)
        .map_err(|err| anyhow!("gRPC client for {}: {err}", inputs.grpc_url))?;

    let mut packages = Vec::with_capacity(package_ids.len());
    for (name, package_id) in package_ids {
        let mut package = fetch_package(&mut client, package_id)
            .await
            .with_context(|| format!("fetch {name} ({package_id})"))?;
        apply_source_names(&mut package, &name, source_root)?;
        packages.push((name, package));
    }

    canonicalize_sdk_ir(&mut packages);
    for (name, package) in packages {
        let module_count = package.modules.len();
        let path = write_package_ir(&out_dir, &name, &package)?;
        println!("wrote {} ({} modules)", path.display(), module_count);
    }

    Ok(())
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
    if NEXUS_PACKAGES.iter().any(|(name, _)| *name == package_name) && !source_dir.is_dir() {
        bail!(
            "source directory for {package_name} does not exist: {}",
            source_dir.display()
        );
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
                option if option.starts_with('-') => bail!("unknown option: {option}"),
                _ => positional.push(arg),
            }
        }

        match positional.as_slice() {
            [objects_file] => Ok(Self {
                objects_file: PathBuf::from(objects_file),
                grpc_url: DEFAULT_GRPC_URL.to_string(),
                source_root,
            }),
            [objects_file, grpc_url] => Ok(Self {
                objects_file: PathBuf::from(objects_file),
                grpc_url: grpc_url.to_string(),
                source_root,
            }),
            _ => bail!(
                "expected: regenerate_bindings <objects_toml> [grpc_url] [--source-root <path>]"
            ),
        }
    }
}

fn packages_from_objects_file(path: &Path) -> Result<Vec<(String, Address)>> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    packages_from_objects_toml(&text, &path.display().to_string())
}

fn packages_from_objects_toml(text: &str, source: &str) -> Result<Vec<(String, Address)>> {
    let parsed: toml::Value = toml::from_str(text).with_context(|| format!("parse {source}"))?;

    let mut packages = Vec::new();
    for (package, _) in NEXUS_PACKAGES {
        let key = format!("{package}_pkg_id");
        let id = parsed
            .get(&key)
            .and_then(toml::Value::as_str)
            .ok_or_else(|| anyhow!("{source} is missing {key}"))?;
        packages.push(((*package).to_string(), parse_address(id)?));
    }
    let talus_id = parsed
        .get("us_token")
        .and_then(|value| value.get("package_id"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| anyhow!("{source} is missing us_token.package_id"))?;
    packages.push((TALUS_PACKAGE.0.to_string(), parse_address(talus_id)?));
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
        std::collections::BTreeMap,
        sui_move_codegen::ir::{
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
    fn parses_required_nexus_packages() {
        let objects = [
            r#"primitives_pkg_id = "0x11""#,
            r#"interface_pkg_id = "0x12""#,
            r#"registry_pkg_id = "0x13""#,
            r#"workflow_pkg_id = "0x14""#,
            r#"scheduler_pkg_id = "0x15""#,
            r#"[us_token]"#,
            r#"package_id = "0x16""#,
        ]
        .join("\n");
        let packages = packages_from_objects_toml(&objects, "objects.localnet.toml")
            .expect("objects TOML parses");

        assert_eq!(
            packages,
            vec![
                ("primitives".to_string(), Address::from_str("0x11").unwrap()),
                ("interface".to_string(), Address::from_str("0x12").unwrap()),
                ("registry".to_string(), Address::from_str("0x13").unwrap()),
                ("workflow".to_string(), Address::from_str("0x14").unwrap()),
                ("scheduler".to_string(), Address::from_str("0x15").unwrap()),
                ("talus".to_string(), Address::from_str("0x16").unwrap()),
            ]
        );
    }

    #[test]
    fn rejects_objects_without_talus_package() {
        let objects = [
            r#"primitives_pkg_id = "0x11""#,
            r#"interface_pkg_id = "0x12""#,
            r#"registry_pkg_id = "0x13""#,
            r#"workflow_pkg_id = "0x14""#,
            r#"scheduler_pkg_id = "0x15""#,
        ]
        .join("\n");

        let error = packages_from_objects_toml(&objects, "objects.toml")
            .expect_err("Talus package metadata is required");

        assert!(error.to_string().contains("us_token.package_id"));
    }

    #[test]
    fn accepts_objects_file_and_optional_grpc_url() {
        assert_eq!(
            Inputs::from_args(["objects.toml".to_string()]).unwrap(),
            Inputs {
                objects_file: PathBuf::from("objects.toml"),
                grpc_url: DEFAULT_GRPC_URL.to_string(),
                source_root: None,
            }
        );

        assert_eq!(
            Inputs::from_args([
                "objects.toml".to_string(),
                "http://localhost:9000".to_string()
            ])
            .unwrap(),
            Inputs {
                objects_file: PathBuf::from("objects.toml"),
                grpc_url: "http://localhost:9000".to_string(),
                source_root: None,
            }
        );
    }

    #[test]
    fn accepts_explicit_source_root() {
        assert_eq!(
            Inputs::from_args([
                "objects.toml".to_string(),
                "--source-root".to_string(),
                "../nexus-next/sui".to_string(),
            ])
            .unwrap(),
            Inputs {
                objects_file: PathBuf::from("objects.toml"),
                grpc_url: DEFAULT_GRPC_URL.to_string(),
                source_root: Some(PathBuf::from("../nexus-next/sui")),
            }
        );

        assert_eq!(
            Inputs::from_args([
                "objects.toml".to_string(),
                "http://localhost:9000".to_string(),
                "--source-root".to_string(),
                "../nexus-next/sui".to_string(),
            ])
            .unwrap(),
            Inputs {
                objects_file: PathBuf::from("objects.toml"),
                grpc_url: "http://localhost:9000".to_string(),
                source_root: Some(PathBuf::from("../nexus-next/sui")),
            }
        );
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
            ("primitives".to_string(), package("0x11", "0x10", "0x20")),
            ("interface".to_string(), package("0x21", "0x20", "0x10")),
            ("talus".to_string(), package("0x31", "0x30", "0x10")),
        ];
        let mut second = vec![
            ("primitives".to_string(), package("0x111", "0x110", "0x220")),
            ("interface".to_string(), package("0x221", "0x220", "0x110")),
            ("talus".to_string(), package("0x331", "0x330", "0x110")),
        ];
        second[0].1.version = 7;
        second[1].1.version = 9;
        second[2].1.version = 11;

        canonicalize_sdk_ir(&mut first);
        canonicalize_sdk_ir(&mut second);

        assert_eq!(first, second);
        assert_eq!(first[0].1.storage_id, "0xa1");
        assert_eq!(first[0].1.original_id.as_deref(), Some("0xa1"));
        assert_eq!(first[2].1.storage_id, "0xa6");
        assert_eq!(first[2].1.original_id.as_deref(), Some("0xa6"));
        assert_eq!(
            first[0].1.modules["m"].functions[0].parameters[0].ty,
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
