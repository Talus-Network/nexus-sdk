//! Transaction signers that keep key material outside the process.

#[cfg(feature = "gcp-kms")]
pub mod gcp_kms;
#[cfg(feature = "remote-signer")]
pub mod remote;

#[cfg(feature = "gcp-kms")]
pub use gcp_kms::GcpKmsSigner;
#[cfg(feature = "remote-signer")]
pub use remote::{RemoteSignatureScheme, RemoteSigner, RemoteSignerConfig};
