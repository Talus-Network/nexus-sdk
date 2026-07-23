//! Programmable transaction builders for `nexus_registry::leader`.

use {
    crate::{
        move_bindings::{
            primitives::owner_cap,
            registry::{leader as leader_binding, leader_cap},
        },
        move_boundary,
        sui,
        types::NexusObjects,
    },
    sui::types::ProgrammableTransaction,
};

type OverNetworkCap = owner_cap::CloneableOwnerCap<leader_cap::OverNetwork>;

/// Struct tag for the shared `CloneableOwnerCap<OverNetwork>` capability.
pub fn over_network_cap_struct_tag(objects: &NexusObjects) -> sui::types::StructTag {
    crate::move_bindings::struct_tag::<OverNetworkCap>(objects)
}

/// Register the transaction sender as a leader using part of an owned Talus `$US` coin.
///
/// The coin remains owned by the sender with any balance above `stake_us`.
pub fn register_for_self_ptb(
    objects: &NexusObjects,
    stake_coin: &sui::types::ObjectReference,
    stake_us: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_object(&objects.leader_registry, true)?;
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::{DefaultDagExecutorTarget, UsTokenConfig},
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

    fn nexus_objects() -> NexusObjects {
        NexusObjects {
            workflow_pkg_id: addr("0x1"),
            scheduler_pkg_id: addr("0x11"),
            primitives_pkg_id: addr("0x2"),
            interface_pkg_id: addr("0x3"),
            network_id: addr("0x4"),
            registry_pkg_id: addr("0x5"),
            tool_registry: object_ref("0x6", 1, 6),
            verifier_registry: object_ref("0x7", 1, 7),
            network_auth: object_ref("0x8", 1, 8),
            agent_registry: object_ref("0xc", 1, 12),
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: addr("0xa1"),
                skill_id: 177,
            },
            gas_service: object_ref("0xd", 1, 13),
            leader_registry: object_ref("0xe", 1, 14),
            priority_fee_vault: object_ref("0xf", 1, 15),
            priority_fee_vault_owner_cap: object_ref("0x10", 1, 16),
            us_token: UsTokenConfig::new(addr("0x12")),
            workflow_original_pkg_id: None,
            scheduler_original_pkg_id: None,
        }
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
        assert_eq!(register.package, objects.registry_pkg_id);
        assert_eq!(register.module.as_str(), "leader");
        assert_eq!(register.function.as_str(), "register");
        assert_eq!(register.arguments[1], Argument::Input(1));
        assert_eq!(
            ptb.commands.len(),
            2,
            "the registration coin must remain owned so any unused balance is preserved"
        );
    }
}
