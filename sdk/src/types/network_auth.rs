//! Public SDK helpers for generated `nexus_registry::network_auth` Move types.

#[cfg(test)]
use super::{generated_support::sui_address_to_uid, MoveOption};
pub use crate::types::generated::registry_types::network_auth::{
    IdentityKey,
    KeyBinding,
    KeyRecord,
    NetworkAuth,
};
use {
    super::generated_support::MoveString,
    crate::{sui, types::generated::sui_framework_types::object::ID},
};

impl IdentityKey {
    /// Construct a generated Move `IdentityKey::Leader` value.
    pub fn leader(leader_cap_id: sui::types::Address) -> Self {
        Self::Leader {
            leader_cap_id: ID {
                bytes: leader_cap_id,
            },
        }
    }

    /// Construct a generated Move `IdentityKey::Tool` value from a tool FQN string.
    pub fn tool_fqn(fqn: &str) -> Self {
        Self::Tool {
            fqn: MoveString {
                bytes: fqn.as_bytes().to_vec(),
            },
        }
    }

    /// Leader capability object ID for leader identities.
    pub fn leader_cap_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::Leader { leader_cap_id } => Some(leader_cap_id.bytes),
            Self::Tool { .. } => None,
        }
    }

    /// Tool FQN string for tool identities.
    pub fn tool_fqn_string(&self) -> Option<String> {
        match self {
            Self::Leader { .. } => None,
            Self::Tool { fqn } => Some(String::from(fqn.clone())),
        }
    }
}

impl KeyBinding {
    pub fn object_id(&self) -> sui::types::Address {
        self.id.id.bytes
    }

    pub fn description(&self) -> Option<&[u8]> {
        self.description.0.as_deref()
    }

    pub fn active_key_id(&self) -> Option<u64> {
        self.active_key_id.0
    }

    pub fn key_table_id(&self) -> sui::types::Address {
        self.keys.id()
    }

    pub fn key_table_size(&self) -> usize {
        self.keys.size()
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        id: sui::types::Address,
        identity: IdentityKey,
        description: Option<Vec<u8>>,
        next_key_id: u64,
        active_key_id: Option<u64>,
        keys: super::MoveTable<u64, KeyRecord>,
    ) -> Self {
        Self {
            id: sui_address_to_uid(id),
            identity,
            description: MoveOption(description),
            next_key_id,
            active_key_id: MoveOption(active_key_id),
            keys,
        }
    }
}

impl KeyRecord {
    pub fn revoked_at_ms(&self) -> Option<u64> {
        self.revoked_at_ms.0
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        scheme: u8,
        public_key: Vec<u8>,
        added_at_ms: u64,
        revoked_at_ms: Option<u64>,
    ) -> Self {
        Self {
            scheme,
            public_key,
            added_at_ms,
            revoked_at_ms: MoveOption(revoked_at_ms),
        }
    }
}

impl NetworkAuth {
    pub fn object_id(&self) -> sui::types::Address {
        self.id.id.bytes
    }

    pub fn leader_cap_ids(&self) -> impl Iterator<Item = sui::types::Address> + '_ {
        self.identities
            .contents
            .iter()
            .filter_map(IdentityKey::leader_cap_id)
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(id: sui::types::Address, identities: Vec<IdentityKey>) -> Self {
        Self {
            id: sui_address_to_uid(id),
            identities: super::MoveVecSet {
                contents: identities,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, bcs};

    #[test]
    fn identity_key_helpers_use_generated_move_shape() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);

        let leader = IdentityKey::leader(address);
        assert_eq!(leader.leader_cap_id(), Some(address));
        assert_eq!(leader.tool_fqn_string(), None);

        let tool = IdentityKey::tool_fqn("xyz.demo.tool@1");
        assert_eq!(tool.leader_cap_id(), None);
        assert_eq!(tool.tool_fqn_string().as_deref(), Some("xyz.demo.tool@1"));
    }

    #[test]
    fn identity_key_bcs_roundtrip() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);
        let key = IdentityKey::leader(address);
        let bytes = bcs::to_bytes(&key).unwrap();
        let decoded: IdentityKey = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, key);
        assert_eq!(decoded.leader_cap_id(), Some(address));
    }

    #[test]
    fn key_binding_bcs_roundtrip() {
        let mut rng = rand::thread_rng();
        let active_key_id = Some(4);
        let binding = KeyBinding::new_for_test(
            sui::types::Address::generate(&mut rng),
            IdentityKey::leader(sui::types::Address::generate(&mut rng)),
            Some(b"nexus".to_vec()),
            7,
            active_key_id,
            super::super::MoveTable::new(sui::types::Address::generate(&mut rng), 2),
        );

        let bytes = bcs::to_bytes(&binding).unwrap();
        let decoded: KeyBinding = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.object_id(), binding.object_id());
        assert_eq!(decoded.key_table_id(), binding.key_table_id());
        assert_eq!(decoded.key_table_size(), 2);
        assert_eq!(decoded.next_key_id, binding.next_key_id);
        assert_eq!(decoded.active_key_id(), active_key_id);
        assert_eq!(decoded.description(), Some(b"nexus".as_slice()));
    }

    #[test]
    fn key_record_and_network_auth_helpers_use_generated_move_shape() {
        let mut rng = rand::thread_rng();
        let id = sui::types::Address::generate(&mut rng);
        let first_leader = sui::types::Address::generate(&mut rng);
        let second_leader = sui::types::Address::generate(&mut rng);
        let record = KeyRecord::new_for_test(1, vec![1, 2, 3], 10, Some(20));

        assert_eq!(record.revoked_at_ms(), Some(20));
        assert_eq!(
            NetworkAuth::new_for_test(
                id,
                vec![
                    IdentityKey::leader(first_leader),
                    IdentityKey::tool_fqn("xyz.demo.tool@1"),
                    IdentityKey::leader(second_leader),
                ],
            )
            .leader_cap_ids()
            .collect::<Vec<_>>(),
            vec![first_leader, second_leader]
        );
        assert_eq!(NetworkAuth::new_for_test(id, vec![]).object_id(), id);
    }
}
