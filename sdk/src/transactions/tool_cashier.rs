//! Tool cashier policy administration and entitlement purchases.

use crate::{
    move_bindings::{
        interface::payment::{self as payment_binding, PaymentSourceKind},
        move_std::type_name::TypeName,
        sui_framework::{coin as coin_binding, sui::SUI, transfer::Receiving},
        tool::{
            finite_credits as finite_credits_binding,
            invocation::Invocation,
            time_pass as time_pass_binding,
            tool_cashier::{self as tool_cashier_binding, CashierDeposit},
        },
    },
    move_boundary,
    sui,
    types::NexusContext,
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
    objects: &NexusContext,
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

fn destroy_spent_sui_coin(
    transaction: &mut move_boundary::NexusPtbBuilder,
    coin: sui::types::Argument,
) -> anyhow::Result<()> {
    transaction.call_target(coin_binding::destroy_zero_target::<SUI>, vec![coin])?;
    Ok(())
}

/// Constructs a package defined [`PaymentSourceKind`] through its Move API.
///
/// A package defined enum cannot be forged as a pure PTB input. Calling its
/// constructors preserves the type boundary enforced by Move.
fn payment_source_kind_arg(
    transaction: &mut move_boundary::NexusPtbBuilder,
    source: &PaymentSourceKind,
) -> anyhow::Result<sui::types::Argument> {
    match source {
        PaymentSourceKind::UserFunded { user } => {
            let user = transaction.arg(user)?;
            transaction.call_target(
                payment_binding::payment_source_kind_user_funded_target,
                vec![user],
            )
        }
        PaymentSourceKind::AgentFunded { agent_id } => {
            let agent_id = transaction.object_id(agent_id.bytes)?;
            transaction.call_target(
                payment_binding::payment_source_kind_agent_funded_target,
                vec![agent_id],
            )
        }
    }
}

/// Enables finite credit sales and admission.
pub fn enable_finite_credits_ptb(
    objects: &NexusContext,
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

/// Closes finite credit issuance without invalidating existing accounts.
pub fn close_finite_credit_issuance_ptb(
    objects: &NexusContext,
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
    objects: &NexusContext,
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
    objects: &NexusContext,
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

/// Purchases and shares the first canonical finite credit account.
pub fn buy_finite_credits_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    credits: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, true)?;
        let coin = transaction.owned_object(pay_with)?;
        let beneficiary = payment_source_kind_arg(transaction, &beneficiary)?;
        let credits_count = transaction.arg(&credits)?;
        transaction.call_target(
            finite_credits_binding::buy_target,
            vec![cashier, coin, beneficiary, credits_count],
        )?;
        Ok(())
    })
}

/// Purchases units for an existing canonical finite credit account.
pub fn buy_more_finite_credits_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    credits: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    additional_credits: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let credits = transaction.shared_object(credits, true)?;
        let coin = transaction.owned_object(pay_with)?;
        let additional_credits = transaction.arg(&additional_credits)?;
        transaction.call_target(
            finite_credits_binding::buy_more_target,
            vec![cashier, credits, coin, additional_credits],
        )?;
        Ok(())
    })
}

/// Purchases finite credits with SUI withdrawn from the sender address balance.
///
/// The optional account selects whether the transaction creates the canonical
/// account or adds units to the account that already occupies that slot.
pub fn buy_finite_credits_from_balance_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    account: Option<&sui::types::ObjectReference>,
    beneficiary: PaymentSourceKind,
    credits: u64,
    price: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let coin = transaction.withdraw_sui_coin(price)?;
        match account {
            Some(account) => {
                let cashier = transaction.shared_object(tool_cashier, false)?;
                let account = transaction.shared_object(account, true)?;
                let credits = transaction.arg(&credits)?;
                transaction.call_target(
                    finite_credits_binding::buy_more_target,
                    vec![cashier, account, coin, credits],
                )?;
            }
            None => {
                let cashier = transaction.shared_object(tool_cashier, true)?;
                let beneficiary = payment_source_kind_arg(transaction, &beneficiary)?;
                let credits = transaction.arg(&credits)?;
                transaction.call_target(
                    finite_credits_binding::buy_target,
                    vec![cashier, coin, beneficiary, credits],
                )?;
            }
        }
        destroy_spent_sui_coin(transaction, coin)
    })
}

/// Issues and shares the first canonical finite credit account.
pub fn issue_finite_credits_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    credits: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, true)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let beneficiary = payment_source_kind_arg(transaction, &beneficiary)?;
        let credits = transaction.arg(&credits)?;
        let issued = transaction.call_target(
            finite_credits_binding::issue_target,
            vec![cashier, admin, beneficiary, credits],
        )?;
        transaction.call_target(finite_credits_binding::share_target, vec![issued])?;
        Ok(())
    })
}

/// Adds an owner grant to an existing canonical finite credit account.
pub fn issue_more_finite_credits_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    credits: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    additional_credits: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let credits = transaction.shared_object(credits, true)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let additional_credits = transaction.arg(&additional_credits)?;
        transaction.call_target(
            finite_credits_binding::issue_more_target,
            vec![cashier, credits, admin, additional_credits],
        )?;
        Ok(())
    })
}

/// Enables time pass sales and admission.
pub fn enable_time_pass_ptb(
    objects: &NexusContext,
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

/// Closes time pass issuance without invalidating existing accounts.
pub fn close_time_pass_issuance_ptb(
    objects: &NexusContext,
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
    objects: &NexusContext,
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
    objects: &NexusContext,
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

/// Purchases and shares the first canonical time pass account.
pub fn buy_time_pass_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    duration_ms: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, true)?;
        let coin = transaction.owned_object(pay_with)?;
        let beneficiary = payment_source_kind_arg(transaction, &beneficiary)?;
        let duration = transaction.arg(&duration_ms)?;
        let clock = transaction.clock()?;
        transaction.call_target(
            time_pass_binding::buy_target,
            vec![cashier, coin, beneficiary, duration, clock],
        )?;
        Ok(())
    })
}

/// Purchases duration for an existing canonical time pass account.
pub fn buy_more_time_pass_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    pass: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    duration_ms: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let pass = transaction.shared_object(pass, true)?;
        let coin = transaction.owned_object(pay_with)?;
        let duration = transaction.arg(&duration_ms)?;
        let clock = transaction.clock()?;
        transaction.call_target(
            time_pass_binding::buy_more_target,
            vec![cashier, pass, coin, duration, clock],
        )?;
        Ok(())
    })
}

/// Purchases time pass duration with SUI withdrawn from the sender address balance.
///
/// The optional account selects whether the transaction creates the canonical
/// account or extends the account that already occupies that slot.
pub fn buy_time_pass_from_balance_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    account: Option<&sui::types::ObjectReference>,
    beneficiary: PaymentSourceKind,
    duration_ms: u64,
    price: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let coin = transaction.withdraw_sui_coin(price)?;
        let clock = transaction.clock()?;
        match account {
            Some(account) => {
                let cashier = transaction.shared_object(tool_cashier, false)?;
                let account = transaction.shared_object(account, true)?;
                let duration = transaction.arg(&duration_ms)?;
                transaction.call_target(
                    time_pass_binding::buy_more_target,
                    vec![cashier, account, coin, duration, clock],
                )?;
            }
            None => {
                let cashier = transaction.shared_object(tool_cashier, true)?;
                let beneficiary = payment_source_kind_arg(transaction, &beneficiary)?;
                let duration = transaction.arg(&duration_ms)?;
                transaction.call_target(
                    time_pass_binding::buy_target,
                    vec![cashier, coin, beneficiary, duration, clock],
                )?;
            }
        }
        destroy_spent_sui_coin(transaction, coin)
    })
}

/// Issues and shares the first canonical time pass account.
pub fn issue_time_pass_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    beneficiary: PaymentSourceKind,
    valid_from_ms: u64,
    valid_until_ms: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, true)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let beneficiary = payment_source_kind_arg(transaction, &beneficiary)?;
        let valid_from_ms = transaction.arg(&valid_from_ms)?;
        let valid_until_ms = transaction.arg(&valid_until_ms)?;
        let pass = transaction.call_target(
            time_pass_binding::issue_target,
            vec![cashier, admin, beneficiary, valid_from_ms, valid_until_ms],
        )?;
        transaction.call_target(time_pass_binding::share_target, vec![pass])?;
        Ok(())
    })
}

/// Replaces an existing canonical time pass window under owner authority.
pub fn update_time_pass_window_ptb(
    objects: &NexusContext,
    tool_cashier: &sui::types::ObjectReference,
    pass: &sui::types::ObjectReference,
    cashier_admin: &sui::types::ObjectReference,
    valid_from_ms: u64,
    valid_until_ms: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(tool_cashier, false)?;
        let pass = transaction.shared_object(pass, true)?;
        let admin = transaction.owned_object(cashier_admin)?;
        let valid_from_ms = transaction.arg(&valid_from_ms)?;
        let valid_until_ms = transaction.arg(&valid_until_ms)?;
        transaction.call_target(
            time_pass_binding::update_window_target,
            vec![cashier, pass, admin, valid_from_ms, valid_until_ms],
        )?;
        Ok(())
    })
}

/// Restores a refunded Invocation to its exact finite credit account.
pub fn restore_finite_credit_refund_ptb(
    objects: &NexusContext,
    credits: &sui::types::ObjectReference,
    refunded: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let credits = transaction.shared_object(credits, true)?;
        let refunded = transaction.receiving_object::<Invocation>(refunded)?;
        transaction.call_target(
            finite_credits_binding::restore_refund_target,
            vec![credits, refunded],
        )?;
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
    objects: &NexusContext,
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
    objects: &NexusContext,
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
            test_utils::sui_mocks::{mock_nexus_context, object_ref_for_id},
            transactions::invocation::InvocationPolicyCall,
        },
        sui_sdk_types::Command,
    };

    fn assert_move_call(
        transaction: &sui::types::ProgrammableTransaction,
        module: &str,
        function: &str,
    ) {
        assert!(transaction.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == module && call.function.as_str() == function
        )));
    }

    #[test]
    fn collection_rejects_an_empty_invocation_batch() {
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let policy = InvocationPolicyCall::fixed_price(&objects).unwrap().policy;

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
        let objects = mock_nexus_context();
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
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xc3"));

        let credits =
            issue_finite_credits_ptb(&objects, &cashier, &admin, beneficiary.clone(), 5).unwrap();
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
            Command::MoveCall(call)
                if call.module.as_str() == "time_pass" && call.function.as_str() == "share"
        )));
    }

    #[test]
    fn purchases_withdraw_exact_sui_from_the_sender_balance() {
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xc3"));

        let credits = buy_finite_credits_from_balance_ptb(
            &objects,
            &cashier,
            None,
            beneficiary.clone(),
            5,
            35,
        )
        .unwrap();
        let pass =
            buy_time_pass_from_balance_ptb(&objects, &cashier, None, beneficiary, 10, 20).unwrap();

        for (transaction, policy) in [(credits, "finite_credits"), (pass, "time_pass")] {
            assert_eq!(
                transaction
                    .inputs
                    .iter()
                    .filter(|input| matches!(input, sui::types::Input::FundsWithdrawal(_)))
                    .count(),
                1,
            );
            assert!(transaction.commands.iter().any(|command| matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == "payment"
                        && call.function.as_str() == "payment_source_kind_user_funded"
            )));
            assert!(transaction.commands.iter().any(|command| matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == policy && call.function.as_str() == "buy"
            )));
            assert!(!transaction.commands.iter().any(|command| matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == policy && call.function.as_str() == "share"
            )));
            assert!(transaction.commands.iter().any(|command| matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == "coin" && call.function.as_str() == "destroy_zero"
            )));
        }
    }

    #[test]
    fn agent_beneficiary_is_constructed_through_the_payment_module() {
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let beneficiary = PaymentSourceKind::agent_funded(sui::types::Address::from_static("0xa1"));

        let transaction =
            buy_finite_credits_from_balance_ptb(&objects, &cashier, None, beneficiary, 5, 35)
                .unwrap();

        assert!(transaction.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "object"
                    && call.function.as_str() == "id_from_address"
        )));
        assert!(transaction.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "payment"
                    && call.function.as_str() == "payment_source_kind_agent_funded"
        )));
    }

    #[test]
    fn existing_accounts_are_updated_without_creating_another_account() {
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let credits = object_ref_for_id(sui::types::Address::from_static("0xc5"));
        let pass = object_ref_for_id(sui::types::Address::from_static("0xc6"));
        let coin = object_ref_for_id(sui::types::Address::from_static("0xc7"));

        let credits_tx =
            issue_more_finite_credits_ptb(&objects, &cashier, &credits, &admin, 2).unwrap();
        assert!(credits_tx.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "finite_credits"
                    && call.function.as_str() == "issue_more"
        )));

        let pass_tx = buy_more_time_pass_ptb(&objects, &cashier, &pass, &coin, 10).unwrap();
        assert!(pass_tx.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "time_pass" && call.function.as_str() == "buy_more"
        )));
    }

    #[test]
    fn refunded_invocation_is_restored_into_the_exact_account() {
        let objects = mock_nexus_context();
        let credits = object_ref_for_id(sui::types::Address::from_static("0xc5"));
        let refunded = object_ref_for_id(sui::types::Address::from_static("0xc6"));

        let transaction = restore_finite_credit_refund_ptb(&objects, &credits, &refunded).unwrap();

        assert!(transaction.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "finite_credits"
                    && call.function.as_str() == "restore_refund"
        )));
    }

    #[test]
    fn policy_administration_builders_call_the_exact_policy_functions() {
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));

        let cases = [
            (
                enable_finite_credits_ptb(&objects, &cashier, &admin, 2, 1, 10).unwrap(),
                "finite_credits",
                "enable",
            ),
            (
                close_finite_credit_issuance_ptb(&objects, &cashier, &admin).unwrap(),
                "finite_credits",
                "close_issuance",
            ),
            (
                open_finite_credit_issuance_ptb(&objects, &cashier, &admin).unwrap(),
                "finite_credits",
                "open_issuance",
            ),
            (
                update_finite_credit_terms_ptb(&objects, &cashier, &admin, 3, 2, 20).unwrap(),
                "finite_credits",
                "update_terms",
            ),
            (
                enable_time_pass_ptb(&objects, &cashier, &admin, 2, 1, 10).unwrap(),
                "time_pass",
                "enable",
            ),
            (
                close_time_pass_issuance_ptb(&objects, &cashier, &admin).unwrap(),
                "time_pass",
                "close_issuance",
            ),
            (
                open_time_pass_issuance_ptb(&objects, &cashier, &admin).unwrap(),
                "time_pass",
                "open_issuance",
            ),
            (
                update_time_pass_terms_ptb(&objects, &cashier, &admin, 3, 2, 20).unwrap(),
                "time_pass",
                "update_terms",
            ),
        ];

        for (transaction, module, function) in cases {
            assert_move_call(&transaction, module, function);
        }
    }

    #[test]
    fn direct_coin_purchase_builders_cover_new_and_existing_accounts() {
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let credits = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let pass = object_ref_for_id(sui::types::Address::from_static("0xc3"));
        let coin = object_ref_for_id(sui::types::Address::from_static("0xc4"));
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xc5"));

        let new_credits =
            buy_finite_credits_ptb(&objects, &cashier, &coin, beneficiary.clone(), 4).unwrap();
        let more_credits =
            buy_more_finite_credits_ptb(&objects, &cashier, &credits, &coin, 4).unwrap();
        let new_pass =
            buy_time_pass_ptb(&objects, &cashier, &coin, beneficiary.clone(), 40).unwrap();
        let more_pass = buy_more_time_pass_ptb(&objects, &cashier, &pass, &coin, 40).unwrap();
        let balance_credits = buy_finite_credits_from_balance_ptb(
            &objects,
            &cashier,
            Some(&credits),
            beneficiary.clone(),
            4,
            8,
        )
        .unwrap();
        let balance_pass =
            buy_time_pass_from_balance_ptb(&objects, &cashier, Some(&pass), beneficiary, 40, 80)
                .unwrap();

        assert_move_call(&new_credits, "finite_credits", "buy");
        assert_move_call(&more_credits, "finite_credits", "buy_more");
        assert_move_call(&new_pass, "time_pass", "buy");
        assert_move_call(&more_pass, "time_pass", "buy_more");
        assert_move_call(&balance_credits, "finite_credits", "buy_more");
        assert_move_call(&balance_pass, "time_pass", "buy_more");
    }

    #[test]
    fn existing_pass_and_deposit_collection_builders_preserve_their_sources() {
        let objects = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0xc1"));
        let admin = object_ref_for_id(sui::types::Address::from_static("0xc2"));
        let pass = object_ref_for_id(sui::types::Address::from_static("0xc3"));
        let deposit = object_ref_for_id(sui::types::Address::from_static("0xc4"));

        let update =
            update_time_pass_window_ptb(&objects, &cashier, &pass, &admin, 10, 20).unwrap();
        assert_move_call(&update, "time_pass", "update_window");

        let collection = collect_deposits_ptb(
            &objects,
            &cashier,
            &admin,
            &[deposit],
            sui::types::Address::from_static("0xc5"),
        )
        .unwrap();
        assert_move_call(&collection, "tool_cashier", "collect_deposits");

        let error = collect_deposits_ptb(
            &objects,
            &cashier,
            &admin,
            &[],
            sui::types::Address::from_static("0xc5"),
        )
        .expect_err("empty deposit collection must fail locally");
        assert!(error.to_string().contains("at least one deposit"));
    }
}
