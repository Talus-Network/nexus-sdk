//! Minimal master-key support for `Secret<T>`
//!
//! Generates or recovers a 256-bit AES key, storing it in the
//! OS key-ring when possible, or deriving it from the optional
//! `NEXUS_CLI_STORE_PASSPHRASE` environment variable with Argon2id.

use {
    argon2::{Algorithm, Argon2, Params, Version},
    directories::ProjectDirs,
    keyring::Entry,
    rand::{rngs::OsRng, RngCore},
    std::{env, fs, io, path::PathBuf},
    thiserror::Error,
    zeroize::Zeroizing,
};

// === constants ===

/// Service / user names for the OS key-ring.
pub const SERVICE: &str = "nexus-cli-store";
pub const USER: &str = "master-key";

/// 256-bit master-key length.
pub const KEY_LEN: usize = 32;
/// 128-bit salt for Argon2id passphrase derivation.
pub const SALT_LEN: usize = 16;

/// Argon2id default parameters (64 MiB, 4 passes, single thread).
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_ITERATIONS: u32 = 4;

// === error type ===

#[derive(Debug, Error)]
pub enum MasterKeyError {
    #[error("key-ring error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("unable to locate a per-user configuration directory")]
    ProjectDirNotFound,
    #[error("argon2 failure: {0}")]
    Argon2(String),
    #[error(
        "no persistent master key found; \
            run `nexus-cli crypto init-key` or `set-passphrase`"
    )]
    NoPersistentKey,
    #[error(
        "a different persistent key already exists; \
            re-run with --force if you really want to replace it"
    )]
    KeyAlreadyExists,
}

/// Obtain the process-wide master key.
///
/// Resolution order (abort if none succeed):
/// 1. `NEXUS_CLI_STORE_PASSPHRASE` env-var -> Argon2id(pass, salt)
/// 2. key-ring entry  {service=SERVICE, user="passphrase"} -> Argon2id(pass, salt)
/// 3. key-ring entry  {service=SERVICE, user=USER} -> raw 32-byte key
pub fn get_master_key() -> Result<Zeroizing<[u8; KEY_LEN]>, MasterKeyError> {
    // 1. ENV-VAR branch (highest priority)
    if let Ok(passphrase) = env::var("NEXUS_CLI_STORE_PASSPHRASE") {
        return derive_from_passphrase(&passphrase);
    }

    // 2. Key-ring pass-phrase branch
    if let Ok(passphrase) = Entry::new(SERVICE, "passphrase").and_then(|e| e.get_password()) {
        return derive_from_passphrase(&passphrase);
    }

    // 3. Raw 32-byte key stored in key-ring
    if let Ok(hex_key) = Entry::new(SERVICE, USER).and_then(|e| e.get_password()) {
        let bytes = hex::decode(&hex_key)?;
        if bytes.len() == KEY_LEN {
            let key_array: [u8; KEY_LEN] = bytes.try_into().unwrap();
            return Ok(Zeroizing::new(key_array));
        }
        // Corrupt entry – clean up to avoid surprises next run
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
    }

    // 4. Nothing worked -> hard error
    Err(MasterKeyError::NoPersistentKey)
}

fn derive_from_passphrase(passphrase: &str) -> Result<Zeroizing<[u8; KEY_LEN]>, MasterKeyError> {
    let (_, salt) = get_or_create_salt()?;
    let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_ITERATIONS, 1, Some(KEY_LEN))
        .map_err(|e| MasterKeyError::Argon2(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(passphrase.as_bytes(), &salt, &mut *key)
        .map_err(|e| MasterKeyError::Argon2(e.to_string()))?;
    Ok(key)
}

/// Locate `$XDG_CONFIG_HOME/nexus-cli/salt.bin` or platform-specific config dir,
/// creating both the directory and the salt (with 0600 perms) on first run.
fn get_or_create_salt() -> Result<(PathBuf, [u8; SALT_LEN]), MasterKeyError> {
    // For tests, prefer XDG_CONFIG_HOME if set
    let config_dir = if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("nexus-cli")
    } else {
        // Fall back to ProjectDirs for production use
        ProjectDirs::from("com", "nexus", "nexus-cli")
            .ok_or(MasterKeyError::ProjectDirNotFound)?
            .config_dir()
            .to_path_buf()
    };

    let salt_path = config_dir.join("salt.bin");

    if salt_path.exists() {
        let bytes = fs::read(&salt_path)?;
        if bytes.len() == SALT_LEN {
            let salt_array: [u8; SALT_LEN] = bytes.try_into().unwrap();
            return Ok((salt_path, salt_array));
        }
        // Invalid salt file, recreate it
        eprintln!("Warning: Invalid salt file, recreating...");
        let _ = fs::remove_file(&salt_path);
    }

    // First run: create parent dir + fresh salt.
    fs::create_dir_all(&config_dir)?;
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    write_salt_securely(&salt_path, &salt)?;
    Ok((salt_path, salt))
}

/// Write salt to path with owner-only (0o600) permissions.
fn write_salt_securely(path: &PathBuf, salt: &[u8; SALT_LEN]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?
            .write_all(salt)?;
    }
    #[cfg(not(unix))]
    {
        // Fallback: rely on platform defaults.
        fs::write(path, salt)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        keyring::{mock, set_default_credential_builder, Entry},
        std::{
            env,
            fs,
            sync::{Mutex, Once},
        },
        tempfile::TempDir,
    };

    /// Serialise env‑var mutations to avoid cross‑test interference.
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    /// Ensure the mock key‑ring is installed exactly once for the whole test run.
    static INIT_KEYRING: Once = Once::new();

    fn init_mock_keyring() {
        INIT_KEYRING.call_once(|| {
            // Replace the default credential store with the in‑memory mock.
            set_default_credential_builder(mock::default_credential_builder());
        });
    }

    /// Helper that acquires the env‑lock, cleans up key‑ring state, executes the
    /// test closure, and finally cleans up again. This keeps individual tests
    /// run in order.
    fn with_env<F: FnOnce() -> R, R>(f: F) -> R {
        init_mock_keyring();

        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Pre‑test clean‑up.
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());

        // Execute the actual test body.
        let result = f();

        // Post‑test clean‑up.
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());
        let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());

        result
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn no_key_available_fails_with_hard_error() {
        with_env(|| {
            env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");

            // Should fail with NoPersistentKey error
            let result = get_master_key();
            assert!(result.is_err(), "should fail when no key is available");
            match result.unwrap_err() {
                MasterKeyError::NoPersistentKey => {}
                other => panic!("expected NoPersistentKey error, got: {:?}", other),
            }
        });
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn env_var_passphrase_is_used_when_provided() {
        with_env(|| {
            let tmp = TempDir::new().unwrap();
            let xdg_path = tmp.path().join("xdg_config");
            let original_xdg = env::var("XDG_CONFIG_HOME").ok();
            env::set_var("XDG_CONFIG_HOME", &xdg_path);

            env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "my secret passphrase");

            let key = get_master_key().expect("should derive key from env var");
            assert_eq!(key.len(), KEY_LEN);

            let key2 = get_master_key().expect("second call should work");
            assert_eq!(&*key, &*key2);

            // Cleanup
            if let Some(old) = original_xdg {
                env::set_var("XDG_CONFIG_HOME", old);
            } else {
                env::remove_var("XDG_CONFIG_HOME");
            }
            env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        });
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn passphrase_key_is_stable_and_salt_persists() {
        with_env(|| {
            let tmp = TempDir::new().unwrap();
            let xdg_path = tmp.path().join("xdg_config");
            let original_xdg = env::var("XDG_CONFIG_HOME").ok();
            env::set_var("XDG_CONFIG_HOME", &xdg_path);
            env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "correct horse battery staple");

            let k1 = get_master_key().expect("first derivation");
            let k2 = get_master_key().expect("second derivation");
            assert_eq!(&*k1, &*k2);

            let salt_path = xdg_path.join("nexus-cli").join("salt.bin");
            assert!(salt_path.exists());
            let salt_content = fs::read(&salt_path).unwrap();
            assert_eq!(salt_content.len(), SALT_LEN);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&salt_path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600, "salt file not private");
            }

            if let Some(old) = original_xdg {
                env::set_var("XDG_CONFIG_HOME", old);
            } else {
                env::remove_var("XDG_CONFIG_HOME");
            }
            env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        });
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn different_passphrases_produce_different_keys() {
        with_env(|| {
            let tmp = TempDir::new().unwrap();
            let xdg_path = tmp.path().join("xdg_config");
            let original_xdg = env::var("XDG_CONFIG_HOME").ok();
            env::set_var("XDG_CONFIG_HOME", &xdg_path);

            env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "alpha");
            let k1 = get_master_key().unwrap();

            env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "bravo");
            let k2 = get_master_key().unwrap();

            assert_ne!(&*k1, &*k2);

            if let Some(old) = original_xdg {
                env::set_var("XDG_CONFIG_HOME", old);
            } else {
                env::remove_var("XDG_CONFIG_HOME");
            }
            env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        });
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn salt_persistence_across_processes() {
        with_env(|| {
            let tmp = TempDir::new().unwrap();
            let xdg_path = tmp.path().join("xdg_config");
            let original_xdg = env::var("XDG_CONFIG_HOME").ok();
            env::set_var("XDG_CONFIG_HOME", &xdg_path);
            env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "test passphrase");

            let k1 = get_master_key().unwrap();

            let salt_path = xdg_path.join("nexus-cli").join("salt.bin");
            let salt1 = fs::read(&salt_path).unwrap();

            let k2 = get_master_key().unwrap();
            let salt2 = fs::read(&salt_path).unwrap();
            // Check that the salt is the same across processes
            assert_eq!(salt1, salt2);
            assert_eq!(&*k1, &*k2);
            // Cleanup
            if let Some(old) = original_xdg {
                env::set_var("XDG_CONFIG_HOME", old);
            } else {
                env::remove_var("XDG_CONFIG_HOME");
            }
            env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        });
    }
}
