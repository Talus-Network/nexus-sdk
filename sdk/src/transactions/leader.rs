//! Programmable transaction builders for `nexus_registry::leader`.

use {
    crate::{
        move_bindings::{
            primitives::owner_cap,
            registry::{leader as leader_binding, leader_cap},
            sui_framework::{coin as coin_binding, sui as sui_binding},
        },
        move_boundary,
        sui,
        types::NexusObjects,
    },
    sui::types::ProgrammableTransaction,
};

type OverNetworkCap = owner_cap::CloneableOwnerCap<leader_cap::OverNetwork>;

fn sui_type_tag(objects: &NexusObjects) -> sui::types::TypeTag {
    crate::move_bindings::type_tag::<sui_binding::SUI>(objects)
}

/// Struct tag for the shared `CloneableOwnerCap<OverNetwork>` capability.
pub fn over_network_cap_struct_tag(objects: &NexusObjects) -> sui::types::StructTag {
    crate::move_bindings::struct_tag::<OverNetworkCap>(objects)
}

/// Register the transaction sender as a leader using sender address-balance funds.
pub fn register_for_self_ptb(
    objects: &NexusObjects,
    stake_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_object(&objects.leader_registry, true)?;
        let pay_with = tx.funds_withdrawal_coin(sui_type_tag(objects), stake_mist)?;
        let amount = tx.arg(&stake_mist)?;
        let metadata = tx.call_target(leader_binding::empty_metadata_target, vec![])?;
        let clock = tx.clock()?;

        tx.call_target(
            leader_binding::register_target,
            vec![leader_registry, pay_with, amount, metadata, clock],
        )?;
        tx.call_target(
            coin_binding::destroy_zero_target::<sui_binding::SUI>,
            vec![pay_with],
        )?;

        Ok(())
    })
}

/// Activate this leader and claim ownership with the transaction digest token.
pub fn activate_and_claim_for_self_ptb(
    objects: &NexusObjects,
    leader_cap_over_network: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_object(&objects.leader_registry, true)?;
        let leader_cap = tx.shared_object(leader_cap_over_network, false)?;

        tx.call_target(
            leader_binding::activate_and_claim_target,
            vec![leader_registry, leader_cap],
        )?;
        Ok(())
    })
}

/// Suspend this leader only if `token` still matches the active claim token.
pub fn suspend_if_token_for_self_ptb(
    objects: &NexusObjects,
    leader_cap_over_network: &sui::types::ObjectReference,
    token: Vec<u8>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_object(&objects.leader_registry, true)?;
        let leader_cap = tx.shared_object(leader_cap_over_network, false)?;
        let token = tx.arg(&token)?;

        tx.call_target(
            leader_binding::suspend_if_token_target,
            vec![leader_registry, leader_cap, token],
        )?;
        Ok(())
    })
}
