//! Rust projections of `nexus_workflow::network_auth` on-chain types.

use {
    super::{MoveTable, MoveVecSet},
    crate::sui,
    serde::{Deserialize, Serialize},
};

/// Move `nexus_workflow::network_auth::IdentityKey`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum IdentityKey {
    /// `IdentityKey::Leader { leader_cap_id }`
    Leader {
        /// ID of the leader's `leader_cap::OverNetwork` capability object.
        leader_cap_id: sui::types::Address,
    },
    /// `IdentityKey::Tool { fqn }`
    Tool {
        /// Fully qualified tool name.
        fqn: String,
    },
}

impl IdentityKey {
    /// Construct a leader identity key.
    pub fn leader(leader_cap_id: sui::types::Address) -> Self {
        Self::Leader { leader_cap_id }
    }

    /// Construct a tool identity key from a tool FQN string.
    pub fn tool_fqn(fqn: &str) -> Self {
        Self::Tool {
            fqn: fqn.to_string(),
        }
    }
}

/// Move `nexus_workflow::network_auth::NetworkAuth`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct NetworkAuth {
    pub id: sui::types::Address,
    pub identities: MoveVecSet<IdentityKey>,
}

/// Move `nexus_workflow::network_auth::KeyRecord`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct KeyRecord {
    pub scheme: u8,
    pub public_key: Vec<u8>,
    pub added_at_ms: u64,
    pub revoked_at_ms: Option<u64>,
}

/// Move `nexus_workflow::network_auth::KeyBinding`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyBinding {
    pub id: sui::types::Address,
    pub identity: IdentityKey,
    pub description: Option<Vec<u8>>,
    pub next_key_id: u64,
    pub active_key_id: Option<u64>,
    pub keys: MoveTable<u64, KeyRecord>,
}

#[cfg(test)]
mod tests {
    use {super::*, bcs};

    #[test]
    fn identity_key_helpers() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);

        let leader = IdentityKey::leader(address);
        assert_eq!(
            leader,
            IdentityKey::Leader {
                leader_cap_id: address
            }
        );

        let tool = IdentityKey::tool_fqn("xyz.demo.tool@1");
        assert_eq!(
            tool,
            IdentityKey::Tool {
                fqn: "xyz.demo.tool@1".to_string()
            }
        );
    }

    #[test]
    fn identity_key_bcs_roundtrip() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);
        let key = IdentityKey::leader(address);
        let bytes = bcs::to_bytes(&key).unwrap();
        let decoded: IdentityKey = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn key_binding_bcs_roundtrip() {
        let mut rng = rand::thread_rng();
        let binding = KeyBinding {
            id: sui::types::Address::generate(&mut rng),
            identity: IdentityKey::leader(sui::types::Address::generate(&mut rng)),
            description: Some(b"nexus".to_vec()),
            next_key_id: 7,
            active_key_id: Some(4),
            keys: MoveTable::new(sui::types::Address::generate(&mut rng), 2),
        };

        let bytes = bcs::to_bytes(&binding).unwrap();
        let decoded: KeyBinding = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.next_key_id, binding.next_key_id);
        assert_eq!(decoded.active_key_id, binding.active_key_id);
    }
}
