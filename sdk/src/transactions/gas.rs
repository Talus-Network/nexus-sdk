use {
    crate::{
        move_bindings::{
            registry::priority_fee_vault as priority_fee_vault_binding,
            workflow::{gas as gas_binding, gas_extension as gas_extension_binding},
        },
        move_boundary,
        sui,
        types::NexusObjects,
    },
    sui::types::{Argument, ProgrammableTransaction},
};

fn tool_and_owner_args(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<Vec<Argument>> {
    Ok(vec![
        tx.shared_object(tool_gas, true)?,
        tx.shared_object(tool, false)?,
        tx.owned_object(owner_cap)?,
    ])
}

fn tool_ticket_args(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    amount: u64,
    pay_with: &sui::types::ObjectReference,
) -> anyhow::Result<Vec<Argument>> {
    Ok(vec![
        tx.shared_object(tool_gas, true)?,
        tx.shared_object(tool, false)?,
        tx.arg(&amount)?,
        tx.owned_object(pay_with)?,
        tx.clock()?,
    ])
}

/// Build a PTB to enable the expiry gas extension for a tool.
pub(crate) fn enable_expiry_ptb(
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    cost_per_minute: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let mut args = tool_and_owner_args(tx, tool_gas, tool, owner_cap)?;
        args.push(tx.arg(&cost_per_minute)?);

        tx.call_target(gas_extension_binding::enable_expiry_target, args)?;
        Ok(())
    })
}

/// Build a PTB to disable the expiry gas extension for a tool.
pub(crate) fn disable_expiry_ptb(
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let args = tool_and_owner_args(tx, tool_gas, tool, owner_cap)?;

        tx.call_target(gas_extension_binding::disable_expiry_target, args)?;
        Ok(())
    })
}

/// Build a PTB to buy an expiry gas ticket.
pub(crate) fn buy_expiry_gas_ticket_ptb(
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    minutes: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let args = tool_ticket_args(tx, tool_gas, tool, minutes, pay_with)?;

        tx.call_target(gas_extension_binding::buy_expiry_gas_ticket_target, args)?;
        Ok(())
    })
}

/// PTB template to snapshot all DAG tool costs into the execution payment.
#[allow(clippy::too_many_arguments)]
pub(crate) fn snapshot_dag_tool_costs(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    gas_service: sui::types::Argument,
    execution: sui::types::Argument,
    dag: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(
        gas_binding::snapshot_dag_tool_costs_target,
        vec![gas_service, execution, dag],
    )
}

/// PTB template to settle payment for a vertex using pending DAG settlement
/// state when present.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_payment_state_for_vertex(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool_gas: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    expected_vertex: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(
        gas_binding::settle_payment_state_for_vertex_target,
        vec![tool_gas, dag, execution, expected_vertex],
    )
}

/// PTB template to abort an expired execution by first unlocking/refunding the
/// matching ToolGas vertex payment.
#[allow(clippy::too_many_arguments)]
pub fn abort_expired_execution_with_tool_gas_ptb(
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool_gas = tx.shared_object(tool_gas, true)?;
        let dag = tx.shared_object(dag, false)?;
        let execution = tx.shared_object(execution, true)?;
        let clock = tx.clock()?;

        tx.call_target(
            gas_extension_binding::abort_expired_execution_with_tool_gas_target,
            vec![tool_gas, dag, execution, clock],
        )?;
        Ok(())
    })
}

/// PTB template to configure the `$US` priority fee vault exchange rate.
pub fn configure_priority_fee_vault(
    objects: &NexusObjects,
    exchange_rate_sui_us: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let priority_fee_vault = tx.shared_object(&objects.priority_fee_vault, true)?;
        let owner_cap = tx.owned_object(&objects.priority_fee_vault_owner_cap)?;
        let exchange_rate_sui_us = tx.arg(&exchange_rate_sui_us)?;

        tx.call_target(
            priority_fee_vault_binding::configure_target,
            vec![priority_fee_vault, owner_cap, exchange_rate_sui_us],
        )?;
        Ok(())
    })
}

/// PTB template to swap an owned `Coin<US>` for vault SUI.
pub fn swap_us_for_sui(
    objects: &NexusObjects,
    us_coin: &sui::types::ObjectReference,
    min_sui_out: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let priority_fee_vault = tx.shared_object(&objects.priority_fee_vault, true)?;
        let us_coin = tx.owned_object(us_coin)?;
        let min_sui_out = tx.arg(&min_sui_out)?;
        let result = tx.call_target(
            priority_fee_vault_binding::swap_us_for_sui_target,
            vec![priority_fee_vault, us_coin, min_sui_out],
        )?;
        let sui_out = tx.nested_result(result, 0)?;
        let us_refund = tx.nested_result(result, 1)?;
        let recipient = tx.arg(&recipient)?;
        tx.transfer_objects(vec![sui_out, us_refund], recipient)?;
        Ok(())
    })
}

/// PTB template to withdraw a leader's priority-fee share from the registry vault.
pub fn withdraw_priority_fee(
    objects: &NexusObjects,
    leader_cap: &sui::types::ObjectReference,
    share_to_withdraw: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let priority_fee_vault = tx.shared_object(&objects.priority_fee_vault, true)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let leader_cap = tx.owned_object(leader_cap)?;
        let share_to_withdraw = tx.arg(&share_to_withdraw)?;
        let us_out = tx.call_target(
            priority_fee_vault_binding::withdraw_priority_fee_target,
            vec![
                priority_fee_vault,
                leader_registry,
                leader_cap,
                share_to_withdraw,
            ],
        )?;
        let recipient = tx.arg(&recipient)?;
        tx.transfer_objects(vec![us_out], recipient)?;
        Ok(())
    })
}

/// PTB template to refund payment settlement for a vertex.
#[allow(clippy::too_many_arguments)]
pub(crate) fn enable_limited_invocations_ptb(
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    cost_per_invocation: u64,
    min_invocations: u64,
    max_invocations: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let mut args = tool_and_owner_args(tx, tool_gas, tool, owner_cap)?;
        args.push(tx.arg(&cost_per_invocation)?);
        args.push(tx.arg(&min_invocations)?);
        args.push(tx.arg(&max_invocations)?);

        tx.call_target(
            gas_extension_binding::enable_limited_invocations_target,
            args,
        )?;
        Ok(())
    })
}

/// Build a PTB to disable the limited invocations gas extension for a tool.
pub(crate) fn disable_limited_invocations_ptb(
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let args = tool_and_owner_args(tx, tool_gas, tool, owner_cap)?;

        tx.call_target(
            gas_extension_binding::disable_limited_invocations_target,
            args,
        )?;
        Ok(())
    })
}

/// Build a PTB to buy a limited invocations gas ticket.
pub(crate) fn buy_limited_invocations_gas_ticket_ptb(
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    invocations: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let args = tool_ticket_args(tx, tool_gas, tool, invocations, pay_with)?;

        tx.call_target(
            gas_extension_binding::buy_limited_invocations_gas_ticket_target,
            args,
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
            panic!("expected MoveCall command, got {command:?}");
        };
        call
    }

    fn input_for_argument<'a>(ptb: &'a ProgrammableTransaction, argument: &Argument) -> &'a Input {
        let Argument::Input(index) = argument else {
            panic!("expected input argument, got {argument:?}");
        };
        &ptb.inputs[*index as usize]
    }

    fn assert_shared(
        ptb: &ProgrammableTransaction,
        argument: &Argument,
        expected: &sui::types::ObjectReference,
        mutable: bool,
    ) {
        let Input::Shared(shared) = input_for_argument(ptb, argument) else {
            panic!("expected shared object input");
        };
        assert_eq!(shared.object_id(), *expected.object_id());
        assert_eq!(shared.version(), expected.version());
        assert_eq!(shared.mutability().is_mutable(), mutable);
    }

    fn assert_owned(
        ptb: &ProgrammableTransaction,
        argument: &Argument,
        expected: &sui::types::ObjectReference,
    ) {
        let Input::ImmutableOrOwned(actual) = input_for_argument(ptb, argument) else {
            panic!("expected owned object input");
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn abort_expired_execution_with_tool_gas_uses_generated_target() {
        let objects = nexus_objects();
        let tool_gas = object_ref("0x20", 2, 20);
        let dag = object_ref("0x21", 3, 21);
        let execution = object_ref("0x22", 4, 22);
        let ptb = abort_expired_execution_with_tool_gas_ptb(&objects, &tool_gas, &dag, &execution)
            .unwrap();

        let call = move_call(ptb.commands.last().unwrap());
        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module.as_str(), "gas_extension");
        assert_eq!(
            call.function.as_str(),
            "abort_expired_execution_with_tool_gas"
        );
        assert_shared(&ptb, &call.arguments[0], &tool_gas, true);
        assert_shared(&ptb, &call.arguments[1], &dag, false);
        assert_shared(&ptb, &call.arguments[2], &execution, true);
    }

    #[test]
    fn configure_priority_fee_vault_uses_generated_registry_target() {
        let objects = nexus_objects();
        let ptb = configure_priority_fee_vault(&objects, 77).unwrap();

        let call = move_call(&ptb.commands[0]);
        assert_eq!(call.package, objects.registry_pkg_id);
        assert_eq!(call.module.as_str(), "priority_fee_vault");
        assert_eq!(call.function.as_str(), "configure");
        assert_eq!(call.arguments.len(), 3);
        assert_shared(&ptb, &call.arguments[0], &objects.priority_fee_vault, true);
        assert_owned(
            &ptb,
            &call.arguments[1],
            &objects.priority_fee_vault_owner_cap,
        );
        assert_eq!(
            input_for_argument(&ptb, &call.arguments[2]),
            &Input::Pure(77u64.to_le_bytes().to_vec())
        );
    }

    #[test]
    fn swap_us_for_sui_uses_generated_target_and_transfers_both_results() {
        let objects = nexus_objects();
        let us_coin = object_ref("0x20", 2, 20);
        let recipient = addr("0x99");
        let ptb = swap_us_for_sui(&objects, &us_coin, 42, recipient).unwrap();

        let call = move_call(&ptb.commands[0]);
        assert_eq!(call.package, objects.registry_pkg_id);
        assert_eq!(call.module.as_str(), "priority_fee_vault");
        assert_eq!(call.function.as_str(), "swap_us_for_sui");
        assert_eq!(call.arguments.len(), 3);
        assert_shared(&ptb, &call.arguments[0], &objects.priority_fee_vault, true);
        assert_owned(&ptb, &call.arguments[1], &us_coin);
        assert!(matches!(&ptb.commands[1], Command::TransferObjects(_)));
    }

    #[test]
    fn withdraw_priority_fee_uses_generated_target_and_transfers_us() {
        let objects = nexus_objects();
        let leader_cap = object_ref("0x20", 2, 20);
        let ptb = withdraw_priority_fee(&objects, &leader_cap, 55, addr("0x99")).unwrap();

        let call = move_call(&ptb.commands[0]);
        assert_eq!(call.package, objects.registry_pkg_id);
        assert_eq!(call.module.as_str(), "priority_fee_vault");
        assert_eq!(call.function.as_str(), "withdraw_priority_fee");
        assert_eq!(call.arguments.len(), 4);
        assert_shared(&ptb, &call.arguments[0], &objects.priority_fee_vault, true);
        assert_shared(&ptb, &call.arguments[1], &objects.leader_registry, false);
        assert_owned(&ptb, &call.arguments[2], &leader_cap);
        assert!(matches!(&ptb.commands[1], Command::TransferObjects(_)));
    }
}
