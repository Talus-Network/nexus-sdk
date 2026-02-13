//! CLI master key handling.
//!
//! The CLI optionally encrypts locally persisted secret state (identity key, sessions, etc.) using
//! a 32‑byte master key stored in the OS keyring.
//!
//! # Storage format
//! - Where: OS keyring entry `(SERVICE, USER)` (see constants below).
//! - What: a hex string encoding of 32 random bytes (AES‑256 key).

use {
    keyring::Entry,
    nexus_sdk::secret_store,
    rand::{rngs::OsRng, RngCore},
    thiserror::Error,
};

/// Service / user names for the OS key-ring.
pub const SERVICE: &str = "nexus-cli-store";
pub const USER: &str = "master-key";

pub const KEY_LEN: usize = secret_store::KEY_LEN;

#[derive(Debug, Error)]
pub enum MasterKeyError {
    /// OS keyring errors (unavailable backend, access denied, etc.).
    #[error("key-ring error: {0}")]
    Keyring(#[from] keyring::Error),
    /// Stored key is not valid hex.
    #[error("hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),
    /// Stored key decoded but did not have the expected length.
    #[error("invalid master key length (expected {expected} bytes, got {got})")]
    InvalidKeyLength { expected: usize, got: usize },
    /// No key is present in the keyring.
    #[cfg(test)]
    #[error("no master key found; run `nexus secrets enable` to enable encryption")]
    NoPersistentKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureMasterKey {
    /// A new key was created and stored.
    Created,
    /// A key already existed; nothing was changed.
    AlreadyExists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMasterKey {
    /// The key entry existed and was deleted.
    Deleted,
    /// No key entry existed.
    NotFound,
}

/// Load the master key from the OS keyring.
///
/// Returns `Ok(None)` if no key entry exists.
pub fn load_master_key() -> Result<Option<secret_store::SecretKey>, MasterKeyError> {
    let hex_key = match Entry::new(SERVICE, USER)?.get_password() {
        Ok(v) => v,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(MasterKeyError::Keyring(e)),
    };

    let bytes = hex::decode(&hex_key)?;
    if bytes.len() != KEY_LEN {
        return Err(MasterKeyError::InvalidKeyLength {
            expected: KEY_LEN,
            got: bytes.len(),
        });
    }

    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&bytes);
    Ok(Some(secret_store::SecretKey::new(key)))
}

#[cfg(test)]
pub fn require_master_key() -> Result<secret_store::SecretKey, MasterKeyError> {
    load_master_key()?.ok_or(MasterKeyError::NoPersistentKey)
}

/// Ensure a master key exists, creating it if missing.
///
/// This is used by `nexus secrets enable` and by the runtime policy in `auto` mode when the CLI is
/// about to persist a secret.
pub fn ensure_master_key_exists() -> Result<EnsureMasterKey, MasterKeyError> {
    let entry = Entry::new(SERVICE, USER)?;

    match entry.get_password() {
        Ok(_) => return Ok(EnsureMasterKey::AlreadyExists),
        Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(MasterKeyError::Keyring(e)),
    }

    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);
    entry.set_password(&hex::encode(key))?;

    Ok(EnsureMasterKey::Created)
}

pub fn delete_master_key() -> Result<DeleteMasterKey, MasterKeyError> {
    let entry = Entry::new(SERVICE, USER)?;
    match entry.delete_credential() {
        Ok(()) => Ok(DeleteMasterKey::Deleted),
        Err(keyring::Error::NoEntry) => Ok(DeleteMasterKey::NotFound),
        Err(e) => Err(MasterKeyError::Keyring(e)),
    }
}

#[cfg(test)]
pub(crate) mod test_keyring {
    use {
        keyring::{
            credential::{
                Credential,
                CredentialApi,
                CredentialBuilder,
                CredentialBuilderApi,
                CredentialPersistence,
            },
            error::Error,
            set_default_credential_builder,
            Result,
        },
        std::{
            any::Any,
            collections::HashMap,
            sync::{Mutex, Once, OnceLock},
        },
    };

    type Key = (Option<String>, String, String);

    static STORE: OnceLock<Mutex<HashMap<Key, Vec<u8>>>> = OnceLock::new();
    fn store() -> &'static Mutex<HashMap<Key, Vec<u8>>> {
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    #[derive(Debug, Clone)]
    struct PersistentCredential {
        target: Option<String>,
        service: String,
        user: String,
    }

    impl PersistentCredential {
        fn key(&self) -> Key {
            (self.target.clone(), self.service.clone(), self.user.clone())
        }
    }

    impl CredentialApi for PersistentCredential {
        fn set_secret(&self, secret: &[u8]) -> Result<()> {
            store()
                .lock()
                .expect("Can't access mock store for set")
                .insert(self.key(), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self) -> Result<Vec<u8>> {
            store()
                .lock()
                .expect("Can't access mock store for get")
                .get(&self.key())
                .cloned()
                .ok_or(Error::NoEntry)
        }

        fn delete_credential(&self) -> Result<()> {
            let mut guard = store().lock().expect("Can't access mock store for delete");
            match guard.remove(&self.key()) {
                Some(_) => Ok(()),
                None => Err(Error::NoEntry),
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn debug_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(self, f)
        }
    }

    #[derive(Debug, Default)]
    struct PersistentBuilder;
    impl CredentialBuilderApi for PersistentBuilder {
        fn build(
            &self,
            target: Option<&str>,
            service: &str,
            user: &str,
        ) -> Result<Box<Credential>> {
            let cred: Box<dyn CredentialApi + Send + Sync> = Box::new(PersistentCredential {
                target: target.map(ToOwned::to_owned),
                service: service.to_owned(),
                user: user.to_owned(),
            });
            Ok(cred)
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn persistence(&self) -> CredentialPersistence {
            CredentialPersistence::ProcessOnly
        }
    }

    static INIT: Once = Once::new();

    pub(crate) fn install() {
        INIT.call_once(|| {
            let builder: Box<dyn CredentialBuilderApi + Send + Sync> = Box::new(PersistentBuilder);
            set_default_credential_builder(builder as Box<CredentialBuilder>);
        });
    }

    pub(crate) fn reset() {
        install();
        store()
            .lock()
            .expect("Can't access mock store for reset")
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serial_test::serial};

    #[test]
    #[serial(master_key_env)]
    fn load_master_key_none_when_missing() {
        test_keyring::reset();
        let _ = Entry::new(SERVICE, USER).and_then(|e| e.delete_credential());

        let key = load_master_key().unwrap();
        assert!(key.is_none());
    }
}
