//! Nexus network fee transactions.

use crate::{
    move_bindings::registry::priority_fee_vault as priority_fee_vault_binding,
    move_boundary,
    sui,
    types::NexusObjects,
};

/// Configure the `$US` priority fee vault exchange rate.
pub fn configure_priority_fee_vault(
    objects: &NexusObjects,
    exchange_rate_million_mists_us: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let vault = transaction.shared_object(&objects.priority_fee_vault, true)?;
        let owner_cap = transaction.owned_object(&objects.priority_fee_vault_owner_cap)?;
        let exchange_rate = transaction.arg(&exchange_rate_million_mists_us)?;
        transaction.call_target(
            priority_fee_vault_binding::configure_target,
            vec![vault, owner_cap, exchange_rate],
        )?;
        Ok(())
    })
}

/// Swap an owned `Coin<US>` for vault SUI.
pub fn swap_us_for_sui(
    objects: &NexusObjects,
    us_coin: &sui::types::ObjectReference,
    min_sui_out: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let vault = transaction.shared_object(&objects.priority_fee_vault, true)?;
        let us_coin = transaction.owned_object(us_coin)?;
        let min_sui_out = transaction.arg(&min_sui_out)?;
        let result = transaction.call_target(
            priority_fee_vault_binding::swap_us_for_sui_target,
            vec![vault, us_coin, min_sui_out],
        )?;
        let sui_out = transaction.nested_result(result, 0)?;
        let us_refund = transaction.nested_result(result, 1)?;
        let recipient = transaction.arg(&recipient)?;
        transaction.transfer_objects(vec![sui_out, us_refund], recipient)?;
        Ok(())
    })
}

/// Withdraw a leader's priority fee share from the network vault.
pub fn withdraw_priority_fee(
    objects: &NexusObjects,
    leader_cap: &sui::types::ObjectReference,
    share_to_withdraw: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let vault = transaction.shared_object(&objects.priority_fee_vault, true)?;
        let leader_registry = transaction.shared_object(&objects.leader_registry, false)?;
        let leader_cap = transaction.owned_object(leader_cap)?;
        let share = transaction.arg(&share_to_withdraw)?;
        let us_out = transaction.call_target(
            priority_fee_vault_binding::withdraw_priority_fee_target,
            vec![vault, leader_registry, leader_cap, share],
        )?;
        let recipient = transaction.arg(&recipient)?;
        transaction.transfer_objects(vec![us_out], recipient)?;
        Ok(())
    })
}
