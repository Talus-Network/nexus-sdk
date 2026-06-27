//! Rust projections of `nexus_registry::leader` on-chain object data.

use {
    super::{MoveOption, MoveTable, MoveVecSet, TypeName},
    crate::{
        sui,
        types::{
            generated::{primitives_types, registry_types},
            generated_support,
        },
    },
    anyhow::Context,
    serde::{Deserialize, Serialize},
};

/// Shared `nexus_registry::leader::LeaderRegistry` object contents.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeaderRegistry {
    id: sui::types::Address,
    unbonding_duration_ms: u64,
    min_stake_mist: u64,
    max_transaction_budget: u64,
    leaders: MoveVecSet<sui::types::Address>,
    records: MoveTable<sui::types::Address, generated_support::Ignored>,
    capabilities: LeaderCapabilities,
    workflow_witness_type: MoveOption<TypeName>,
}

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
        let raw = bcs::from_bytes::<registry_types::leader::LeaderRegistry>(contents)
            .context("failed to decode leader registry object")?;
        Ok(Self::from_raw(raw))
    }

    /// Network ID used by all leader capabilities issued by this registry.
    pub fn network_id(&self) -> sui::types::Address {
        self.capabilities.network_id()
    }

    /// Maximum budget (in MIST) a leader may spend on a single transaction.
    pub fn max_transaction_budget(&self) -> u64 {
        self.max_transaction_budget
    }

    fn from_raw(raw: registry_types::leader::LeaderRegistry) -> Self {
        Self {
            id: raw.id.into(),
            unbonding_duration_ms: raw.unbonding_duration_ms,
            min_stake_mist: raw.min_stake_mist,
            max_transaction_budget: raw.max_transaction_budget,
            leaders: MoveVecSet {
                contents: raw.leaders.contents.into_iter().map(Into::into).collect(),
            },
            records: MoveTable::new(raw.records.id, raw.records.size),
            capabilities: LeaderCapabilities::from_raw(raw.capabilities),
            workflow_witness_type: raw.workflow_witness_type,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(id: sui::types::Address, network: sui::types::Address) -> Self {
        Self {
            id,
            unbonding_duration_ms: 0,
            min_stake_mist: 0,
            max_transaction_budget: 10_000_000_000,
            leaders: MoveVecSet { contents: vec![] },
            records: MoveTable::new(sui::types::Address::ZERO, 0),
            capabilities: LeaderCapabilities {
                allowed_addresses: MoveVecSet { contents: vec![] },
                network_id: network,
                admin_cap_id: sui::types::Address::ZERO,
                leader_cap_issuer: leader_cap_issuer_for_test(
                    sui::types::Address::ZERO,
                    id,
                    sui::types::Address::ZERO,
                ),
            },
            workflow_witness_type: MoveOption(None),
        }
    }
}

type LeaderCapIssuer =
    primitives_types::owner_cap::CloneableOwnerCap<registry_types::leader_cap::OverNetwork>;

/// Stored capability issuer state inside `LeaderRegistry`.
#[derive(Debug, Deserialize, Serialize)]
pub struct LeaderCapabilities {
    allowed_addresses: MoveVecSet<sui::types::Address>,
    network_id: sui::types::Address,
    admin_cap_id: sui::types::Address,
    leader_cap_issuer: LeaderCapIssuer,
}

impl Clone for LeaderCapabilities {
    fn clone(&self) -> Self {
        Self {
            allowed_addresses: self.allowed_addresses.clone(),
            network_id: self.network_id,
            admin_cap_id: self.admin_cap_id,
            leader_cap_issuer: clone_leader_cap_issuer(&self.leader_cap_issuer),
        }
    }
}

impl LeaderCapabilities {
    /// Network ID used by all issued `leader_cap::OverNetwork` objects.
    pub fn network_id(&self) -> sui::types::Address {
        self.network_id
    }

    fn from_raw(raw: registry_types::leader::CapabilityManger) -> Self {
        Self {
            allowed_addresses: raw.allowed_addresses,
            network_id: raw.network_id.into(),
            admin_cap_id: raw.admin_cap_id.into(),
            leader_cap_issuer: raw.leader_cap_issuer,
        }
    }
}

fn clone_leader_cap_issuer(value: &LeaderCapIssuer) -> LeaderCapIssuer {
    leader_cap_issuer_from_parts(
        value.id.id.bytes,
        value.what_for.bytes,
        value.inner.unique.bytes,
    )
}

fn leader_cap_issuer_from_parts(
    id: sui::types::Address,
    what_for: sui::types::Address,
    unique: sui::types::Address,
) -> LeaderCapIssuer {
    LeaderCapIssuer {
        id: generated_support::UID {
            id: generated_support::ID { bytes: id },
        },
        what_for: generated_support::ID { bytes: what_for },
        inner: primitives_types::owner_cap::OwnerCap {
            unique: generated_support::ID { bytes: unique },
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

    /// Round-trip the empty `new_for_test` payload: BCS-encode, decode via
    /// `from_bcs`, then re-encode and assert the byte stream is bit-identical
    /// to the input. This is the strongest invariant we can assert without
    /// `PartialEq` on the type — it proves every nested struct (`MoveVecSet`,
    /// `MoveTable`, `LeaderCapabilities`, generated owner-cap types, and
    /// `MoveOption<TypeName>`) reads back exactly what was written.
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

    /// Round-trip a populated payload that exercises the non-empty branch of
    /// every collection on the type: a `MoveVecSet` with multiple addresses,
    /// a `MoveTable` with a non-zero size, and a `MoveOption<TypeName>`
    /// holding `Some(TypeName)`. Catches Deserialize regressions that an
    /// empty-only fixture would silently let through (e.g. wrong tag width
    /// on the option, wrong length prefix on the set, missing `#[serde(skip)]`
    /// on `MoveTable._marker`).
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
            id: registry_id,
            unbonding_duration_ms: 5_000,
            min_stake_mist: 1_000_000,
            max_transaction_budget: 7_500_000_000,
            leaders: MoveVecSet {
                contents: vec![leader_a, leader_b],
            },
            records: MoveTable::new(table_id, 3),
            capabilities: LeaderCapabilities {
                allowed_addresses: MoveVecSet {
                    contents: vec![leader_a],
                },
                network_id,
                admin_cap_id,
                leader_cap_issuer: leader_cap_issuer_for_test(issuer_id, registry_id, issuer_inner),
            },
            workflow_witness_type: MoveOption(Some(TypeName::new("0x42::workflow::Witness"))),
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
    fn leader_capabilities_clone_preserves_nested_generated_owner_cap() {
        let registry_id = sui::types::Address::from_static("0x10");
        let network_id = sui::types::Address::from_static("0x20");
        let leader = sui::types::Address::from_static("0x30");
        let admin_cap_id = sui::types::Address::from_static("0x50");
        let issuer_id = sui::types::Address::from_static("0x60");
        let issuer_inner = sui::types::Address::from_static("0x61");
        let capabilities = LeaderCapabilities {
            allowed_addresses: MoveVecSet {
                contents: vec![leader],
            },
            network_id,
            admin_cap_id,
            leader_cap_issuer: leader_cap_issuer_for_test(issuer_id, registry_id, issuer_inner),
        };

        let cloned = capabilities.clone();

        assert_eq!(cloned.allowed_addresses.contents, vec![leader]);
        assert_eq!(cloned.network_id(), network_id);
        assert_eq!(cloned.admin_cap_id, admin_cap_id);
        assert_eq!(cloned.leader_cap_issuer.id.id.bytes, issuer_id);
        assert_eq!(cloned.leader_cap_issuer.what_for.bytes, registry_id);
        assert_eq!(cloned.leader_cap_issuer.inner.unique.bytes, issuer_inner);
    }

    /// Truncated input must surface the contextual error message from
    /// `from_bcs` rather than panicking or returning a generic `bcs::Error`.
    /// Half the byte stream guarantees the decoder runs out partway through a
    /// nested struct so we hit the `context(...)` wrap.
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

    /// Garbage bytes that don't even parse as the leading `Address` must still
    /// surface the wrapped error. Pairs with the truncation test to cover both
    /// "ran out of bytes" and "field is unparsable" failure modes.
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
