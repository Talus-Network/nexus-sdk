//! Regenerate committed Move binding IR with [`sui_move_codegen`].

use {
    anyhow::{anyhow, bail, Context, Result},
    std::{
        collections::BTreeMap,
        env, fs,
        path::{Path, PathBuf},
        str::FromStr,
    },
    sui_move_codegen::{fetch_package, ir::NormalizedPackage, GrpcClient},
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
        let package = fetch_package(&mut client, package_id)
            .await
            .with_context(|| format!("fetch {name} ({package_id})"))?;
        let module_count = package.modules.len();
        let path = write_package_ir(&out_dir, &name, &package)?;
        println!("wrote {} ({} modules)", path.display(), module_count);
    }

    Ok(())
}

impl Inputs {
    fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let args: Vec<_> = args.into_iter().collect();
        match args.as_slice() {
            [objects_file] => Ok(Self {
                objects_file: PathBuf::from(objects_file),
                grpc_url: DEFAULT_GRPC_URL.to_string(),
            }),
            [objects_file, grpc_url] => Ok(Self {
                objects_file: PathBuf::from(objects_file),
                grpc_url: grpc_url.to_string(),
            }),
            _ => bail!("expected: regenerate_bindings <objects_toml> [grpc_url]"),
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
            Datatype, DatatypeKind, Function, FunctionParam, NormalizedModule, TypeName, TypeRef,
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
            }
        );
    }

    #[test]
    fn writes_ir_without_changing_addresses_modules_or_functions() {
        let out_dir = tempfile_dir();
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

        let path = write_package_ir(&out_dir, "primitives", &package).expect("write IR");
        let written = fs::read_to_string(path).expect("read IR");
        let decoded = NormalizedPackage::from_json_str(&written).expect("decode IR");

        assert_eq!(decoded.storage_id, "0x111");
        assert_eq!(decoded.original_id.as_deref(), Some("0x222"));
        assert!(decoded
            .modules
            .contains_key("module_that_sdk_used_to_filter"));
        assert_eq!(decoded.modules["m"].functions[0].name, "keep_me");
    }

    fn tempfile_dir() -> PathBuf {
        let path = env::temp_dir().join(format!(
            "nexus-sdk-regenerate-bindings-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }
}
