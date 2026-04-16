//! Rust types mirroring Nexus interface versioning used in Move.
//!
//! Onchain, `nexus_interface::version::InterfaceVersion` is a wrapper around a `u64`.
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

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn interface_version_key_serde_roundtrip() {
        let key = InterfaceVersionKey { inner: 42 };
        let value = serde_json::to_value(&key).unwrap();
        assert_eq!(value, json!({"inner": "42"}));

        let from_str: InterfaceVersionKey = serde_json::from_value(json!({"inner": "43"})).unwrap();
        assert_eq!(from_str.inner, 43);

        let from_num: InterfaceVersionKey = serde_json::from_value(json!({"inner": 44})).unwrap();
        assert_eq!(from_num.inner, 44);
    }

    #[test]
    fn tap_witness_with_iv_deserializes() {
        let witness: TapWitnessWithIv =
            serde_json::from_value(json!({"iv": {"inner": "7"}})).unwrap();
        assert_eq!(witness.iv.inner, 7);
    }
}
