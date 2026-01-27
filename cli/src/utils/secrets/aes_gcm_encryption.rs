use {
    super::master_key_provider::MasterKeyProvider,
    aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm,
    },
    nexus_sdk::secret_core::{
        error::SecretStoreError,
        traits::{EncryptionAlgo, KeyProvider},
    },
    zeroize::Zeroizing,
};

const KEY_LEN: usize = 32; // 256-bit AES key

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AesGcmEncryption;

impl EncryptionAlgo for AesGcmEncryption {
    const NONCE_LEN: usize = 12;

    fn encrypt(nonce: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
        let key: Zeroizing<[u8; KEY_LEN]> = MasterKeyProvider.key()?;
        let cipher =
            Aes256Gcm::new_from_slice(&*key).map_err(|e| SecretStoreError::Crypto(Box::new(e)))?;
        let nonce_array: [u8; 12] = nonce
            .try_into()
            .map_err(|_| SecretStoreError::Crypto("Invalid nonce length".into()))?;
        cipher
            .encrypt(&nonce_array.into(), plaintext)
            .map_err(|e| SecretStoreError::Crypto(Box::new(e)))
    }

    fn decrypt(nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, SecretStoreError> {
        let key: Zeroizing<[u8; KEY_LEN]> = MasterKeyProvider.key()?;
        let cipher =
            Aes256Gcm::new_from_slice(&*key).map_err(|e| SecretStoreError::Crypto(Box::new(e)))?;
        let nonce_array: [u8; 12] = nonce
            .try_into()
            .map_err(|_| SecretStoreError::Crypto("Invalid nonce length".into()))?;
        cipher
            .decrypt(&nonce_array.into(), ciphertext)
            .map_err(|e| SecretStoreError::Crypto(Box::new(e)))
    }
}
