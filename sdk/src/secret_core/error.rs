use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SecretStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("base‑64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("serialization codec error: {0}")]
    Codec(Box<str>),
    #[error("cryptography failure: {0}")]
    Crypto(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("provider failure: {0}")]
    Provider(Box<str>),
}
