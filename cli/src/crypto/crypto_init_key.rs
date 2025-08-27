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
pub async fn crypto_init_key(force: bool) -> AnyResult<(), NexusCliError> {
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

    // 2. Remove crypto section from CLI configuration before rotating the key
    let cleanup_handle = loading!("Clearing crypto section from configuration...");
    let conf_path = match expand_tilde(CLI_CONF_PATH) {
        Ok(p) => p,
        Err(e) => {
            cleanup_handle.error();
            return Err(NexusCliError::Any(e));
        }
    };
    match tokio::fs::try_exists(&conf_path).await {
        Ok(true) => {
            let content = match tokio::fs::read_to_string(&conf_path).await {
                Ok(c) => c,
                Err(e) => {
                    cleanup_handle.error();
                    return Err(NexusCliError::Io(e));
                }
            };

            // Parse the TOML generically so we can remove `crypto` without needing to decrypt it
            let mut value: toml::Value = match toml::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    cleanup_handle.error();
                    return Err(NexusCliError::Any(anyhow!(e)));
                }
            };

            if let Some(table) = value.as_table_mut() {
                table.remove("crypto");
            }

            let serialized = match toml::to_string_pretty(&value) {
                Ok(s) => s,
                Err(e) => {
                    cleanup_handle.error();
                    return Err(NexusCliError::Any(anyhow!(e)));
                }
            };
            if let Err(e) = tokio::fs::write(&conf_path, serialized).await {
                cleanup_handle.error();
                return Err(NexusCliError::Io(e));
            }
        }
        Ok(false) => {
            // No config file yet; nothing to clear
        }
        Err(e) => {
            cleanup_handle.error();
            return Err(NexusCliError::Any(anyhow!(
                "Failed to check existence of CLI configuration: {e}"
            )));
        }
    }

    cleanup_handle.success();
    // 3. Generate and store a new 32-byte key
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
            Ok(())
        }
        Err(e) => {
            generate_handle.error();
            Err(NexusCliError::Any(e.into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        keyring::{mock, set_default_credential_builder, Entry},
        std::{env, fs},
        tempfile::TempDir,
    };

    #[tokio::test]
    #[serial_test::serial(master_key_env)]
    async fn test_crypto_init_key_clears_crypto_section() {
        // Use in-memory mock keyring to avoid needing a system keychain
        set_default_credential_builder(mock::default_credential_builder());

        // Isolate HOME so that ~/.nexus/conf.toml resolves into a temp dir
        let tmp_home = TempDir::new().expect("temp home");
        env::set_var("HOME", tmp_home.path());

        // Isolate XDG config so salt lives under the temp dir
        let tmp_xdg = TempDir::new().expect("temp xdg");
        env::set_var("XDG_CONFIG_HOME", tmp_xdg.path());

        // Ensure no lingering keyring entries
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());

        // Provide a passphrase-based key so we can serialize an encrypted crypto section
        env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "test-passphrase-clear-crypto");

        // Create a config with a crypto section and persist it at ~/.nexus/conf.toml
        let mut conf = CliConf::default();
        conf.crypto = Some(Secret::new(CryptoConf {
            identity_key: Some(IdentityKey::generate()),
            sessions: Default::default(),
        }));
        conf.save().await.expect("save conf with crypto");

        // Sanity: confirm crypto key exists in file
        let conf_path = expand_tilde(CLI_CONF_PATH).expect("expand path");
        let content_before = fs::read_to_string(&conf_path).expect("read conf before");
        assert!(
            content_before.contains("crypto"),
            "crypto section should exist before rotation"
        );

        // Rotate key with --force; this should clear the crypto section first
        crypto_init_key(true)
            .await
            .expect("crypto_init_key should succeed");

        // Verify crypto section was removed but file still exists
        let content_after = fs::read_to_string(&conf_path).expect("read conf after");
        let parsed: toml::Value = toml::from_str(&content_after).expect("parse toml after");
        assert!(
            parsed.as_table().and_then(|t| t.get("crypto")).is_none(),
            "crypto section should be removed after rotation"
        );

        // Cleanup env and keyring
        env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        env::remove_var("XDG_CONFIG_HOME");
        env::remove_var("HOME");
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());
    }
}
