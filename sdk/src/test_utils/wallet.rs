//! Test utils for wallet management. Create and import mnemonics.

use crate::{sui, sui::traits::AccountKeystore};

/// Create a test wallet with a random mnemonic.
pub fn create_test_wallet() -> anyhow::Result<(sui::Keystore, sui::Address)> {
    let derivation_path = None;
    let word_length = None;

    let (_, _, _, secret_mnemonic) =
        sui::generate_new_key(sui::SignatureScheme::ED25519, derivation_path, word_length)?;

    import_wallet(secret_mnemonic.as_str())
}

/// Import a wallet from the provided secret mnemonic.
pub fn import_wallet(secret_mnemonic: &str) -> anyhow::Result<(sui::Keystore, sui::Address)> {
    let mut keystore = sui::Keystore::InMem(Default::default());

    let derivation_path = None;
    let alias = None;

    let addr = keystore.import_from_mnemonic(
        secret_mnemonic,
        sui::SignatureScheme::ED25519,
        derivation_path,
        alias,
    )?;

    Ok((keystore, addr))
}
