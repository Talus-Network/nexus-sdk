//! Test utils for wallet management. Create and import mnemonics.

use crate::{sui, sui::traits::AccountKeystore};

/// Create an ephemeral wallet context that can be loaded.
pub fn create_ephemeral_wallet_context(
    sui_port: u16,
) -> anyhow::Result<(sui::WalletContext, String)> {
    let derivation_path = None;
    let word_length = None;

    let (addr, _, _, sui_secret_mnemonic) =
        sui::generate_new_key(sui::SignatureScheme::ED25519, derivation_path, word_length)
            .expect("Failed to generate key.");

    let config = format!(
        "\
keystore:
  InMem:
    aliases: {}
    keys: {}
envs:
  - alias: test
    rpc: http://127.0.0.1:{sui_port}
    ws: ~
    basic_auth: ~
active_env: test
active_address: \"{addr}\"",
        "{}", "{}"
    );

    // Create sui config in tmp and load it.
    let dir = tempfile::tempdir().map_err(|e| anyhow::anyhow!(e))?;
    let path = dir.path().join("config.yaml");

    std::fs::write(&path, config).map_err(|e| anyhow::anyhow!(e))?;

    let request_timeout = None;
    let max_concurrent_requests = None;

    let mut context = sui::WalletContext::new(&path, request_timeout, max_concurrent_requests)?;

    let derivation_path = None;
    let alias = None;

    context.config.keystore.import_from_mnemonic(
        sui_secret_mnemonic.as_str(),
        sui::SignatureScheme::ED25519,
        derivation_path,
        alias,
    )?;

    Ok((context, sui_secret_mnemonic))
}

/// Sign and execute the provided transaction.
// TODO: remove
#[allow(unused_variables)]
pub async fn sign_and_execute_tx(
    wallet: &mut sui::WalletContext,
    tx: sui::ProgrammableTransaction,
    gas_coin: sui::ObjectID,
    gas_budget: u64,
) -> anyhow::Result<sui::TransactionBlockResponse> {
    todo!()
}
