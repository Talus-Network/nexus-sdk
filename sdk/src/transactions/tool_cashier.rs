//! Tool cashier policy administration and entitlement purchases.

use crate::{
    move_bindings::{
        interface::payment::PaymentSourceKind,
        sui_framework::transfer as transfer_binding,
        tool::{
            finite_credits as finite_credits_binding,
            fixed_price as fixed_price_binding,
            free_invocation as free_invocation_binding,
            time_pass::{self as time_pass_binding, TimePass},
        },
    },
    move_boundary,
    sui,
    types::NexusObjects,
};

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

fn policy_toggle_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    target: impl FnOnce() -> Result<sui_move_call::CallTarget, sui_move_call::CallSpecError>,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let arguments = cashier_admin_args(transaction, tool_cashier, cashier_admin)?;
        transaction.call_target(target, arguments)?;
        Ok(())
    })
}

/// Enables the canonical fixed price policy.
pub fn enable_fixed_price_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        fixed_price_binding::enable_target,
    )
}

/// Disables the canonical fixed price policy.
pub fn disable_fixed_price_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        fixed_price_binding::disable_target,
    )
}

/// Enables sponsored free Invocations.
pub fn enable_free_invocation_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        free_invocation_binding::enable_target,
    )
}

/// Disables sponsored free Invocations.
pub fn disable_free_invocation_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        free_invocation_binding::disable_target,
    )
}

/// Enables finite credit sales and admission.
pub fn enable_finite_credits_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    price_per_credit: u64,
    minimum_credits: u64,
    maximum_credits: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let mut arguments = cashier_admin_args(transaction, tool_cashier, cashier_admin)?;
        arguments.push(transaction.arg(&price_per_credit)?);
        arguments.push(transaction.arg(&minimum_credits)?);
        arguments.push(transaction.arg(&maximum_credits)?);
        transaction.call_target(finite_credits_binding::enable_target, arguments)?;
        Ok(())
    })
}

/// Disables finite credit sales and admission.
pub fn disable_finite_credits_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        finite_credits_binding::disable_target,
    )
}

/// Purchases and shares independently consumable finite credits.
pub fn buy_finite_credits_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    refund_to: sui::types::Address,
    credits: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let coin = transaction.owned_object(pay_with)?;
        let beneficiary = transaction.arg(&beneficiary)?;
        let refund_to = transaction.arg(&refund_to)?;
        let credits_count = transaction.arg(&credits)?;
        let result = transaction.call_target(
            finite_credits_binding::buy_target,
            vec![cashier, coin, beneficiary, refund_to, credits_count],
        )?;
        let credits = transaction.nested_result(result, 0)?;
        transaction.call_target(finite_credits_binding::share_target, vec![credits])?;
        Ok(())
    })
}

/// Enables time pass sales and admission.
pub fn enable_time_pass_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    price_per_ms: u64,
    minimum_duration_ms: u64,
    maximum_duration_ms: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let mut arguments = cashier_admin_args(transaction, tool_cashier, cashier_admin)?;
        arguments.push(transaction.arg(&price_per_ms)?);
        arguments.push(transaction.arg(&minimum_duration_ms)?);
        arguments.push(transaction.arg(&maximum_duration_ms)?);
        transaction.call_target(time_pass_binding::enable_target, arguments)?;
        Ok(())
    })
}

/// Disables time pass sales and admission.
pub fn disable_time_pass_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        time_pass_binding::disable_target,
    )
}

/// Purchases and freezes a time pass for parallel admission.
pub fn buy_time_pass_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    duration_ms: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let coin = transaction.owned_object(pay_with)?;
        let clock = transaction.clock()?;
        let beneficiary = transaction.arg(&beneficiary)?;
        let duration = transaction.arg(&duration_ms)?;
        let result = transaction.call_target(
            time_pass_binding::buy_target,
            vec![cashier, coin, clock, beneficiary, duration],
        )?;
        let pass = transaction.nested_result(result, 0)?;
        transaction.call_target(
            transfer_binding::public_freeze_object_target::<TimePass>,
            vec![pass],
        )?;
        Ok(())
    })
}
