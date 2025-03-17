use {
    crate::{sui, sui::traits::*},
    std::path::PathBuf,
};

/// Publishes a Move package to Sui.
///
/// [`path`] is the path relative to `nexus-sdk` `Cargo.toml` directory.
pub async fn publish_move_package(
    sui: &sui::Client,
    faucet_port: u16,
    path_str: &str,
) -> sui::TransactionBlockResponse {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PathBuf::from(path_str));

    // Compile the package.
    let package = sui_move_build::BuildConfig::new_for_testing()
        .build(&path)
        .expect("Failed to build package.");

    // Use the provided mnemonic to sign the transaction.
    let (keystore, addr) = create_wallet();

    // Request some gas tokens. Assume localnet.
    super::faucet::request_tokens(addr, &format!("http://127.0.0.1:{faucet_port}/gas"))
        .await
        .expect("Failed to request tokens from faucet.");

    // Fetch the gas coin to pay with.
    let gas_coin = get_gas_coin(&sui, addr).await;
    let reference_gas_price = sui.read_api().get_reference_gas_price().await.unwrap();

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

fn create_wallet() -> (sui::Keystore, sui::Address) {
    // Generate a mnemonic.
    let derivation_path = None;
    let word_length = None;

    let (_, _, _, secret_mnemonic) =
        sui::generate_new_key(sui::SignatureScheme::ED25519, derivation_path, word_length).unwrap();

    let mut keystore = sui::Keystore::InMem(Default::default());

    let derivation_path = None;
    let alias = None;

    let addr = keystore
        .import_from_mnemonic(
            secret_mnemonic.as_str(),
            sui::SignatureScheme::ED25519,
            derivation_path,
            alias,
        )
        .expect("Importing from mnemonic must succeed.");

    (keystore, addr)
}

async fn get_gas_coin(sui: &sui::Client, addr: sui::Address) -> sui::Coin {
    let limit = None;
    let cursor = None;
    let default_to_sui_coin_type = None;

    let response = sui
        .coin_read_api()
        .get_coins(addr, default_to_sui_coin_type, cursor, limit)
        .await
        .expect("Failed to fetch gas coins.");

    response
        .data
        .iter()
        .next()
        .expect("Address must have at least one gas coin.")
        .clone()
}
