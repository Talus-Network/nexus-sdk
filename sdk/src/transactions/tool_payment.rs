//! Tool payment transactions.

use crate::{
    move_bindings::{
        tool::payment_extension as payment_extension_binding,
        workflow::tool_payment_adapter as tool_payment_adapter_binding,
    },
    move_boundary,
    sui,
    types::NexusObjects,
};

fn payment_admin_args(
    transaction: &mut move_boundary::NexusPtbBuilder,
    tool_payment: &sui::types::ObjectReference,
    payment_admin: &sui::types::ObjectReference,
) -> anyhow::Result<Vec<sui::types::Argument>> {
    Ok(vec![
        transaction.shared_object(tool_payment, true)?,
        transaction.owned_object(payment_admin)?,
    ])
}

fn payment_ticket_args(
    transaction: &mut move_boundary::NexusPtbBuilder,
    tool_payment: &sui::types::ObjectReference,
    amount: u64,
    pay_with: &sui::types::ObjectReference,
) -> anyhow::Result<Vec<sui::types::Argument>> {
    Ok(vec![
        transaction.shared_object(tool_payment, true)?,
        transaction.arg(&amount)?,
        transaction.owned_object(pay_with)?,
        transaction.clock()?,
    ])
}

/// Enable expiry payment tickets for a Tool.
pub(crate) fn enable_expiry_ptb(
    objects: &NexusObjects,
    tool_payment: &sui::types::ObjectReference,
    payment_admin: &sui::types::ObjectReference,
    cost_per_minute: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let mut arguments = payment_admin_args(transaction, tool_payment, payment_admin)?;
        arguments.push(transaction.arg(&cost_per_minute)?);
        transaction.call_target(payment_extension_binding::enable_expiry_target, arguments)?;
        Ok(())
    })
}

/// Disable expiry payment tickets for a Tool.
pub(crate) fn disable_expiry_ptb(
    objects: &NexusObjects,
    tool_payment: &sui::types::ObjectReference,
    payment_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = payment_admin_args(transaction, tool_payment, payment_admin)?;
        transaction.call_target(payment_extension_binding::disable_expiry_target, arguments)?;
        Ok(())
    })
}

/// Buy an expiry payment ticket.
pub(crate) fn buy_expiry_payment_ticket_ptb(
    objects: &NexusObjects,
    tool_payment: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    minutes: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = payment_ticket_args(transaction, tool_payment, minutes, pay_with)?;
        transaction.call_target(
            payment_extension_binding::buy_expiry_payment_ticket_target,
            arguments,
        )?;
        Ok(())
    })
}

/// Enable limited invocation payment tickets for a Tool.
pub(crate) fn enable_limited_invocations_ptb(
    objects: &NexusObjects,
    tool_payment: &sui::types::ObjectReference,
    payment_admin: &sui::types::ObjectReference,
    cost_per_invocation: u64,
    min_invocations: u64,
    max_invocations: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let mut arguments = payment_admin_args(transaction, tool_payment, payment_admin)?;
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
    objects: &NexusObjects,
    tool_payment: &sui::types::ObjectReference,
    payment_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = payment_admin_args(transaction, tool_payment, payment_admin)?;
        transaction.call_target(
            payment_extension_binding::disable_limited_invocations_target,
            arguments,
        )?;
        Ok(())
    })
}

/// Buy a limited invocation payment ticket.
pub(crate) fn buy_limited_invocations_payment_ticket_ptb(
    objects: &NexusObjects,
    tool_payment: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    invocations: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = payment_ticket_args(transaction, tool_payment, invocations, pay_with)?;
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
    tool_payment: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    expected_vertex: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    transaction.call_target(
        tool_payment_adapter_binding::settle_payment_state_for_vertex_target,
        vec![tool_payment, dag, execution, expected_vertex],
    )
}

/// Abort an expired execution after refunding its matching Tool payment lock.
pub fn abort_expired_execution_with_tool_payment_ptb(
    objects: &NexusObjects,
    tool_payment: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let tool_payment = transaction.shared_object(tool_payment, true)?;
        let dag = transaction.shared_object(dag, false)?;
        let execution = transaction.shared_object(execution, true)?;
        let clock = transaction.clock()?;
        transaction.call_target(
            tool_payment_adapter_binding::abort_expired_execution_with_tool_payment_target,
            vec![tool_payment, dag, execution, clock],
        )?;
        Ok(())
    })
}
