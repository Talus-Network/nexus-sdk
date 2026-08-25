//! Programmable transaction builders for `nexus_registry::leader`.

use {
    crate::{
        move_bindings::{
            primitives::owner_cap,
            registry::{leader as leader_binding, leader_cap},
        },
        move_boundary,
        sui,
        types::NexusContext,
    },
    sui::types::ProgrammableTransaction,
};

type OverNetworkCap = owner_cap::CloneableOwnerCap<leader_cap::OverNetwork>;

/// Struct tag for the shared `CloneableOwnerCap<OverNetwork>` capability.
pub fn over_network_cap_struct_tag(objects: &NexusContext) -> sui::types::StructTag {
    crate::move_bindings::struct_tag::<OverNetworkCap>(objects)
}

/// Configure every leader registry policy value in one transaction.
///
/// Keeping these updates atomic prevents registration or selection from observing
/// a mixture of old and new protocol policy. The owner description is required
/// because the administration capability may use either address or consensus
/// ownership.
pub fn configure_registry_ptb(
    objects: &NexusContext,
    admin_cap: &sui::types::ObjectReference,
    admin_owner: &sui::types::Owner,
    unbonding_duration_ms: u64,
    min_stake_us: u64,
    max_transaction_budget_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_root(&objects.leader_registry, true)?;
        let admin_cap = tx.object_from_owner(admin_cap, admin_owner, true)?;
        let unbonding_duration_ms = tx.arg(&unbonding_duration_ms)?;
        let min_stake_us = tx.arg(&min_stake_us)?;
        let max_transaction_budget_mist = tx.arg(&max_transaction_budget_mist)?;

        tx.call_target(
            leader_binding::set_unbonding_duration_ms_target,
            vec![leader_registry, admin_cap, unbonding_duration_ms],
        )?;
        tx.call_target(
            leader_binding::set_min_stake_us_target,
            vec![leader_registry, admin_cap, min_stake_us],
        )?;
        tx.call_target(
            leader_binding::set_max_transaction_budget_target,
            vec![leader_registry, admin_cap, max_transaction_budget_mist],
        )?;
        Ok(())
    })
}

/// Register the transaction sender as a leader using part of an owned Talus `$US` coin.
///
/// The coin remains owned by the sender with any balance above `stake_us`.
pub fn register_for_self_ptb(
    objects: &NexusContext,
    stake_coin: &sui::types::ObjectReference,
    stake_us: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_root(&objects.leader_registry, true)?;
        let pay_with = tx.owned_object(stake_coin)?;
        let amount = tx.arg(&stake_us)?;
        let metadata = tx.call_target(leader_binding::empty_metadata_target, vec![])?;
        let clock = tx.clock()?;

        tx.call_target(
            leader_binding::register_target,
            vec![leader_registry, pay_with, amount, metadata, clock],
        )?;

        Ok(())
    })
}

/// Activate this leader and claim ownership with the transaction digest token.
pub fn activate_and_claim_for_self_ptb(
    objects: &NexusContext,
    leader_cap_over_network: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_root(&objects.leader_registry, true)?;
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
    objects: &NexusContext,
    leader_cap_over_network: &sui::types::ObjectReference,
    token: Vec<u8>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_root(&objects.leader_registry, true)?;
        let leader_cap = tx.shared_object(leader_cap_over_network, false)?;
        let token = tx.arg(&token)?;

        tx.call_target(
            leader_binding::suspend_if_token_target,
            vec![leader_registry, leader_cap, token],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{test_utils::sui_mocks, types::PackageRole},
        sui::types::{Argument, Command, Input, MoveCall},
    };

    fn addr(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn object_ref(value: &'static str, version: u64, digest: u8) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(
            addr(value),
            version,
            sui::types::Digest::from([digest; 32]),
        )
    }

    fn nexus_objects() -> crate::types::NexusContext {
        sui_mocks::mock_nexus_context()
    }

    fn move_call(command: &Command) -> &MoveCall {
        let Command::MoveCall(call) = command else {
            panic!("expected MoveCall command");
        };
        call
    }

    #[test]
    fn register_for_self_preserves_the_stake_coin_after_registration() {
        let objects = nexus_objects();
        let stake_coin = object_ref("0x20", 2, 20);
        let ptb = register_for_self_ptb(&objects, &stake_coin, 3).unwrap();

        let Input::ImmutableOrOwned(input_stake_coin) = &ptb.inputs[1] else {
            panic!("expected owned US stake coin input");
        };
        assert_eq!(input_stake_coin, &stake_coin);

        let register = move_call(&ptb.commands[1]);
        assert_eq!(
            register.package,
            objects
                .require_package(PackageRole::Registry)
                .unwrap()
                .storage_id
        );
        assert_eq!(register.module.as_str(), "leader");
        assert_eq!(register.function.as_str(), "register");
        assert_eq!(register.arguments.len(), 5);
        assert_eq!(register.arguments[1], Argument::Input(1));
        assert_eq!(
            ptb.commands.len(),
            2,
            "the registration coin must remain owned so any unused balance is preserved"
        );
    }

    #[test]
    fn registry_configuration_is_atomic_and_preserves_admin_ownership() {
        let objects = nexus_objects();
        let admin_cap = object_ref("0x30", 9, 30);
        let admin_owner = sui::types::Owner::ConsensusAddress {
            start_version: 4,
            owner: addr("0x31"),
        };

        let ptb = configure_registry_ptb(
            &objects,
            &admin_cap,
            &admin_owner,
            86_400_000,
            1_000_000_000,
            10_000_000_000,
        )
        .expect("leader registry configuration PTB");

        assert!(ptb.inputs.iter().any(|input| {
            matches!(
                input,
                Input::Shared(shared)
                    if shared.object_id() == *admin_cap.object_id()
                        && shared.version() == 4
                        && shared.mutability().is_mutable()
            )
        }));
        let calls = ptb
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::MoveCall(call) if call.module.as_str() == "leader" => {
                    Some(call.function.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls,
            [
                "set_unbonding_duration_ms",
                "set_min_stake_us",
                "set_max_transaction_budget",
            ]
        );
    }
}
