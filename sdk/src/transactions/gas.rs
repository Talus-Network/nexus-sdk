use {
    crate::{
        move_bindings::workflow::{gas as gas_binding, gas_extension as gas_extension_binding},
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

/// Build a PTB to enable the limited invocations gas extension for a tool.
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
    use {super::*, crate::types::DefaultDagExecutorTarget, sui::types::Command};

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
            workflow_original_pkg_id: None,
            scheduler_original_pkg_id: None,
        }
    }

    #[test]
    fn abort_expired_execution_with_tool_gas_uses_current_move_signature() {
        let objects = nexus_objects();
        let ptb = abort_expired_execution_with_tool_gas_ptb(
            &objects,
            &object_ref("0x20", 2, 20),
            &object_ref("0x21", 3, 21),
            &object_ref("0x22", 4, 22),
        )
        .expect("ptb should build");

        let call = ptb
            .commands
            .iter()
            .find_map(|command| match command {
                Command::MoveCall(call)
                    if call.module.as_str() == "gas_extension"
                        && call.function.as_str() == "abort_expired_execution_with_tool_gas" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("abort move call should exist");

        assert_eq!(call.arguments.len(), 4);
    }
}
