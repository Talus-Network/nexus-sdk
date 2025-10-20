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
    use {
        super::*,
        crate::utils::secrets::master_key::{SERVICE, USER},
        keyring::{mock, set_default_credential_builder, Entry},
        std::env,
        tempfile::TempDir,
    };

    #[tokio::test]
    #[serial_test::serial(master_key_env)]
    async fn test_crypto_generate_identity_key() {
        // Use in-memory mock keyring to avoid needing a system keychain
        set_default_credential_builder(mock::default_credential_builder());

        // Isolate XDG config so salt lives under the temp dir
        let tmp_xdg = TempDir::new().expect("temp xdg");
        env::set_var("XDG_CONFIG_HOME", tmp_xdg.path());

        // Ensure no lingering keyring entries
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());

        // Provide a passphrase-based key so we can serialize an encrypted crypto section
        env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "test-passphrase-clear-crypto");

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
