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
}

/// Obtain the process-wide master key.
///
/// First tries the OS key-ring. If not present, derives a key from
/// `NEXUS_CLI_STORE_PASSPHRASE` using Argon2id and an application-scoped
/// salt. If neither exists, generates a
/// random key and stores it in the key-ring.
pub fn get_master_key() -> Result<Zeroizing<[u8; KEY_LEN]>, MasterKeyError> {
    // Try to get key from key-ring
    // Only try keyring if no passphrase is set
    // This ensures passphrase-derived keys are always re-derived
    // rather than potentially getting a stale cached key
    if env::var("NEXUS_CLI_STORE_PASSPHRASE").is_err() {
        match Entry::new(SERVICE, USER) {
            Ok(entry) => {
                if let Ok(stored_hex) = entry.get_password() {
                    let bytes = hex::decode(&stored_hex)?;
                    if bytes.len() == KEY_LEN {
                        let key_array: [u8; KEY_LEN] = bytes.try_into().unwrap();
                        return Ok(Zeroizing::new(key_array));
                    }
                    // Invalid key in keyring, delete it
                    let _ = entry.delete_credential();
                }
            }
            Err(e) => {
                // Keyring not available, continue without it
                eprintln!("Warning: Keyring not available: {}", e);
            }
        }
    }

    // Try to derive key from passphrase
    if let Ok(passphrase) = env::var("NEXUS_CLI_STORE_PASSPHRASE") {
        let (_, salt) = get_or_create_salt()?;

        // Derive 256-bit key from passphrase + salt.
        let params = Params::new(ARGON2_MEMORY_KIB, ARGON2_ITERATIONS, 1, Some(KEY_LEN))
            .map_err(|e| MasterKeyError::Argon2(e.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        argon2
            .hash_password_into(passphrase.as_bytes(), &salt, &mut *key)
            .map_err(|e| MasterKeyError::Argon2(e.to_string()))?;

        // Don't cache passphrase-derived keys in keyring to avoid confusion
        // The passphrase + salt should always produce the same key
        return Ok(key);
    }

    // Generate new random key
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    OsRng.fill_bytes(&mut *key);

    // Try to save to keyring, but don't fail if it doesn't work
    match Entry::new(SERVICE, USER) {
        Ok(entry) => match entry.set_password(&hex::encode(&*key)) {
            Ok(_) => Ok(key),
            Err(e) => {
                eprintln!("Warning: Failed to save key to keyring: {}", e);
                Ok(key)
            }
        },
        Err(e) => {
            eprintln!("Warning: Keyring not available: {}", e);
            Ok(key)
        }
    }
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
        std::{env, fs, sync::Mutex},
        tempfile::TempDir,
    };

    /// Serialise env-var mutations to avoid cross-test interference.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce() -> R, R>(f: F) -> R {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Clean up any existing keyring entries before test
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());

        let result = f();

        // Clean up after test
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());

        result
    }

    #[test]
    #[serial_test::serial(master_key_env)]
    fn passphrase_key_is_stable_and_salt_persists() {
        with_env(|| {
            // Isolate XDG paths in a temp tree.
            let tmp = TempDir::new().unwrap();
            let xdg_path = tmp.path().join("xdg_config");
            let original_xdg = env::var("XDG_CONFIG_HOME").ok();
            env::set_var("XDG_CONFIG_HOME", &xdg_path);
            env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "correct horse battery staple");

            // Same passphrase → identical keys.
            let k1 = get_master_key().expect("first derivation");
            let k2 = get_master_key().expect("second derivation");
            assert_eq!(&*k1, &*k2, "key must be deterministic");

            // A salt must have been created.
            let salt_path = xdg_path.join("nexus-cli").join("salt.bin");
            assert!(salt_path.exists(), "salt file missing");

            // Verify salt content is correct size
            let salt_content = fs::read(&salt_path).unwrap();
            assert_eq!(salt_content.len(), SALT_LEN, "salt file has wrong size");

            // On Unix, salt must be 0o600.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&salt_path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600, "salt file not private");
            }

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

            assert_ne!(&*k1, &*k2, "distinct passphrases must yield distinct keys");

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
    fn random_key_is_persisted_in_keyring() {
        with_env(|| {
            // Ensure the passphrase path is disabled.
            env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");

            let first = get_master_key().expect("random key generated");

            // For random keys, we try to use the keyring
            // If keyring works, keys should match
            // If keyring fails, we can't guarantee persistence
            let second = get_master_key().expect("second call");

            // Try to check if keyring is working by attempting to use it directly
            match Entry::new(SERVICE, USER) {
                Ok(entry) => {
                    match entry.get_password() {
                        Ok(_) => {
                            // Keyring is working, keys should match
                            assert_eq!(
                                &*first, &*second,
                                "subsequent calls should return cached key from key-ring"
                            );
                        }
                        Err(_) => {
                            // Keyring not working, skip assertion
                            eprintln!("Note: Keyring not available, skipping persistence check");
                        }
                    }
                }
                Err(_) => {
                    // Keyring not available at all
                    eprintln!("Note: Keyring service not available, skipping persistence check");
                }
            }
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

            // Get key and force salt creation
            let k1 = get_master_key().unwrap();

            // Read the salt that was created
            let salt_path = xdg_path.join("nexus-cli").join("salt.bin");
            let salt1 = fs::read(&salt_path).unwrap();

            // Simulate new process by getting key again
            let k2 = get_master_key().unwrap();
            let salt2 = fs::read(&salt_path).unwrap();

            // Salt should not change
            assert_eq!(salt1, salt2, "salt should persist across calls");
            assert_eq!(&*k1, &*k2, "keys should be identical with same salt");

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
