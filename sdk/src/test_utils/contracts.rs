use {
    crate::{sui, sui::traits::*},
    std::path::PathBuf,
};

/// Publishes a Move package to Sui.
///
/// [`path`] is the path relative to `Cargo.toml` directory.
pub async fn publish_move_package(
    sui: &sui::Client,
    addr: sui::Address,
    keystore: &sui::Keystore,
    path_str: &str,
    gas_coin: sui::Coin,
) -> sui::TransactionBlockResponse {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PathBuf::from(path_str));

    // Compile the package.
    let package = sui_move_build::BuildConfig::new_for_testing()
        .build(&path)
        .expect("Failed to build package.");

    let reference_gas_price = sui
        .read_api()
        .get_reference_gas_price()
        .await
        .expect("Failed to fetch reference gas price.");

    let with_unpublished_deps = false;
    let sui_tx_data = sui::TransactionData::new_module(
        addr,
        gas_coin.object_ref(),
        package.get_package_bytes(with_unpublished_deps),
        package.get_dependency_storage_package_ids(),
        sui::MIST_PER_SUI / 10,
        reference_gas_price,
    );

    let signature = keystore
        .sign_secure(&addr, &sui_tx_data, sui::Intent::sui_transaction())
        .expect("Signing TX must succeed.");

    let envelope = sui::Transaction::from_data(sui_tx_data, vec![signature]);
    let resp_options = sui::TransactionBlockResponseOptions::new()
        .with_events()
        .with_effects()
        .with_object_changes();
    let resp_finality = sui::ExecuteTransactionRequestType::WaitForLocalExecution;

    let response = sui
        .quorum_driver_api()
        .execute_transaction_block(envelope, resp_options, Some(resp_finality))
        .await
        .expect("Failed to execute transaction.");

    if let Some(effects) = response.effects.clone() {
        if effects.into_status().is_err() {
            panic!("Transaction has erroneous effects");
        }
    }

    response
}
