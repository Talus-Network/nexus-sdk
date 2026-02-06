//! Minimal, reusable at-rest secret wrapper for config persistence.
//!
//! This module is designed for storing small secrets (keys, tokens, sessions, etc.) in
//! human-editable config formats (TOML/JSON/YAML) without sprinkling encryption logic throughout
//! your data model.
//!
//! The core type is [`StoredSecret`]:
//! - It serializes as a single string, so it can be embedded inside other structs.
//! - It encodes the plaintext value via a [`Codec`] (default: [`BincodeCodec`]).
//! - If a [`KeyProvider`] yields a 32‑byte key, it encrypts using AES‑256‑GCM.
//! - If no key is available, it stores the encoded bytes as plaintext (still encoded).
//!
//! # Format (v1)
//! `StoredSecret<T>` encodes as one of:
//!
//! - `plain:v1:<base64(pt)>`
//! - `enc:v1:<base64(nonce || ct)>`
//!
//! Where:
//! - `nonce` is 12 random bytes (AES‑GCM nonce).
//! - `ct` is the AES‑GCM output (ciphertext + 16‑byte authentication tag).
//!
//! There is no AAD and no key id recorded in the envelope. This keeps the primitive tiny,
//! but means context‑binding and rotation semantics are owned by the caller.
//!
//! # Keying model
//! [`KeyProvider::key`] returns:
//! - `Ok(Some(key))`: encrypt on write; decrypt on read
//! - `Ok(None)`: write plaintext; reject encrypted payloads as undecryptable
//!
//! When deserializing an `enc:v1:` value and the key is unavailable, deserialization fails with
//! [`SecretStoreError::KeyUnavailable`].
//!
//! # Threat model (what this helps with)
//! - Protects secrets if the config file is leaked while
//!   the encryption key remains unavailable to the attacker.
//! - Detects tampering of encrypted blobs via AEAD authentication.
//!
//! # In‑memory considerations
//! `StoredSecret<T>` is an at‑rest wrapper. It stores `T` in memory and does not automatically
//! zeroize `T` on drop. If you need in‑memory hygiene, wrap sensitive bytes in `zeroize` types
//! (e.g. [`zeroize::Zeroizing`]) or use a dedicated redacted/zeroized type for `T`.
//!
//! # Example
//! ```rust,no_run
//! # use nexus_sdk::secret_store::{KeyProvider, SecretKey, SecretStoreError, StoredSecret, KEY_LEN};
//! # use serde::{Deserialize, Serialize};
//! # use zeroize::Zeroizing;
//! #
//! #[derive(Default, Clone, Copy)]
//! struct StaticKey;
//! impl KeyProvider for StaticKey {
//!     fn key(&self) -> Result<Option<SecretKey>, SecretStoreError> {
//!         Ok(Some(Zeroizing::new([7u8; KEY_LEN])))
//!     }
//! }
//!
//! type EncSecret<T> = StoredSecret<T, StaticKey>;
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct Conf {
//!     api_token: EncSecret<String>,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let conf = Conf {
//!         api_token: EncSecret::new("shh".to_owned()),
//!     };
//!
//!     // Use any serde format. `bincode` is available when the `secret_store` feature is enabled.
//!     let bytes = bincode::serialize(&conf)?;
//!     let roundtrip: Conf = bincode::deserialize(&bytes)?;
//!     assert_eq!(roundtrip.api_token.into_inner(), "shh");
//!     Ok(())
//! }
//! ```

use {
    aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm,
    },
    base64::{engine::general_purpose, Engine as _},
    rand::{rngs::OsRng, RngCore},
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer},
    std::{
        fmt,
        marker::PhantomData,
        ops::{Deref, DerefMut},
    },
    thiserror::Error,
    zeroize::Zeroizing,
};

/// AES‑256 key length in bytes.
pub const KEY_LEN: usize = 32;
/// AES‑GCM nonce length in bytes.
pub const NONCE_LEN: usize = 12;

const ENC_PREFIX: &str = "enc:v1:";
const PLAIN_PREFIX: &str = "plain:v1:";

pub type SecretKey = Zeroizing<[u8; KEY_LEN]>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SecretStoreError {
    /// Failed to decode base64 in the serialized representation.
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    /// Failed to encode/decode the value using the selected [`Codec`].
    #[error("codec error: {0}")]
    Codec(String),
    /// Cryptographic failure (unexpected in normal operation).
    #[error("cryptography failure: {0}")]
    Crypto(String),
    /// The serialized value is encrypted, but no key is available to decrypt it.
    #[error("encryption key unavailable")]
    KeyUnavailable,
    /// The configured key provider returned an operational error.
    #[error("key provider failure: {0}")]
    Provider(String),
}

/// Encodes/decodes the plaintext value to bytes.
///
/// This indirection exists so the envelope can stay a single string while the inner type remains
/// strongly typed. The default codec is [`BincodeCodec`].
pub trait Codec: Default + Send + Sync + 'static {
    /// Encode `value` into a byte vector.
    fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, SecretStoreError>;
    /// Decode a `T` from bytes produced by [`Codec::encode`].
    fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SecretStoreError>;
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BincodeCodec;
impl Codec for BincodeCodec {
    fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, SecretStoreError> {
        bincode::serialize(value).map_err(|e| SecretStoreError::Codec(e.to_string()))
    }

    fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SecretStoreError> {
        bincode::deserialize(bytes).map_err(|e| SecretStoreError::Codec(e.to_string()))
    }
}

/// Supplies an optional encryption key.
///
/// - `Ok(Some(key))` => encrypt/decrypt
/// - `Ok(None)` => store plaintext / reject encrypted payloads as undecryptable
///
/// ## Why `Default`?
/// `StoredSecret` implements `Serialize`/`Deserialize` without holding a provider value. Requiring
/// `Default` makes it possible to obtain a provider instance on demand.
///
/// If you need runtime configuration, use a provider that reads from process‑global state (env,
/// statics) or wrap configuration in the value being serialized (higher-level choice).
pub trait KeyProvider: Default + Send + Sync + 'static {
    /// Return the current key. Returning `None` opts into plaintext storage.
    fn key(&self) -> Result<Option<SecretKey>, SecretStoreError>;
}

/// Provider that always yields no key (plaintext-only).
#[derive(Default, Debug, Clone, Copy)]
pub struct NoKey;
impl KeyProvider for NoKey {
    fn key(&self) -> Result<Option<SecretKey>, SecretStoreError> {
        Ok(None)
    }
}

/// At‑rest secret wrapper that serializes as a single string.
///
/// Notes:
/// - [`fmt::Debug`] and [`fmt::Display`] are redacted to reduce accidental leakage.
/// - This type implements [`Deref`] and [`DerefMut`] for ergonomic access, which also means you
///   can still leak secrets by explicitly formatting the inner value. Treat `T` as sensitive.
#[derive(Clone)]
pub struct StoredSecret<T, K: KeyProvider = NoKey, C: Codec = BincodeCodec> {
    value: T,
    _key: PhantomData<K>,
    _codec: PhantomData<C>,
}

impl<T: PartialEq, K: KeyProvider, C: Codec> PartialEq for StoredSecret<T, K, C> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: Eq, K: KeyProvider, C: Codec> Eq for StoredSecret<T, K, C> {}

impl<T, K: KeyProvider, C: Codec> StoredSecret<T, K, C> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            _key: PhantomData,
            _codec: PhantomData,
        }
    }

    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T: Default, K: KeyProvider, C: Codec> Default for StoredSecret<T, K, C> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T, K: KeyProvider, C: Codec> From<T> for StoredSecret<T, K, C> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T, K: KeyProvider, C: Codec> Deref for StoredSecret<T, K, C> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T, K: KeyProvider, C: Codec> DerefMut for StoredSecret<T, K, C> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T, K: KeyProvider, C: Codec> fmt::Debug for StoredSecret<T, K, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StoredSecret([Redacted])")
    }
}

impl<T, K: KeyProvider, C: Codec> fmt::Display for StoredSecret<T, K, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[Redacted]")
    }
}

impl<T, K, C> Serialize for StoredSecret<T, K, C>
where
    T: Serialize,
    K: KeyProvider,
    C: Codec,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let pt = Zeroizing::new(C::encode(&self.value).map_err(serde::ser::Error::custom)?);

        let key_opt = K::default().key().map_err(serde::ser::Error::custom)?;
        let encoded = match key_opt {
            Some(key) => {
                let mut nonce = [0u8; NONCE_LEN];
                OsRng.fill_bytes(&mut nonce);

                let ct = encrypt(&key, &nonce, &pt).map_err(serde::ser::Error::custom)?;

                let mut buf = Vec::with_capacity(NONCE_LEN + ct.len());
                buf.extend_from_slice(&nonce);
                buf.extend_from_slice(&ct);

                format!("{}{}", ENC_PREFIX, general_purpose::STANDARD.encode(buf))
            }
            None => format!("{}{}", PLAIN_PREFIX, general_purpose::STANDARD.encode(&*pt)),
        };

        serializer.serialize_str(&encoded)
    }
}

impl<'de, T, K, C> Deserialize<'de> for StoredSecret<T, K, C>
where
    T: DeserializeOwned,
    K: KeyProvider,
    C: Codec,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if let Some(rest) = s.strip_prefix(PLAIN_PREFIX) {
            let pt = Zeroizing::new(
                general_purpose::STANDARD
                    .decode(rest)
                    .map_err(serde::de::Error::custom)?,
            );
            let value: T = C::decode(&pt).map_err(serde::de::Error::custom)?;
            return Ok(Self::new(value));
        }

        if let Some(rest) = s.strip_prefix(ENC_PREFIX) {
            let decoded = general_purpose::STANDARD
                .decode(rest)
                .map_err(serde::de::Error::custom)?;

            if decoded.len() < NONCE_LEN {
                return Err(serde::de::Error::custom("ciphertext too short"));
            }

            let (nonce_bytes, ct) = decoded.split_at(NONCE_LEN);
            let nonce: [u8; NONCE_LEN] = nonce_bytes
                .try_into()
                .map_err(|_| serde::de::Error::custom("invalid nonce length"))?;

            let key_opt = K::default().key().map_err(serde::de::Error::custom)?;
            let key = key_opt
                .ok_or_else(|| serde::de::Error::custom(SecretStoreError::KeyUnavailable))?;

            let pt = Zeroizing::new(decrypt(&key, &nonce, ct).map_err(serde::de::Error::custom)?);
            let value: T = C::decode(&pt).map_err(serde::de::Error::custom)?;
            return Ok(Self::new(value));
        }

        Err(serde::de::Error::custom("unknown secret encoding"))
    }
}

fn encrypt(
    key: &SecretKey,
    nonce: &[u8; NONCE_LEN],
    pt: &[u8],
) -> Result<Vec<u8>, SecretStoreError> {
    let cipher =
        Aes256Gcm::new_from_slice(&**key).map_err(|e| SecretStoreError::Crypto(e.to_string()))?;
    cipher
        .encrypt(aes_gcm::Nonce::from_slice(nonce), pt)
        .map_err(|e| SecretStoreError::Crypto(e.to_string()))
}

fn decrypt(
    key: &SecretKey,
    nonce: &[u8; NONCE_LEN],
    ct: &[u8],
) -> Result<Vec<u8>, SecretStoreError> {
    let cipher =
        Aes256Gcm::new_from_slice(&**key).map_err(|e| SecretStoreError::Crypto(e.to_string()))?;
    cipher
        .decrypt(aes_gcm::Nonce::from_slice(nonce), ct)
        .map_err(|e| SecretStoreError::Crypto(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Foo {
        a: u32,
        b: String,
    }

    #[derive(Default, Debug, Clone, Copy)]
    struct StaticKey;
    impl KeyProvider for StaticKey {
        fn key(&self) -> Result<Option<SecretKey>, SecretStoreError> {
            Ok(Some(Zeroizing::new([7u8; KEY_LEN])))
        }
    }

    type EncSecretFoo = StoredSecret<Foo, StaticKey, BincodeCodec>;
    type PlainSecretFoo = StoredSecret<Foo, NoKey, BincodeCodec>;

    #[test]
    fn plaintext_roundtrip() {
        let secret = PlainSecretFoo::new(Foo {
            a: 1,
            b: "x".into(),
        });

        let s = serde_json::to_string(&secret).unwrap();
        assert!(s.contains(PLAIN_PREFIX));

        let decoded: PlainSecretFoo = serde_json::from_str(&s).unwrap();
        assert_eq!(
            decoded.into_inner(),
            Foo {
                a: 1,
                b: "x".into()
            }
        );
    }

    #[test]
    fn encrypted_roundtrip_and_nonce_randomization() {
        let secret = EncSecretFoo::new(Foo {
            a: 9,
            b: "abc".into(),
        });

        let s1 = serde_json::to_string(&secret).unwrap();
        let s2 = serde_json::to_string(&secret).unwrap();
        assert!(s1.contains(ENC_PREFIX));
        assert_ne!(s1, s2, "nonce should randomize ciphertext");

        let d1: EncSecretFoo = serde_json::from_str(&s1).unwrap();
        let d2: EncSecretFoo = serde_json::from_str(&s2).unwrap();
        assert_eq!(d1.into_inner(), d2.into_inner());
    }
}
