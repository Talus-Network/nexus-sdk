//! Transaction signers that keep key material outside the process.

#[cfg(feature = "remote-signer")]
pub mod remote;

#[cfg(feature = "remote-signer")]
pub use remote::{RemoteSignatureScheme, RemoteSigner, RemoteSignerConfig};
