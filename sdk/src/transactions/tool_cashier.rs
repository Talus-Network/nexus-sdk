//! Tool cashier policy administration and entitlement purchases.

use crate::{
    move_bindings::{
        interface::payment::PaymentSourceKind,
        move_std::type_name::TypeName,
        sui_framework::{
            coin as coin_binding,
            sui::SUI,
            transfer::{self as transfer_binding, Receiving},
        },
        tool::{
            finite_credits as finite_credits_binding, fixed_price as fixed_price_binding,
            free_invocation as free_invocation_binding,
            invocation::Invocation,
            time_pass::{self as time_pass_binding, TimePass},
            tool_cashier::{self as tool_cashier_binding, CashierDeposit},
        },
    },
    move_boundary, sui,
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

/// Closes finite credit issuance without invalidating existing [`Credits`](crate::move_bindings::tool::finite_credits::Credits).
pub fn close_finite_credit_issuance_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        finite_credits_binding::close_issuance_target,
    )
}

/// Opens finite credit issuance using the current offer terms.
pub fn open_finite_credit_issuance_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        finite_credits_binding::open_issuance_target,
    )
}

/// Updates the finite credit offer while preserving issued credits.
pub fn update_finite_credit_terms_ptb(
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
        transaction.call_target(finite_credits_binding::update_terms_target, arguments)?;
        Ok(())
    })
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

/// Issues and shares finite credits under Tool owner authority.
pub fn issue_finite_credits_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    refund_to: sui::types::Address,
    credits: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let beneficiary = transaction.arg(&beneficiary)?;
        let refund_to = transaction.arg(&refund_to)?;
        let credits = transaction.arg(&credits)?;
        let issued = transaction.call_target(
            finite_credits_binding::issue_target,
            vec![cashier, admin, beneficiary, refund_to, credits],
        )?;
        transaction.call_target(finite_credits_binding::share_target, vec![issued])?;
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

/// Closes time pass issuance without invalidating existing [`TimePass`].
pub fn close_time_pass_issuance_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        time_pass_binding::close_issuance_target,
    )
}

/// Opens time pass issuance using the current offer terms.
pub fn open_time_pass_issuance_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    policy_toggle_ptb(
        objects,
        tool_cashier,
        cashier_admin,
        time_pass_binding::open_issuance_target,
    )
}

/// Updates the time pass offer while preserving issued passes.
pub fn update_time_pass_terms_ptb(
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
        transaction.call_target(time_pass_binding::update_terms_target, arguments)?;
        Ok(())
    })
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

/// Issues and freezes a time pass under Tool owner authority.
pub fn issue_time_pass_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    valid_from_ms: u64,
    valid_until_ms: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let beneficiary = transaction.arg(&beneficiary)?;
        let valid_from_ms = transaction.arg(&valid_from_ms)?;
        let valid_until_ms = transaction.arg(&valid_until_ms)?;
        let pass = transaction.call_target(
            time_pass_binding::issue_target,
            vec![cashier, admin, beneficiary, valid_from_ms, valid_until_ms],
        )?;
        transaction.call_target(
            transfer_binding::public_freeze_object_target::<TimePass>,
            vec![pass],
        )?;
        Ok(())
    })
}

/// Splits shared finite credits and shares the independent result.
pub fn split_finite_credits_ptb(
    objects: &NexusObjects,
    credits: &sui::types::ObjectReference,
    amount: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let credits = transaction.shared_object(credits, true)?;
        let amount = transaction.arg(&amount)?;
        let split =
            transaction.call_target(finite_credits_binding::split_target, vec![credits, amount])?;
        transaction.call_target(finite_credits_binding::share_target, vec![split])?;
        Ok(())
    })
}

/// Joins one shared finite credit object into another.
pub fn join_finite_credits_ptb(
    objects: &NexusObjects,
    credits: &sui::types::ObjectReference,
    other: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let credits = transaction.shared_object(credits, true)?;
        let other = transaction.shared_object(other, true)?;
        transaction.call_target(finite_credits_binding::join_target, vec![credits, other])?;
        Ok(())
    })
}

/// Claims one refunded Invocation as a shared one unit credit object.
pub fn claim_finite_credit_refund_ptb(
    objects: &NexusObjects,
    refunded: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let refunded = transaction.owned_object(refunded)?;
        let credits =
            transaction.call_target(finite_credits_binding::claim_refund_target, vec![refunded])?;
        transaction.call_target(finite_credits_binding::share_target, vec![credits])?;
        Ok(())
    })
}

/// Collects completed Invocations through the policy module that created them.
///
/// Every policy module exposes `collect` with the canonical Invocation batch
/// signature. Collection mutates [`ToolCashier`](crate::move_bindings::tool::tool_cashier::ToolCashier)
/// only after execution and therefore does not join the admission conflict
/// domain.
pub fn collect_invocations_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    policy: &TypeName,
    invocations: &[sui::types::ObjectReference],
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    if invocations.is_empty() {
        anyhow::bail!("cashier collection requires at least one Invocation");
    }
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, true)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let invocations = invocations
            .iter()
            .map(|invocation| transaction.receiving_object::<Invocation>(invocation))
            .collect::<Result<Vec<_>, _>>()?;
        let invocations = transaction.move_vector::<Receiving<Invocation>>(invocations)?;
        let (package, module) = super::invocation::policy_target(objects, policy)?;
        let funds = transaction.call_function(
            package,
            module,
            "collect",
            vec![cashier, admin, invocations],
        )?;
        let coin =
            transaction.call_target(coin_binding::from_balance_target::<SUI>, vec![funds])?;
        let recipient = transaction.arg(&recipient)?;
        transaction.transfer_objects(vec![coin], recipient)?;
        Ok(())
    })
}

/// Collects prepaid sale deposits from a [`ToolCashier`](crate::move_bindings::tool::tool_cashier::ToolCashier)
/// inbox and sends one SUI coin to `recipient`.
pub fn collect_deposits_ptb(
    objects: &NexusObjects,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    deposits: &[sui::types::ObjectReference],
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    if deposits.is_empty() {
        anyhow::bail!("cashier collection requires at least one deposit");
    }
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, true)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let deposits = deposits
            .iter()
            .map(|deposit| transaction.receiving_object::<CashierDeposit>(deposit))
            .collect::<Result<Vec<_>, _>>()?;
        let deposits = transaction.move_vector::<Receiving<CashierDeposit>>(deposits)?;
        let funds = transaction.call_target(
            tool_cashier_binding::collect_deposits_target,
            vec![cashier, admin, deposits],
        )?;
        let coin =
            transaction.call_target(coin_binding::from_balance_target::<SUI>, vec![funds])?;
        let recipient = transaction.arg(&recipient)?;
        transaction.transfer_objects(vec![coin], recipient)?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::move_std::type_name::TypeName,
            test_utils::sui_mocks::{mock_nexus_objects, object_ref_for_id},
            transactions::invocation::InvocationPolicyCall,
        },
        sui_sdk_types::Command,
    };

    #[test]
    fn collection_rejects_an_empty_invocation_batch() {
        let objects = mock_nexus_objects();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let policy = InvocationPolicyCall::fixed_price(&objects).policy;

        let error = collect_invocations_ptb(
            &objects,
            &cashier,
            &admin,
            &policy,
            &[],
            sui::types::Address::from_static("0xc3"),
        )
        .expect_err("empty collection must fail locally");

        assert!(error.to_string().contains("at least one Invocation"));
    }

    #[test]
    fn custom_policy_collection_calls_the_policy_module() {
        let objects = mock_nexus_objects();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let invocation = object_ref_for_id(sui::types::Address::from_static("0xc4"));
        let policy = TypeName::new("0x71::commercial_terms::Policy");

        let transaction = collect_invocations_ptb(
            &objects,
            &cashier,
            &admin,
            &policy,
            &[invocation],
            sui::types::Address::from_static("0xc3"),
        )
        .unwrap();

        assert!(transaction.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "commercial_terms"
                    && call.function.as_str() == "collect"
        )));
    }

    #[test]
    fn owner_issuance_publishes_canonical_entitlements() {
        let objects = mock_nexus_objects();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xc3"));

        let credits = issue_finite_credits_ptb(
            &objects,
            &cashier,
            &admin,
            beneficiary.clone(),
            sui::types::Address::from_static("0xc3"),
            5,
        )
        .unwrap();
        assert!(credits.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "finite_credits"
                    && call.function.as_str() == "issue"
        )));
        assert!(credits.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "finite_credits"
                    && call.function.as_str() == "share"
        )));

        let pass = issue_time_pass_ptb(&objects, &cashier, &admin, beneficiary, 10, 20).unwrap();
        assert!(pass.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "time_pass"
                    && call.function.as_str() == "issue"
        )));
        assert!(pass.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call) if call.function.as_str() == "public_freeze_object"
        )));
    }

    #[test]
    fn split_and_refund_claim_share_the_resulting_credits() {
        let objects = mock_nexus_objects();
        let credits = object_ref_for_id(sui::types::Address::from_static("0xc5"));
        let refunded = object_ref_for_id(sui::types::Address::from_static("0xc6"));

        for transaction in [
            split_finite_credits_ptb(&objects, &credits, 1).unwrap(),
            claim_finite_credit_refund_ptb(&objects, &refunded).unwrap(),
        ] {
            assert!(transaction.commands.iter().any(|command| matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == "finite_credits"
                        && call.function.as_str() == "share"
            )));
        }
    }
}
