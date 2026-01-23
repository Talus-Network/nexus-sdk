//! Rust projections of `nexus_workflow::network_auth` on-chain types.

use {
    crate::{
        nexus::crawler::DynamicMap,
        sui,
        types::{
            deserialize_encoded_bytes,
            deserialize_sui_option_u64,
            deserialize_sui_u64,
            serialize_encoded_bytes,
            serialize_sui_option_u64,
            serialize_sui_u64,
        },
    },
    serde::{Deserialize, Serialize},
};

/// Move `std::ascii::String` (a wrapper around `vector<u8>`).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MoveAsciiString {
    pub bytes: Vec<u8>,
}

impl MoveAsciiString {
    /// Construct from a Rust string.
    pub fn from_str(s: &str) -> Self {
        Self {
            bytes: s.as_bytes().to_vec(),
        }
    }

    /// Attempt to interpret the bytes as UTF-8.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes).to_string()
    }
}

/// Move `nexus_workflow::network_auth::IdentityKey`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum IdentityKey {
    /// `IdentityKey::Leader { address }`
    Leader { address: sui::types::Address },
    /// `IdentityKey::Tool { fqn }`
    Tool { fqn: MoveAsciiString },
}

impl IdentityKey {
    /// Construct a leader identity key.
    pub fn leader(address: sui::types::Address) -> Self {
        Self::Leader { address }
    }

    /// Construct a tool identity key from a tool FQN string.
    pub fn tool_fqn(fqn: &str) -> Self {
        Self::Tool {
            fqn: MoveAsciiString::from_str(fqn),
        }
    }
}

/// Move `nexus_workflow::network_auth::KeyRecord`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KeyRecord {
    pub scheme: u8,
    #[serde(
        deserialize_with = "deserialize_encoded_bytes",
        serialize_with = "serialize_encoded_bytes"
    )]
    pub public_key: Vec<u8>,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub added_at_ms: u64,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub revoked_at_ms: Option<u64>,
}

/// Minimal projection of `nexus_workflow::network_auth::KeyBinding`.
#[derive(Clone, Debug, Deserialize)]
pub struct KeyBinding {
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_sui_u64")]
    pub next_key_id: u64,
    #[serde(deserialize_with = "deserialize_sui_option_u64")]
    pub active_key_id: Option<u64>,
    /// Dynamic fields backing the on-chain `Table<u64, KeyRecord>`.
    pub keys: DynamicMap<u64, KeyRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_ascii_string_roundtrip() {
        let value = MoveAsciiString::from_str("nexus");
        assert_eq!(value.bytes, b"nexus".to_vec());
        assert_eq!(value.to_string_lossy(), "nexus");
    }

    #[test]
    fn identity_key_helpers() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);

        let leader = IdentityKey::leader(address);
        assert_eq!(leader, IdentityKey::Leader { address });

        let tool = IdentityKey::tool_fqn("xyz.demo.tool@1");
        assert_eq!(
            tool,
            IdentityKey::Tool {
                fqn: MoveAsciiString::from_str("xyz.demo.tool@1")
            }
        );
    }
}
