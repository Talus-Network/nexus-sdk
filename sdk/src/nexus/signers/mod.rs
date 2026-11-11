//! Transaction signers that keep key material outside the process.

pub mod gcp_kms;

pub use gcp_kms::GcpKmsSigner;
/// The credentials crate `GcpKmsSigner::with_endpoint` takes its credentials from,
/// re-exported so callers build them with the exact same version.
pub use google_cloud_auth;
