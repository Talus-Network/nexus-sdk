use {
    crate::{
        move_boundary,
        nexus::signer::{ExecutedTransaction, Signer},
        sui,
        test_utils::sui_mocks,
    },
    std::{
        env,
        path::{Path, PathBuf},
        sync::{Arc, OnceLock},
    },
    tempfile::{Builder, TempDir},
    tokio::sync::Mutex,
};

fn test_artifact_temp_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-temp");
    std::fs::create_dir_all(&root).expect("Failed to create SDK test temp root");
    ensure_test_move_home(&root);
    root
}

fn ensure_test_move_home(root: &Path) {
    static MOVE_HOME: OnceLock<PathBuf> = OnceLock::new();

    let move_home = MOVE_HOME.get_or_init(|| {
        env::var_os("MOVE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let move_home = root.join(".move");
                std::fs::create_dir_all(&move_home).expect("Failed to create SDK test Move home");
                env::set_var("MOVE_HOME", &move_home);
                move_home
            })
    });
    std::fs::create_dir_all(move_home).expect("Failed to create SDK test Move home");
}

fn build_tempdir(prefix: &str) -> TempDir {
    Builder::new()
        .prefix(prefix)
        .tempdir_in(test_artifact_temp_root())
        .expect("Failed to create temporary directory in SDK test artifact root")
}

fn copy_dir_recursive(src: &Path, dest: &Path) {
    std::fs::create_dir_all(dest).expect("Failed to create destination directory");

    for entry in std::fs::read_dir(src).expect("Failed to read source directory") {
        let entry = entry.expect("Failed to read directory entry");
        let file_type = entry.file_type().expect("Failed to read entry type");
        let file_name = entry.file_name();

        if file_type.is_dir() && file_name == "build" {
            continue;
        }

        let dest_path = dest.join(&file_name);

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path);
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &dest_path).expect("Failed to copy file");
        }
    }
}

/// Publishes a Move package to Sui with no package ID overrides.
pub async fn publish_move_package(
    pk: &sui::crypto::Ed25519PrivateKey,
    rpc_url: &str,
    path_str: &str,
    gas_coin: sui::types::ObjectReference,
) -> ExecutedTransaction {
    publish_move_package_with_overrides(pk, rpc_url, path_str, gas_coin, &[]).await
}

/// Publishes a Move package to Sui. Optionally providing overrides for package
/// IDs.
///
/// `path_str` is the path relative to the project `Cargo.toml` directory.
pub async fn publish_move_package_with_overrides(
    pk: &sui::crypto::Ed25519PrivateKey,
    rpc_url: &str,
    path_str: &str,
    gas_coin: sui::types::ObjectReference,
    overrides: &[(&str, sui::types::Address)],
) -> ExecutedTransaction {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_install_dir = manifest_dir.join(path_str);
    let temp_package_root = build_tempdir("nexus-sdk-move-package-");
    let install_dir = temp_package_root.path().join(path_str);

    copy_dir_recursive(&source_install_dir, &install_dir);

    let mut client = sui::grpc::Client::new(rpc_url).expect("Could not create gRPC client");
    let addr = pk.public_key().derive_address();
    let signer = Signer::new(
        Arc::new(Mutex::new(client.clone())),
        pk.clone(),
        std::time::Duration::from_secs(30),
        Arc::new(sui_mocks::mock_nexus_objects()),
    );

    let reference_gas_price = client
        .get_reference_gas_price()
        .await
        .expect("Failed to get reference gas price.");

    let chain_id = {
        let response = client
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default())
            .await
            .expect("Failed to get service info.");

        response
            .into_inner()
            .chain_id
            .expect("Chain ID missing in service info.")
    };

    // Remove any stale Move.lock and Published.toml files.
    let _ = std::fs::remove_file(install_dir.join("Move.lock"));
    let _ = std::fs::remove_file(install_dir.join("Published.toml"));

    // Compile the package.
    let mut build_config = sui_move_build::BuildConfig::new_for_testing_replace_addresses(
        overrides
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string().parse().unwrap()))
            .collect::<Vec<_>>(),
    );

    build_config.environment =
        sui::build::Environment::new("testnet".to_string(), chain_id.clone());

    let package = build_config
        .build(&install_dir)
        .expect("Failed to build package.");

    let with_unpublished_deps = false;

    let mut ptb = sui_move_ptb::PtbBuilder::new();

    let upgrade_cap = ptb
        .publish(
            package.get_package_bytes(with_unpublished_deps),
            move_boundary::publish_dependency_ids_or_framework_defaults(
                package
                    .get_dependency_storage_package_ids()
                    .iter()
                    .map(|id| id.to_string().parse().unwrap()),
            ),
        )
        .expect("publish command should build");
    let address = ptb.arg(&addr).expect("address input should build");

    ptb.transfer_objects(vec![upgrade_cap], address)
        .expect("upgrade cap transfer should build");

    let tx = sui::types::Transaction {
        kind: sui::types::TransactionKind::ProgrammableTransaction(ptb.finish()),
        sender: addr,
        gas_payment: sui::types::GasPayment {
            objects: vec![gas_coin.clone()],
            owner: addr,
            price: reference_gas_price,
            budget: 1_000_000_000,
        },
        expiration: sui::types::TransactionExpiration::None,
    };

    let signature = signer
        .sign_tx(&tx)
        .await
        .expect("Failed to sign transaction.");

    let response = signer
        .execute_tx(tx, signature, &mut gas_coin.clone())
        .await
        .expect("Failed to execute transaction.");

    let pkg_id = response
        .objects
        .iter()
        .find_map(|c| match c.data() {
            sui::types::ObjectData::Package(m) => Some(m.id),
            _ => None,
        })
        .expect("Move package must be published");

    // Write Published.toml.
    let published_toml = format!(
        "# Generated by test infrastructure\n\
         \n\
         [published.testnet]\n\
         chain-id = \"{chain_id}\"\n\
         published-at = \"{pkg_id}\"\n\
         original-id = \"{pkg_id}\"\n\
         version = 1\n"
    );

    std::fs::write(install_dir.join("Published.toml"), published_toml)
        .expect("Failed to write Published.toml");

    response
}
