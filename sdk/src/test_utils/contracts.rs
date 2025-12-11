use {
    crate::{
        idents::sui_framework,
        nexus::signer::{ExecutedTransaction, Signer},
        sui,
        test_utils::sui_mocks,
    },
    move_package::lock_file::{self, LockFile},
    std::{fs::OpenOptions, path::PathBuf, sync::Arc},
    sui_move_build::implicit_deps,
    sui_package_management::system_package_versions::latest_system_packages,
    tokio::sync::Mutex,
};

/// Publishes a Move package to Sui.
///
/// `path_str` is the path relative to the project `Cargo.toml` directory.
pub async fn publish_move_package(
    pk: &sui::crypto::Ed25519PrivateKey,
    rpc_url: &str,
    path_str: &str,
    gas_coin: sui::types::ObjectReference,
) -> ExecutedTransaction {
    let install_dir = PathBuf::from(path_str);
    let lock_file = PathBuf::from(format!("{path_str}/Move.lock"));

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

    // Compile the package.
    let mut build_config = sui_move_build::BuildConfig::new_for_testing();
    build_config.chain_id = Some(chain_id.clone());
    build_config.config.implicit_dependencies = implicit_deps(latest_system_packages());
    let package = build_config
        .build(&install_dir)
        .expect("Failed to build package.");

    let with_unpublished_deps = false;

    let mut tx = sui::tx::TransactionBuilder::new();

    let upgrade_cap = tx.publish(
        package.get_package_bytes(with_unpublished_deps),
        package
            .get_dependency_storage_package_ids()
            .iter()
            .map(|id| id.to_string().parse().unwrap())
            .collect(),
    );
    let address =
        sui_framework::Address::address_from_type(&mut tx, addr).expect("Failed to get address.");

    tx.transfer_objects(vec![upgrade_cap], address);

    tx.set_sender(addr);
    tx.set_gas_budget(1_000_000_000);
    tx.set_gas_price(reference_gas_price);
    tx.add_gas_objects(vec![sui::tx::Input::owned(
        *gas_coin.object_id(),
        gas_coin.version(),
        *gas_coin.digest(),
    )]);

    let tx = tx.finish().expect("Failed to finish transaction.");

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

    // Create the lock file if not exists.
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_file)
        .expect("Failed to create lock file.");

    let mut lock =
        LockFile::from(install_dir.clone(), &lock_file).expect("Failed to read lock file.");

    lock_file::schema::update_managed_address(
        &mut lock,
        "localnet",
        lock_file::schema::ManagedAddressUpdate::Published {
            chain_id,
            original_id: pkg_id.to_string(),
        },
    )
    .expect("Failed to update lock file.");

    lock.commit(lock_file).expect("Failed to update lock file.");

    response
}
