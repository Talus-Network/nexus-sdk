use {
    crate::{
        command_title,
        loading,
        notify_success,
        prelude::*,
        utils::secrets::master_key::{MasterKeyError, KEY_LEN, SERVICE, USER},
    },
    keyring::Entry,
    rand::{rngs::OsRng, RngCore},
};

/// Generate and store a new 32-byte key in the OS key-ring.
/// Important: This will also wipe any crypto configuration from the CLI configuration file.
pub async fn crypto_init_key(force: bool, conf_path: PathBuf) -> AnyResult<(), NexusCliError> {
    command_title!("Generating and storing a new 32-byte master key");

    // 1. Abort if any persistent key already exists (unless --force)
    let check_handle = loading!("Checking for existing keys...");

    if (Entry::new(SERVICE, "passphrase")
        .map_err(|e| NexusCliError::Any(e.into()))?
        .get_password()
        .is_ok()
        || Entry::new(SERVICE, USER)
            .map_err(|e| NexusCliError::Any(e.into()))?
            .get_password()
            .is_ok())
        && !force
    {
        check_handle.error();
        return Err(NexusCliError::Any(MasterKeyError::KeyAlreadyExists.into()));
    }

    check_handle.success();

    // 2. Generate and store a new 32-byte key
    let generate_handle = loading!("Generating and storing master key...");

    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);

    match Entry::new(SERVICE, USER)
        .map_err(|e| NexusCliError::Any(e.into()))?
        .set_password(&hex::encode(key))
    {
        Ok(()) => {
            generate_handle.success();
            // Remove any stale pass-phrase entry so that key-status reports the new raw key.
            let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());
            notify_success!("32-byte master key saved to the OS key-ring");
        }
        Err(e) => {
            generate_handle.error();

            return Err(NexusCliError::Any(e.into()));
        }
    };

    // 3. Remove crypto section from CLI configuration before rotating the key
    let cleanup_handle = loading!("Clearing crypto section from configuration...");

    let mut crypto_conf = CryptoConf::load_from_path(&conf_path)
        .await
        .unwrap_or_default();

    crypto_conf.identity_key = None;
    crypto_conf.sessions.clear();

    match crypto_conf.save_to_path(&conf_path).await {
        Ok(()) => {
            cleanup_handle.success();

            Ok(())
        }
        Err(e) => {
            cleanup_handle.error();

            Err(NexusCliError::Any(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        keyring::{mock, set_default_credential_builder, Entry},
        std::env,
        tempfile::TempDir,
    };

    #[tokio::test]
    #[serial_test::serial(master_key_env)]
    async fn test_crypto_init_key_clears_crypto_section() {
        // Use in-memory mock keyring to avoid needing a system keychain
        set_default_credential_builder(mock::default_credential_builder());

        let tmp = TempDir::new().expect("temp home");
        let conf_path = tmp.path().join("crypto.toml");

        // Isolate XDG config so salt lives under the temp dir
        let tmp_xdg = TempDir::new().expect("temp xdg");
        env::set_var("XDG_CONFIG_HOME", tmp_xdg.path());

        // Ensure no lingering keyring entries
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());

        // Provide a passphrase-based key so we can serialize an encrypted crypto section
        env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "test-passphrase-clear-crypto");

        // Create a config with a crypto section and persist it at ~/.nexus/conf.toml
        let crypto_conf = CryptoConf {
            identity_key: Some(Secret::new(IdentityKey::generate())),
            sessions: Default::default(),
        };

        crypto_conf
            .save_to_path(&conf_path)
            .await
            .expect("save conf with crypto");

        // Sanity: confirm crypto key exists in file
        let saved = CryptoConf::load_from_path(&conf_path)
            .await
            .expect("load conf with crypto");
        assert!(
            saved.sessions.is_empty() && saved.identity_key.is_some(),
            "crypto section should exist before rotation"
        );

        // Rotate key with --force; this should clear the crypto section first
        crypto_init_key(true, conf_path.clone())
            .await
            .expect("crypto_init_key should succeed");

        // Verify crypto section was removed but file still exists
        let cleared = CryptoConf::load_from_path(&conf_path)
            .await
            .expect("load conf after rotation");

        println!("Cleared crypto conf: {:?}", cleared);
        assert!(
            cleared.sessions.is_empty() && cleared.identity_key.is_none(),
            "crypto section should be cleared after rotation"
        );

        // Cleanup env and keyring
        env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("HOME");

        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());
    }
}
