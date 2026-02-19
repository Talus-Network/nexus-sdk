//! Rust types mirroring Nexus interface versioning used in Move.
//!
//! On-chain, `nexus_interface::version::InterfaceVersion` is a wrapper around a `u64`.
//! This is used both in object fields and as a dynamic field key.

use {
    super::serde_parsers::{deserialize_sui_u64, serialize_sui_u64},
    serde::{Deserialize, Serialize},
};

/// Rust representation of `nexus_interface::version::InterfaceVersion`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct InterfaceVersionKey {
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub inner: u64,
}

/// Minimal projection for witness objects that store an interface version under `iv`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TapWitnessWithIv {
    pub iv: InterfaceVersionKey,
}
