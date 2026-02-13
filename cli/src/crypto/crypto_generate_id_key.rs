use crate::{command_title, loading, notify_success, prelude::*};

/// Generate and store a fresh identity key in the Nexus CLI configuration.
/// WARNING: This will invalidate all existing sessions!
pub(crate) async fn crypto_generate_identity_key(
    conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Generating a fresh identity key");

    let conf_handle = loading!("Generating identity key...");

    // Generate a fresh identity key.
    match CryptoConf::set_identity_key(IdentityKey::generate(), Some(&conf_path)).await {
        Ok(()) => {
            conf_handle.success();

            notify_success!("Identity key generated successfully");
            notify_success!("All existing sessions have been invalidated");

            Ok(())
        }
        Err(e) => {
            conf_handle.error();

            Err(NexusCliError::Any(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::secrets::store::master_key, keyring::Entry, tempfile::TempDir};

    #[tokio::test]
    #[serial_test::serial(master_key_env)]
    async fn test_crypto_generate_identity_key() {
        master_key::test_keyring::reset();

        // Ensure no lingering keyring entries
        let _ =
            Entry::new(master_key::SERVICE, master_key::USER).and_then(|e| e.delete_credential());
        // Seed a key so the generated identity key is stored encrypted.
        Entry::new(master_key::SERVICE, master_key::USER)
            .expect("create keyring entry")
            .set_password(&"11".repeat(master_key::KEY_LEN))
            .expect("seed keyring key");

        let tmp = TempDir::new().expect("temp home");
        let conf_path = tmp.path().join("crypto.toml");

        // Generate identity key
        crypto_generate_identity_key(conf_path.clone())
            .await
            .expect("crypto_generate_identity_key should succeed");

        // Load config and verify identity key exists
        let _ik = CryptoConf::get_identity_key(Some(&conf_path))
            .await
            .expect("should load ik");
    }
}
