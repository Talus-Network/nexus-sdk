use {
    super::master_key::{self, KEY_LEN},
    nexus_sdk::secret_core::{error::SecretStoreError, traits::KeyProvider},
    zeroize::Zeroizing,
};

/// Wraps the existing `master_key::get_master_key()` logic.
#[derive(Default, Debug, Clone, Copy)]
pub struct MasterKeyProvider;

impl KeyProvider for MasterKeyProvider {
    type Key = Zeroizing<[u8; KEY_LEN]>;

    fn key(&self) -> Result<Self::Key, SecretStoreError> {
        master_key::get_master_key().map_err(|e| SecretStoreError::Provider(e.to_string()))
    }
}
