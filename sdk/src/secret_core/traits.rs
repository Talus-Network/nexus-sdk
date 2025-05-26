use rand::{rngs::OsRng, RngCore};
use serde::{de::DeserializeOwned, Serialize};

use super::error::SecretStoreError;

/// Helper to fill random bytes.
#[inline]
pub fn random_bytes(buf: &mut [u8]) { OsRng.fill_bytes(buf); }

// Key material

/// A source of key material.
/// For key‑less algorithms you can just implement `key()` to return `()`.
pub trait KeyProvider: Default + Send + Sync + 'static {
    type Key: Send + Sync + 'static;
    fn key(&self) -> Result<Self::Key, SecretStoreError>;
}

/// Provider for algorithms that need no secret key.
#[derive(Default, Debug, Clone, Copy)]
pub struct NullKeyProvider;
impl KeyProvider for NullKeyProvider {
    type Key = (); fn key(&self) -> Result<Self::Key, SecretStoreError> { Ok(()) }
}

// Codec
pub trait PlaintextCodec: Default + Send + Sync + 'static {
    fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, SecretStoreError>;
    fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SecretStoreError>;
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BincodeCodec;
impl PlaintextCodec for BincodeCodec {
    fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, SecretStoreError> {
        bincode::serialize(value).map_err(|e| SecretStoreError::Codec(e.to_string()))
    }
    fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SecretStoreError> {
        bincode::deserialize(bytes).map_err(|e| SecretStoreError::Codec(e.to_string()))
    }
}

// Encryption scheme
pub trait EncryptionAlgo: Default + Send + Sync + 'static {
    /// Size in bytes of the nonce.  Use `0` if deterministic / nonce‑less.
    const NONCE_LEN: usize;

    fn encrypt(
        nonce: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, SecretStoreError>;

    fn decrypt(
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, SecretStoreError>;
}

#[derive(Default, Debug, Clone, Copy)]
pub struct EncryptionAlgoDefault;
impl EncryptionAlgo for EncryptionAlgoDefault {
    const NONCE_LEN: usize = 0;
    fn encrypt(_: &[u8], _: &[u8]) -> Result<Vec<u8>, SecretStoreError> { 
        Err(SecretStoreError::Crypto("no algo".into())) 
    }
    fn decrypt(_: &[u8], _: &[u8]) -> Result<Vec<u8>, SecretStoreError> { 
        Err(SecretStoreError::Crypto("no algo".into())) 
    }
}