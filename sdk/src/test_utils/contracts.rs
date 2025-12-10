use {
    crate::{
        nexus::signer::{ExecutedTransaction, Signer},
        sui::{self, SuiTransactionBlockEffectsAPI},
        test_utils::sui_mocks,
    },
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

    let client = sui::grpc::Client::new(rpc_url).expect("Could not create gRPC client");
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
    build_config.chain_id = Some(chain_id);
    build_config.config.implicit_dependencies = implicit_deps(latest_system_packages());
    let package = build_config
        .build(&install_dir)
        .expect("Failed to build package.");

    let with_unpublished_deps = false;

    let mut tx = sui::tx::TransactionBuilder::new();

    tx.publish(
        package.get_package_bytes(with_unpublished_deps),
        package
            .get_dependency_storage_package_ids()
            .iter()
            .map(|id| id.to_string().parse().unwrap())
            .collect(),
    );

    tx.set_sender(addr);
    tx.set_gas_budget(1_000_000_000);
    tx.set_gas_price(reference_gas_price);
    tx.add_gas_objects(vec![sui::types::Input::ImmutableOrOwned(gas_coin)]);

    tx.finish().expect("Failed to finish transaction.");

    let signature = signer.sign_tx(tx);
    let response = signer
        .execute_tx(tx, signature, &mut gas_coin.clone())
        .await
        .expect("Failed to execute transaction.");

    // Create the lock file if not exists.
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_file)
        .expect("Failed to create lock file.");

    sui_package_management::update_lock_file(
        wallet,
        sui_package_management::LockCommand::Publish,
        Some(install_dir),
        Some(lock_file),
        &response,
    )
    .await
    .expect("Failed to update lock file.");

    response
}
