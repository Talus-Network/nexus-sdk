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

    #[cfg(test)]
    pub(crate) fn new_for_test(id: sui::types::Address, network: sui::types::Address) -> Self {
        Self {
            id,
            unbonding_duration_ms: 0,
            min_stake_mist: 0,
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
