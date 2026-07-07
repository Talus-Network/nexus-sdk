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
    sui_move::MoveType,
    sui_move_call::{CallArg, CallSpecError, CallTarget},
};

type OverNetworkCap = owner_cap::CloneableOwnerCap<leader_cap::OverNetwork>;

fn sui_type_tag(objects: &NexusObjects) -> sui::types::TypeTag {
    crate::move_bindings::type_tag::<sui_binding::SUI>(objects)
}

fn redeem_funds_target<T>() -> Result<CallTarget, CallSpecError>
where
    T: MoveType,
{
    let mut target = CallTarget::new(
        sui::types::Address::from_static("0x2"),
        "coin",
        "redeem_funds",
    )?;
    target.push_type_arg::<T>();
    Ok(target)
}

/// Struct tag for the shared `CloneableOwnerCap<OverNetwork>` capability.
pub fn over_network_cap_struct_tag(objects: &NexusObjects) -> sui::types::StructTag {
    crate::move_bindings::struct_tag::<OverNetworkCap>(objects)
}

/// Register the transaction sender as a leader using sender address balance funds.
pub fn register_for_self_ptb(
    objects: &NexusObjects,
    stake_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_registry = tx.shared_object(&objects.leader_registry, true)?;
        let withdrawal = tx.input(CallArg::FundsWithdrawal(sui::types::FundsWithdrawal::new(
            stake_mist,
            sui_type_tag(objects),
            sui::types::WithdrawFrom::Sender,
        )))?;
        let pay_with = tx.call_target(redeem_funds_target::<sui_binding::SUI>, vec![withdrawal])?;
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::DefaultDagExecutorTarget,
        sui::types::{Argument, Command, MoveCall},
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
    fn register_for_self_redeems_address_balance_funds() {
        let objects = nexus_objects();
        let ptb = register_for_self_ptb(&objects, 1).unwrap();

        let CallArg::FundsWithdrawal(withdrawal) = &ptb.inputs[1] else {
            panic!("expected funds withdrawal input");
        };
        assert_eq!(withdrawal.amount(), Some(1));
        assert_eq!(withdrawal.coin_type(), &sui_type_tag(&objects));
        assert_eq!(withdrawal.source(), sui::types::WithdrawFrom::Sender);

        let redeem = move_call(&ptb.commands[0]);
        assert_eq!(redeem.package, addr("0x2"));
        assert_eq!(redeem.module.as_str(), "coin");
        assert_eq!(redeem.function.as_str(), "redeem_funds");
        assert_eq!(redeem.type_arguments, vec![sui_type_tag(&objects)]);
        assert_eq!(redeem.arguments, vec![Argument::Input(1)]);

        let register = move_call(&ptb.commands[2]);
        assert_eq!(register.module.as_str(), "leader");
        assert_eq!(register.function.as_str(), "register");
        assert_eq!(register.arguments[1], Argument::Result(0));

        let destroy_zero = move_call(&ptb.commands[3]);
        assert_eq!(destroy_zero.module.as_str(), "coin");
        assert_eq!(destroy_zero.function.as_str(), "destroy_zero");
        assert_eq!(destroy_zero.arguments, vec![Argument::Result(0)]);
    }
}
