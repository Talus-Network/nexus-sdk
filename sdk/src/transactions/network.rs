//! Nexus network fee transactions.

use crate::{
    move_bindings::{
        registry::priority_fee_vault::{self as priority_fee_vault_binding, PriorityFeeDeposit},
        sui_framework::transfer::Receiving,
    },
    move_boundary,
    sui,
    types::NexusContext,
};

/// Receive and account for one nonempty batch of priority fee deposit children.
pub fn collect_priority_fee_deposits(
    context: &NexusContext,
    deposits: &[sui::types::ObjectReference],
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    if deposits.is_empty() {
        anyhow::bail!("priority fee deposit collection requires at least one deposit");
    }

    move_boundary::ptb(context, |transaction| {
        let vault = transaction.shared_root(&context.priority_fee_vault, true)?;
        let leader_registry = transaction.shared_root(&context.leader_registry, false)?;
        let deposits = deposits
            .iter()
            .map(|deposit| transaction.receiving_object::<PriorityFeeDeposit>(deposit))
            .collect::<Result<Vec<_>, _>>()?;
        let deposits = transaction.move_vector::<Receiving<PriorityFeeDeposit>>(deposits)?;
        transaction.call_target(
            priority_fee_vault_binding::collect_deposits_target,
            vec![vault, leader_registry, deposits],
        )?;
        Ok(())
    })
}

/// Configure the `$US` priority fee vault exchange rate.
pub fn configure_priority_fee_vault(
    context: &NexusContext,
    owner_cap: &sui::types::ObjectReference,
    exchange_rate_million_mists_us: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(context, |transaction| {
        let vault = transaction.shared_root(&context.priority_fee_vault, true)?;
        let owner_cap = transaction.owned_object(owner_cap)?;
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
    context: &NexusContext,
    us_coin: &sui::types::ObjectReference,
    min_sui_out: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(context, |transaction| {
        let vault = transaction.shared_root(&context.priority_fee_vault, true)?;
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
    context: &NexusContext,
    leader_cap: &sui::types::ObjectReference,
    leader_cap_owner: &sui::types::Owner,
    share_to_withdraw: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(context, |transaction| {
        let vault = transaction.shared_root(&context.priority_fee_vault, true)?;
        let leader_registry = transaction.shared_root(&context.leader_registry, false)?;
        let leader_cap = transaction.object_from_owner(leader_cap, leader_cap_owner, false)?;
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::test_utils::sui_mocks,
        sui::types::{Command, Input},
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn object_ref(value: &'static str, version: u64, digest: u8) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(
            address(value),
            version,
            sui::types::Digest::from([digest; 32]),
        )
    }

    fn nexus_objects() -> crate::types::NexusContext {
        sui_mocks::mock_nexus_context()
    }

    #[test]
    fn priority_fee_collection_uses_typed_receiving_inputs() {
        let objects = nexus_objects();
        let deposits = vec![object_ref("0x20", 7, 20), object_ref("0x21", 8, 21)];

        let ptb = collect_priority_fee_deposits(&objects, &deposits).expect("collection PTB");

        let receiving = ptb
            .inputs
            .iter()
            .filter_map(|input| match input {
                Input::Receiving(reference) => Some(reference),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(receiving, deposits.iter().collect::<Vec<_>>());
        assert!(ptb.inputs.iter().any(|input| {
            matches!(
                input,
                Input::Shared(shared)
                    if shared.object_id() == objects.priority_fee_vault.object_id()
                        && shared.mutability().is_mutable()
            )
        }));
        assert!(ptb.inputs.iter().any(|input| {
            matches!(
                input,
                Input::Shared(shared)
                    if shared.object_id() == objects.leader_registry.object_id()
                        && !shared.mutability().is_mutable()
            )
        }));

        let vector = ptb
            .commands
            .iter()
            .find_map(|command| match command {
                Command::MakeMoveVector(vector) => Some(vector),
                _ => None,
            })
            .expect("typed receiving vector");
        assert_eq!(
            vector.type_,
            Some(crate::move_bindings::type_tag::<
                Receiving<PriorityFeeDeposit>,
            >(&objects))
        );
        assert_eq!(vector.elements.len(), 2);

        let call = ptb
            .commands
            .iter()
            .find_map(|command| match command {
                Command::MoveCall(call)
                    if call.module.as_str() == "priority_fee_vault"
                        && call.function.as_str() == "collect_deposits" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("collection Move call");
        assert_eq!(call.arguments.len(), 3);
    }

    #[test]
    fn priority_fee_collection_rejects_an_empty_batch() {
        let error = collect_priority_fee_deposits(&nexus_objects(), &[])
            .expect_err("empty batch must be rejected");
        assert!(error.to_string().contains("at least one deposit"));
    }

    #[test]
    fn priority_fee_withdrawal_preserves_consensus_address_ownership() {
        let objects = nexus_objects();
        let leader_cap = object_ref("0x30", 9, 30);
        let leader_cap_owner = sui::types::Owner::ConsensusAddress {
            start_version: 4,
            owner: address("0x31"),
        };

        let ptb =
            withdraw_priority_fee(&objects, &leader_cap, &leader_cap_owner, 5, address("0x31"))
                .expect("consensus address owned cap is a valid input");

        assert!(ptb.inputs.iter().any(|input| {
            matches!(
                input,
                Input::Shared(shared)
                    if shared.object_id() == *leader_cap.object_id()
                        && shared.version() == 4
                        && !shared.mutability().is_mutable()
            )
        }));
    }
}
