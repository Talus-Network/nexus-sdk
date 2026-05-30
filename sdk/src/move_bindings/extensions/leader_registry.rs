//! SDK projections for generated leader registry object data.
//!
//! The primary shapes are [`crate::move_bindings::registry::leader::LeaderRegistry`] and
//! [`crate::move_bindings::registry::leader::CapabilityManger`]. These helpers expose decoded
//! addresses and budget values while keeping BCS bytes and generated fields as the source of
//! truth.

#[cfg(test)]
use crate::move_bindings::{
    move_std::{option::Option as MoveOption, type_name::TypeName},
    sui_framework::{table::Table as MoveTable, vec_set::VecSet as MoveVecSet},
};
#[cfg(test)]
use crate::move_bindings::{primitives, registry};
use {
    crate::{
        move_bindings::registry::leader::{CapabilityManger, LeaderRegistry},
        sui,
    },
    anyhow::Context,
};

#[cfg(test)]
type LeaderCapIssuer = primitives::owner_cap::CloneableOwnerCap<registry::leader_cap::OverNetwork>;

impl LeaderRegistry {
    /// Decode a published `LeaderRegistry` object.
    pub fn from_object(object: &sui::types::Object) -> anyhow::Result<Self> {
        let contents = object
            .as_struct()
            .context("leader registry object is not a Move struct")?
            .contents();

        Self::from_bcs(contents)
    }

    /// Decode `LeaderRegistry` object contents from BCS bytes.
    pub fn from_bcs(contents: &[u8]) -> anyhow::Result<Self> {
        bcs::from_bytes(contents).context("failed to decode leader registry object")
    }

    /// Network ID used by all leader capabilities issued by this registry.
    pub fn network_id(&self) -> sui::types::Address {
        self.capabilities.network_id.bytes
    }

    /// Maximum budget (in MIST) a leader may spend on a single transaction.
    pub fn max_transaction_budget(&self) -> u64 {
        self.max_transaction_budget
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(id: sui::types::Address, network: sui::types::Address) -> Self {
        Self {
            id: crate::move_bindings::sui_framework::object::UID::new(id),
            unbonding_duration_ms: 0,
            min_stake_us: 0,
            max_transaction_budget: 10_000_000_000,
            leaders: MoveVecSet { contents: vec![] },
            records: MoveTable::new(sui::types::Address::ZERO, 0),
            capabilities: CapabilityManger {
                allowed_addresses: MoveVecSet { contents: vec![] },
                network_id: crate::move_bindings::sui_framework::object::ID::new(network),
                admin_cap_id: crate::move_bindings::sui_framework::object::ID::new(
                    sui::types::Address::ZERO,
                ),
                leader_cap_issuer: leader_cap_issuer_for_test(
                    sui::types::Address::ZERO,
                    id,
                    sui::types::Address::ZERO,
                ),
            },
            workflow_witness_type: MoveOption::from_option(None),
        }
    }
}

impl CapabilityManger {
    /// Network ID used by all issued `leader_cap::OverNetwork` objects.
    pub fn network_id(&self) -> sui::types::Address {
        self.network_id.bytes
    }
}

#[cfg(test)]
fn leader_cap_issuer_from_parts(
    id: sui::types::Address,
    what_for: sui::types::Address,
    unique: sui::types::Address,
) -> LeaderCapIssuer {
    LeaderCapIssuer {
        id: crate::move_bindings::sui_framework::object::UID::new(id),
        what_for: crate::move_bindings::sui_framework::object::ID::new(what_for),
        inner: primitives::owner_cap::OwnerCap {
            unique: crate::move_bindings::sui_framework::object::ID::new(unique),
            phantom_t0: std::marker::PhantomData,
        },
        phantom_t0: std::marker::PhantomData,
    }
}

#[cfg(test)]
fn leader_cap_issuer_for_test(
    id: sui::types::Address,
    what_for: sui::types::Address,
    unique: sui::types::Address,
) -> LeaderCapIssuer {
    leader_cap_issuer_from_parts(id, what_for, unique)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round trip the empty [`LeaderRegistry::new_for_test`] payload: encode as BCS, decode via
    /// [`LeaderRegistry::from_bcs`], then encode again and assert the byte stream is identical
    /// to the input.
    #[test]
    fn from_bcs_round_trips_empty_registry_payload() {
        let registry_id = sui::types::Address::from_static("0x1");
        let network_id = sui::types::Address::from_static("0x2");
        let original = LeaderRegistry::new_for_test(registry_id, network_id);
        let bytes = bcs::to_bytes(&original).expect("encode test registry");

        let decoded = LeaderRegistry::from_bcs(&bytes).expect("decode round-trips");
        let re_encoded = bcs::to_bytes(&decoded).expect("re-encode decoded registry");

        assert_eq!(
            bytes, re_encoded,
            "BCS round-trip must be byte-identical for the empty registry"
        );
        assert_eq!(decoded.network_id(), network_id);
        assert_eq!(decoded.max_transaction_budget(), 10_000_000_000);
    }

    #[test]
    fn from_bcs_round_trips_populated_registry_payload() {
        let registry_id = sui::types::Address::from_static("0x10");
        let network_id = sui::types::Address::from_static("0x20");
        let leader_a = sui::types::Address::from_static("0x30");
        let leader_b = sui::types::Address::from_static("0x31");
        let table_id = sui::types::Address::from_static("0x40");
        let admin_cap_id = sui::types::Address::from_static("0x50");
        let issuer_id = sui::types::Address::from_static("0x60");
        let issuer_inner = sui::types::Address::from_static("0x61");

        let original = LeaderRegistry {
            id: crate::move_bindings::sui_framework::object::UID::new(registry_id),
            unbonding_duration_ms: 5_000,
            min_stake_us: 1_000_000,
            max_transaction_budget: 7_500_000_000,
            leaders: MoveVecSet {
                contents: vec![
                    crate::move_bindings::sui_framework::object::ID::new(leader_a),
                    crate::move_bindings::sui_framework::object::ID::new(leader_b),
                ],
            },
            records: MoveTable::new(table_id, 3),
            capabilities: CapabilityManger {
                allowed_addresses: MoveVecSet {
                    contents: vec![leader_a],
                },
                network_id: crate::move_bindings::sui_framework::object::ID::new(network_id),
                admin_cap_id: crate::move_bindings::sui_framework::object::ID::new(admin_cap_id),
                leader_cap_issuer: leader_cap_issuer_for_test(issuer_id, registry_id, issuer_inner),
            },
            workflow_witness_type: MoveOption::from_option(Some(TypeName::new(
                "0x42::workflow::Witness",
            ))),
        };

        let bytes = bcs::to_bytes(&original).expect("encode populated registry");
        let decoded = LeaderRegistry::from_bcs(&bytes).expect("decode round-trips");
        let re_encoded = bcs::to_bytes(&decoded).expect("re-encode decoded registry");

        assert_eq!(
            bytes, re_encoded,
            "BCS round-trip must be byte-identical for the populated registry"
        );
        assert_eq!(decoded.network_id(), network_id);
        assert_eq!(decoded.max_transaction_budget(), 7_500_000_000);
    }

    #[test]
    fn capability_manger_clone_preserves_nested_owner_cap() {
        let registry_id = sui::types::Address::from_static("0x10");
        let network_id = sui::types::Address::from_static("0x20");
        let leader = sui::types::Address::from_static("0x30");
        let admin_cap_id = sui::types::Address::from_static("0x50");
        let issuer_id = sui::types::Address::from_static("0x60");
        let issuer_inner = sui::types::Address::from_static("0x61");
        let capabilities = CapabilityManger {
            allowed_addresses: MoveVecSet {
                contents: vec![leader],
            },
            network_id: crate::move_bindings::sui_framework::object::ID::new(network_id),
            admin_cap_id: crate::move_bindings::sui_framework::object::ID::new(admin_cap_id),
            leader_cap_issuer: leader_cap_issuer_for_test(issuer_id, registry_id, issuer_inner),
        };

        let cloned = capabilities.clone();

        assert_eq!(cloned.allowed_addresses.contents, vec![leader]);
        assert_eq!(cloned.network_id(), network_id);
        assert_eq!(cloned.admin_cap_id.bytes, admin_cap_id);
        assert_eq!(cloned.leader_cap_issuer.id.id.bytes, issuer_id);
        assert_eq!(cloned.leader_cap_issuer.what_for.bytes, registry_id);
        assert_eq!(cloned.leader_cap_issuer.inner.unique.bytes, issuer_inner);
    }

    #[test]
    fn from_bcs_reports_truncated_input_via_context() {
        let original = LeaderRegistry::new_for_test(
            sui::types::Address::from_static("0x1"),
            sui::types::Address::from_static("0x2"),
        );
        let bytes = bcs::to_bytes(&original).expect("encode test registry");
        let truncated = &bytes[..bytes.len() / 2];

        let error = LeaderRegistry::from_bcs(truncated)
            .expect_err("decoding a truncated payload must fail");

        assert!(
            error
                .to_string()
                .contains("failed to decode leader registry object"),
            "expected the from_bcs context message, got: {error}"
        );
    }

    #[test]
    fn from_bcs_rejects_garbage_input() {
        let error =
            LeaderRegistry::from_bcs(b"not-bcs").expect_err("decoding random bytes must fail");

        assert!(
            error
                .to_string()
                .contains("failed to decode leader registry object"),
            "expected the from_bcs context message, got: {error}"
        );
    }
}
