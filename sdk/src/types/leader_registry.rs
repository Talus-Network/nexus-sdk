//! Rust projections of `nexus_registry::leader` on-chain object data.

use {
    super::{MoveTable, MoveVecSet, TypeName},
    crate::sui,
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
    records: MoveTable<sui::types::Address, LeaderRecord>,
    capabilities: LeaderCapabilities,
    workflow_witness_type: MoveOptionLayout<TypeName>,
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
        bcs::from_bytes(contents).context("failed to decode leader registry object")
    }

    /// Network ID used by all leader capabilities issued by this registry.
    pub fn network_id(&self) -> sui::types::Address {
        self.capabilities.network_id()
    }

    /// Maximum budget (in MIST) a leader may spend on a single transaction.
    pub fn max_transaction_budget(&self) -> u64 {
        self.max_transaction_budget
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
                leader_cap_issuer: CloneableOwnerCap {
                    id: sui::types::Address::ZERO,
                    what_for: id,
                    inner: OwnerCap {
                        unique: sui::types::Address::ZERO,
                    },
                },
            },
            workflow_witness_type: MoveOptionLayout { vec: vec![] },
        }
    }
}

/// Stored capability issuer state inside `LeaderRegistry`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeaderCapabilities {
    allowed_addresses: MoveVecSet<sui::types::Address>,
    network_id: sui::types::Address,
    admin_cap_id: sui::types::Address,
    leader_cap_issuer: CloneableOwnerCap,
}

impl LeaderCapabilities {
    /// Network ID used by all issued `leader_cap::OverNetwork` objects.
    pub fn network_id(&self) -> sui::types::Address {
        self.network_id
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct LeaderRecord;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CloneableOwnerCap {
    id: sui::types::Address,
    what_for: sui::types::Address,
    inner: OwnerCap,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OwnerCap {
    unique: sui::types::Address,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct MoveOptionLayout<T> {
    vec: Vec<T>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip the empty `new_for_test` payload: BCS-encode, decode via
    /// `from_bcs`, then re-encode and assert the byte stream is bit-identical
    /// to the input. This is the strongest invariant we can assert without
    /// `PartialEq` on the type — it proves every nested struct (`MoveVecSet`,
    /// `MoveTable`, `LeaderCapabilities`, `CloneableOwnerCap`, `OwnerCap`,
    /// `MoveOptionLayout<TypeName>`) reads back exactly what was written.
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
    /// a `MoveTable` with a non-zero size, and a `MoveOptionLayout<TypeName>`
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
                leader_cap_issuer: CloneableOwnerCap {
                    id: issuer_id,
                    what_for: registry_id,
                    inner: OwnerCap {
                        unique: issuer_inner,
                    },
                },
            },
            workflow_witness_type: MoveOptionLayout {
                vec: vec![TypeName::new("0x42::workflow::Witness")],
            },
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
