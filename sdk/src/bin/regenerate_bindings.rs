//! Regenerate committed Move binding IR with [`sui_move_codegen`].

use {
    anyhow::{anyhow, bail, Context, Result},
    std::{
        collections::BTreeMap,
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
const NEXUS_PACKAGES: &[&str] = &[
    "primitives",
    "interface",
    "registry",
    "workflow",
    "scheduler",
];
const FRAMEWORK_PACKAGES: &[(&str, &str)] = &[("move_std", "0x1"), ("sui_framework", "0x2")];

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
    let packages = packages_from_objects_file(&inputs.objects_file)?;
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(IR_DIR);
    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir.display()))?;

    let mut client = GrpcClient::new(&inputs.grpc_url)
        .map_err(|err| anyhow!("gRPC client for {}: {err}", inputs.grpc_url))?;

    for (name, package_id) in packages {
        let mut package = fetch_package(&mut client, package_id)
            .await
            .with_context(|| format!("fetch {name} ({package_id})"))?;
        apply_source_names(&mut package, &name, inputs.source_root.as_deref())?;
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
    if NEXUS_PACKAGES.contains(&package_name) && !source_dir.is_dir() {
        bail!(
            "source directory for {package_name} does not exist: {}",
            source_dir.display()
        );
    }
    apply_function_parameter_names_from_sources(package, &source_dir)
        .with_context(|| format!("apply parameter names from {}", source_dir.display()))
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
    let mut ids = BTreeMap::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let Some(package) = key.strip_suffix("_pkg_id") else {
            continue;
        };
        let value = value.trim();
        let Some(value) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
            bail!("{source}: {key} must be a TOML string");
        };
        ids.insert(package.to_string(), value.to_string());
    }

    let mut packages = Vec::new();
    for package in NEXUS_PACKAGES {
        let id = ids
            .get(*package)
            .ok_or_else(|| anyhow!("{source} is missing {package}_pkg_id"))?;
        packages.push(((*package).to_string(), parse_address(id)?));
    }
    for (package, id) in FRAMEWORK_PACKAGES {
        packages.push(((*package).to_string(), parse_address(id)?));
    }

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
    fn parses_required_objects_and_framework_packages() {
        let objects = [
            r#"primitives_pkg_id = "0x11""#,
            r#"interface_pkg_id = "0x12""#,
            r#"registry_pkg_id = "0x13""#,
            r#"workflow_pkg_id = "0x14""#,
            r#"scheduler_pkg_id = "0x15""#,
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
                ("move_std".to_string(), Address::from_str("0x1").unwrap()),
                (
                    "sui_framework".to_string(),
                    Address::from_str("0x2").unwrap()
                ),
            ]
        );
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
