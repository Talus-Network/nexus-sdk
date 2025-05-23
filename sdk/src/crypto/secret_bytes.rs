use {
    serde::{Deserialize, Serialize},
    x25519_dalek::StaticSecret,
    zeroize::{Zeroize, ZeroizeOnDrop},
};

/// A helper struct to serialize and zeroize a 32-byte secret scalar.
/// Use it to serialize StaticSecret.
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct SecretBytes(#[serde(with = "serde_bytes")] pub [u8; 32]);

impl From<&StaticSecret> for SecretBytes {
    fn from(sk: &StaticSecret) -> Self {
        Self(sk.to_bytes())
    }
}
impl From<SecretBytes> for StaticSecret {
    fn from(raw: SecretBytes) -> Self {
        StaticSecret::from(raw.0)
    }
}
