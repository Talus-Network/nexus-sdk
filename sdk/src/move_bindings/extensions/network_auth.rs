//! SDK projections for generated network auth Move types.
//!
//! The key shapes are [`crate::move_bindings::registry::network_auth::IdentityKey`],
//! [`crate::move_bindings::registry::network_auth::KeyBinding`],
//! [`crate::move_bindings::registry::network_auth::KeyRecord`], and
//! [`crate::move_bindings::registry::network_auth::NetworkAuth`]. This module adds constructors
//! and accessors used by SDK services while preserving the enum variants, table metadata, and
//! optional fields generated from normalized package IR.

#[cfg(test)]
use crate::move_bindings::{
    move_std::option::Option as MoveOption,
    sui_framework::{table::Table as MoveTable, vec_set::VecSet},
};
use crate::{
    move_bindings::{
        registry::network_auth::{IdentityKey, KeyBinding, KeyRecord, NetworkAuth},
        sui_framework::object::ID,
    },
    sui,
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

    /// Construct a generated Move `IdentityKey::Tool` value from its stable Tool object ID.
    pub fn tool(tool_id: sui::types::Address) -> Self {
        Self::Tool {
            tool_id: ID { bytes: tool_id },
        }
    }

    /// Leader capability object ID for leader identities.
    pub fn leader_cap_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::Leader { leader_cap_id } => Some(leader_cap_id.bytes),
            Self::Tool { .. } => None,
        }
    }

    /// Stable Tool object ID for tool identities.
    pub fn tool_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::Leader { .. } => None,
            Self::Tool { tool_id } => Some(tool_id.bytes),
        }
    }
}

impl KeyBinding {
    pub fn object_id(&self) -> sui::types::Address {
        self.id.id.bytes
    }

    pub fn description(&self) -> Option<&[u8]> {
        self.description.as_option().map(Vec::as_slice)
    }

    pub fn active_key_id(&self) -> Option<u64> {
        self.active_key_id.copied_option()
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
        keys: MoveTable<u64, KeyRecord>,
    ) -> Self {
        Self {
            id: crate::move_bindings::sui_framework::object::UID::new(id),
            identity,
            description: MoveOption::from_option(description).into(),
            next_key_id,
            active_key_id: MoveOption::from_option(active_key_id).into(),
            keys,
        }
    }
}

impl KeyRecord {
    pub fn revoked_at_ms(&self) -> Option<u64> {
        self.revoked_at_ms.copied_option()
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
            revoked_at_ms: MoveOption::from_option(revoked_at_ms).into(),
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
            id: crate::move_bindings::sui_framework::object::UID::new(id),
            identities: VecSet {
                contents: identities,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, bcs};

    #[test]
    fn identity_key_helpers_use_move_shape() {
        let mut rng = rand::thread_rng();
        let address = sui::types::Address::generate(&mut rng);

        let leader = IdentityKey::leader(address);
        assert_eq!(leader.leader_cap_id(), Some(address));
        assert_eq!(leader.tool_id(), None);

        let tool = IdentityKey::tool(address);
        assert_eq!(tool.leader_cap_id(), None);
        assert_eq!(tool.tool_id(), Some(address));
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
            MoveTable::new(sui::types::Address::generate(&mut rng), 2),
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
    fn key_record_and_network_auth_helpers_use_move_shape() {
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
                    IdentityKey::tool(id),
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
