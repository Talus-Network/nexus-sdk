//! Tool cashier transactions.

use crate::{
    move_bindings::{
        sui_framework::{coin as coin_binding, sui::SUI},
        tool::{
            payment_extension as payment_extension_binding,
            tool_cashier as tool_cashier_binding,
        },
        workflow::tool_cashier_adapter as tool_cashier_adapter_binding,
    },
    move_boundary,
    sui,
    types::NexusContext,
};

/// Builds a transaction that drains settled SUI from a
/// [`ToolCashier`](tool_cashier_binding::ToolCashier).
pub fn drain_for_self_ptb(
    context: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(context, |transaction| {
        let tool_cashier = transaction.shared_object(tool_cashier, true)?;
        let owner_cap = transaction.owned_object(owner_cap)?;
        let balance = transaction.call_target(
            tool_cashier_binding::claim_target,
            vec![tool_cashier, owner_cap],
        )?;
        let coin =
            transaction.call_target(coin_binding::from_balance_target::<SUI>, vec![balance])?;
        let recipient = transaction.arg(&recipient)?;
        transaction.transfer_objects(vec![coin], recipient)?;
        Ok(())
    })
}

fn cashier_admin_args(
    transaction: &mut move_boundary::NexusPtbBuilder,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<Vec<sui::types::Argument>> {
    Ok(vec![
        transaction.shared_object(tool_cashier, true)?,
        transaction.owned_object(cashier_admin)?,
    ])
}

fn payment_ticket_args(
    transaction: &mut move_boundary::NexusPtbBuilder,
    tool_cashier: &sui::types::ObjectReference,
    amount: u64,
    pay_with: &sui::types::ObjectReference,
) -> anyhow::Result<Vec<sui::types::Argument>> {
    Ok(vec![
        transaction.shared_object(tool_cashier, true)?,
        transaction.arg(&amount)?,
        transaction.owned_object(pay_with)?,
        transaction.clock()?,
    ])
}

/// Enable expiry payment tickets for a Tool.
pub(crate) fn enable_expiry_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    cost_per_minute: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let mut arguments = cashier_admin_args(transaction, tool_cashier, cashier_admin)?;
        arguments.push(transaction.arg(&cost_per_minute)?);
        transaction.call_target(payment_extension_binding::enable_expiry_target, arguments)?;
        Ok(())
    })
}

/// Disable expiry payment tickets for a Tool.
pub(crate) fn disable_expiry_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = cashier_admin_args(transaction, tool_cashier, cashier_admin)?;
        transaction.call_target(payment_extension_binding::disable_expiry_target, arguments)?;
        Ok(())
    })
}

/// Buy an expiry payment ticket.
pub(crate) fn buy_expiry_payment_ticket_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    minutes: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = payment_ticket_args(transaction, tool_cashier, minutes, pay_with)?;
        transaction.call_target(
            payment_extension_binding::buy_expiry_payment_ticket_target,
            arguments,
        )?;
        Ok(())
    })
}

/// Enable limited invocation payment tickets for a Tool.
pub(crate) fn enable_limited_invocations_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    cost_per_invocation: u64,
    min_invocations: u64,
    max_invocations: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let mut arguments = cashier_admin_args(transaction, tool_cashier, cashier_admin)?;
        arguments.push(transaction.arg(&cost_per_invocation)?);
        arguments.push(transaction.arg(&min_invocations)?);
        arguments.push(transaction.arg(&max_invocations)?);
        transaction.call_target(
            payment_extension_binding::enable_limited_invocations_target,
            arguments,
        )?;
        Ok(())
    })
}

/// Disable limited invocation payment tickets for a Tool.
pub(crate) fn disable_limited_invocations_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = cashier_admin_args(transaction, tool_cashier, cashier_admin)?;
        transaction.call_target(
            payment_extension_binding::disable_limited_invocations_target,
            arguments,
        )?;
        Ok(())
    })
}

/// Buy a limited invocation payment ticket.
pub(crate) fn buy_limited_invocations_payment_ticket_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    invocations: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = payment_ticket_args(transaction, tool_cashier, invocations, pay_with)?;
        transaction.call_target(
            payment_extension_binding::buy_limited_invocations_payment_ticket_target,
            arguments,
        )?;
        Ok(())
    })
}

/// Settle pending Tool payment state for one vertex.
pub(crate) fn settle_payment_state_for_vertex(
    transaction: &mut move_boundary::NexusPtbBuilder,
    tool_cashier: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    expected_vertex: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    transaction.call_target(
        tool_cashier_adapter_binding::settle_payment_state_for_vertex_target,
        vec![tool_cashier, dag, execution, expected_vertex],
    )
}

/// Abort an expired execution after refunding its matching Tool payment lock.
pub fn abort_expired_execution_with_tool_cashier_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let tool_cashier = transaction.shared_object(tool_cashier, true)?;
        let dag = transaction.shared_object(dag, false)?;
        let execution = transaction.shared_object(execution, true)?;
        let clock = transaction.clock()?;
        transaction.call_target(
            tool_cashier_adapter_binding::abort_expired_execution_with_tool_cashier_target,
            vec![tool_cashier, dag, execution, clock],
        )?;
        Ok(())
    })
}
