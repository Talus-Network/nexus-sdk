use crate::{
    idents::{sui_framework, workflow},
    sui,
    types::NexusObjects,
};

/// PTB template to enable the expiry gas extension for a tool.
pub fn enable_expiry(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    cost_per_minute: u64,
) -> anyhow::Result<sui::tx::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.object(sui::tx::ObjectInput::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.object(sui::tx::ObjectInput::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.object(sui::tx::ObjectInput::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `cost_per_minute: u64`
    let cost_per_minute = tx.pure(&cost_per_minute);

    // `nexus_workflow::gas_extension::enable_expiry`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::ENABLE_EXPIRY.module,
            workflow::GasExtension::ENABLE_EXPIRY.name,
        ),
        vec![tool_gas, tool, owner_cap, cost_per_minute],
    ))
}

/// PTB template to disable the expiry gas extension for a tool.
pub fn disable_expiry(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.object(sui::tx::ObjectInput::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.object(sui::tx::ObjectInput::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.object(sui::tx::ObjectInput::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `nexus_workflow::gas_extension::disable_expiry`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::DISABLE_EXPIRY.module,
            workflow::GasExtension::DISABLE_EXPIRY.name,
        ),
        vec![tool_gas, tool, owner_cap],
    ))
}

/// PTB template to buy an expiry gas ticket.
pub fn buy_expiry_gas_ticket(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    minutes: u64,
) -> anyhow::Result<sui::tx::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.object(sui::tx::ObjectInput::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.object(sui::tx::ObjectInput::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `minutes: u64`
    let minutes = tx.pure(&minutes);

    // `pay_with: Coin<SUI>`
    let pay_with = tx.object(sui::tx::ObjectInput::owned(
        *pay_with.object_id(),
        pay_with.version(),
        *pay_with.digest(),
    ));

    // `clock: &Clock`
    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus_workflow::gas_extension::buy_expiry_gas_ticket`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::BUY_EXPIRY_GAS_TICKET.module,
            workflow::GasExtension::BUY_EXPIRY_GAS_TICKET.name,
        ),
        vec![tool_gas, tool, minutes, pay_with, clock],
    ))
}

/// PTB template to snapshot all DAG tool costs into the execution payment.
#[allow(clippy::too_many_arguments)]
pub fn snapshot_dag_tool_costs(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    gas_service: sui::tx::Argument,
    execution: sui::tx::Argument,
    dag: sui::tx::Argument,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::SNAPSHOT_DAG_TOOL_COSTS.module,
            workflow::Gas::SNAPSHOT_DAG_TOOL_COSTS.name,
        ),
        vec![gas_service, execution, dag],
    )
}

/// PTB template to finalize payment settlement for a vertex.
#[allow(clippy::too_many_arguments)]
pub fn finalize_payment_state_for_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: sui::tx::Argument,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    expected_vertex: sui::tx::Argument,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::FINALIZE_PAYMENT_STATE_FOR_VERTEX.module,
            workflow::Gas::FINALIZE_PAYMENT_STATE_FOR_VERTEX.name,
        ),
        vec![tool_gas, dag, execution, expected_vertex],
    )
}

/// PTB template to settle payment for a vertex using pending DAG settlement
/// state when present.
#[allow(clippy::too_many_arguments)]
pub fn settle_payment_state_for_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: sui::tx::Argument,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    expected_vertex: sui::tx::Argument,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::SETTLE_PAYMENT_STATE_FOR_VERTEX.module,
            workflow::Gas::SETTLE_PAYMENT_STATE_FOR_VERTEX.name,
        ),
        vec![tool_gas, dag, execution, expected_vertex],
    )
}

/// PTB template to abort an expired execution by first unlocking/refunding the
/// matching ToolGas vertex payment.
#[allow(clippy::too_many_arguments)]
pub fn abort_expired_execution_with_tool_gas(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> sui::tx::Argument {
    let tool_gas_arg = tx.object(sui::tx::ObjectInput::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));
    let dag_arg = tx.object(sui::tx::ObjectInput::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));
    let execution_arg = tx.object(sui::tx::ObjectInput::shared(
        *execution.object_id(),
        execution.version(),
        true,
    ));
    let tool_registry_arg = tx.object(sui::tx::ObjectInput::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));
    let leader_registry_arg = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));
    let clock_arg = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::ABORT_EXPIRED_EXECUTION_WITH_TOOL_GAS.module,
            workflow::GasExtension::ABORT_EXPIRED_EXECUTION_WITH_TOOL_GAS.name,
        ),
        vec![
            tool_gas_arg,
            dag_arg,
            execution_arg,
            tool_registry_arg,
            leader_registry_arg,
            clock_arg,
        ],
    )
}

/// PTB template to refund payment settlement for a vertex.
#[allow(clippy::too_many_arguments)]
pub fn refund_payment_state_for_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: sui::tx::Argument,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    expected_vertex: sui::tx::Argument,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::REFUND_PAYMENT_STATE_FOR_VERTEX.module,
            workflow::Gas::REFUND_PAYMENT_STATE_FOR_VERTEX.name,
        ),
        vec![tool_gas, dag, execution, expected_vertex],
    )
}

/// PTB template to enable the limited invocations gas extension for a tool.
#[allow(clippy::too_many_arguments)]
pub fn enable_limited_invocations(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    cost_per_invocation: u64,
    min_invocations: u64,
    max_invocations: u64,
) -> anyhow::Result<sui::tx::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.object(sui::tx::ObjectInput::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.object(sui::tx::ObjectInput::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.object(sui::tx::ObjectInput::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `cost_per_invocation: u64`
    let cost_per_invocation = tx.pure(&cost_per_invocation);

    // `min_invocations: u64`
    let min_invocations = tx.pure(&min_invocations);

    // `max_invocations: u64`
    let max_invocations = tx.pure(&max_invocations);

    // `nexus_workflow::gas_extension::enable_limited_invocations`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.module,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.name,
        ),
        vec![
            tool_gas,
            tool,
            owner_cap,
            cost_per_invocation,
            min_invocations,
            max_invocations,
        ],
    ))
}

/// PTB template to disable the limited invocations gas extension for a tool.
pub fn disable_limited_invocations(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.object(sui::tx::ObjectInput::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.object(sui::tx::ObjectInput::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.object(sui::tx::ObjectInput::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `nexus_workflow::gas_extension::disable_limited_invocations`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::DISABLE_LIMITED_INVOCATIONS.module,
            workflow::GasExtension::DISABLE_LIMITED_INVOCATIONS.name,
        ),
        vec![tool_gas, tool, owner_cap],
    ))
}

/// PTB template to buy a limited invocations gas ticket.
pub fn buy_limited_invocations_gas_ticket(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    pay_with: &sui::types::ObjectReference,
    invocations: u64,
) -> anyhow::Result<sui::tx::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.object(sui::tx::ObjectInput::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.object(sui::tx::ObjectInput::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `invocations: u64`
    let invocations = tx.pure(&invocations);

    // `pay_with: Coin<SUI>`
    let pay_with = tx.object(sui::tx::ObjectInput::owned(
        *pay_with.object_id(),
        pay_with.version(),
        *pay_with.digest(),
    ));

    // `clock: &Clock`
    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus_workflow::gas_extension::buy_limited_invocations_gas_ticket`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET.module,
            workflow::GasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET.name,
        ),
        vec![tool_gas, tool, invocations, pay_with, clock],
    ))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks};

    /// Default cost per minute for gas expiry
    const DEFAULT_COST_PER_MINUTE: u64 = 100;

    #[test]
    fn test_enable_expiry() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_gas = sui_mocks::mock_sui_object_ref();
        let tool = sui_mocks::mock_sui_object_ref();
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let cost_per_minute = DEFAULT_COST_PER_MINUTE;

        let mut tx = sui::tx::TransactionBuilder::new();
        enable_expiry(
            &mut tx,
            &objects,
            &tool_gas,
            &tool,
            &owner_cap,
            cost_per_minute,
        )
        .unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to enable expiry");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::GasExtension::ENABLE_EXPIRY.module);
        assert_eq!(call.function, workflow::GasExtension::ENABLE_EXPIRY.name);
    }

    #[test]
    fn test_disable_expiry() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_gas = sui_mocks::mock_sui_object_ref();
        let tool = sui_mocks::mock_sui_object_ref();
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        disable_expiry(&mut tx, &objects, &tool_gas, &tool, &owner_cap).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to disable expiry");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::GasExtension::DISABLE_EXPIRY.module);
        assert_eq!(call.function, workflow::GasExtension::DISABLE_EXPIRY.name);
    }

    #[test]
    fn test_buy_expiry_gas_ticket() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_gas = sui_mocks::mock_sui_object_ref();
        let tool = sui_mocks::mock_sui_object_ref();
        let pay_with = sui_mocks::mock_sui_object_ref();
        let minutes = 60;

        let mut tx = sui::tx::TransactionBuilder::new();
        buy_expiry_gas_ticket(&mut tx, &objects, &tool_gas, &tool, &pay_with, minutes).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to buy expiry gas ticket");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::GasExtension::BUY_EXPIRY_GAS_TICKET.module
        );
        assert_eq!(
            call.function,
            workflow::GasExtension::BUY_EXPIRY_GAS_TICKET.name
        );
    }

    #[test]
    fn test_snapshot_dag_tool_costs() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        let __placeholder_arg_0 = tx.pure(&1u64);
        let __placeholder_arg_1 = tx.pure(&2u64);
        let __placeholder_arg_2 = tx.pure(&3u64);
        snapshot_dag_tool_costs(
            &mut tx,
            &objects,
            __placeholder_arg_0,
            __placeholder_arg_1,
            __placeholder_arg_2,
        );
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to snapshot payment tool costs");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Gas::SNAPSHOT_DAG_TOOL_COSTS.module);
        assert_eq!(call.function, workflow::Gas::SNAPSHOT_DAG_TOOL_COSTS.name);
        assert_eq!(call.arguments.len(), 3);
    }

    #[test]
    fn test_finalize_payment_state_for_vertex() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        let __placeholder_arg_3 = tx.pure(&4u64);
        let __placeholder_arg_4 = tx.pure(&5u64);
        let __placeholder_arg_5 = tx.pure(&6u64);
        let __placeholder_arg_6 = tx.pure(&7u64);
        finalize_payment_state_for_vertex(
            &mut tx,
            &objects,
            __placeholder_arg_3,
            __placeholder_arg_4,
            __placeholder_arg_5,
            __placeholder_arg_6,
        );
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to finalize payment state for a vertex");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Gas::FINALIZE_PAYMENT_STATE_FOR_VERTEX.module
        );
        assert_eq!(
            call.function,
            workflow::Gas::FINALIZE_PAYMENT_STATE_FOR_VERTEX.name
        );
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn test_settle_payment_state_for_vertex() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        let __placeholder_arg_7 = tx.pure(&8u64);
        let __placeholder_arg_8 = tx.pure(&9u64);
        let __placeholder_arg_9 = tx.pure(&10u64);
        let __placeholder_arg_10 = tx.pure(&11u64);
        settle_payment_state_for_vertex(
            &mut tx,
            &objects,
            __placeholder_arg_7,
            __placeholder_arg_8,
            __placeholder_arg_9,
            __placeholder_arg_10,
        );
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to settle payment state for a vertex");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Gas::SETTLE_PAYMENT_STATE_FOR_VERTEX.module
        );
        assert_eq!(
            call.function,
            workflow::Gas::SETTLE_PAYMENT_STATE_FOR_VERTEX.name
        );
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn test_abort_expired_execution_with_tool_gas() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_gas = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let execution = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        abort_expired_execution_with_tool_gas(&mut tx, &objects, &tool_gas, &dag, &execution);
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, inputs },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to abort with ToolGas");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::GasExtension::ABORT_EXPIRED_EXECUTION_WITH_TOOL_GAS.module
        );
        assert_eq!(
            call.function,
            workflow::GasExtension::ABORT_EXPIRED_EXECUTION_WITH_TOOL_GAS.name
        );
        assert_eq!(call.arguments.len(), 6);

        let expect_shared = |argument: &sui::types::Argument,
                             expected: &sui::types::ObjectReference,
                             expected_mutable: bool| {
            let sui::types::Argument::Input(index) = argument else {
                panic!("expected input argument, got {argument:?}");
            };
            let Some(sui::types::Input::Shared(shared)) = inputs.get(*index as usize) else {
                panic!("expected shared input at index {index}");
            };
            assert_eq!(shared.object_id(), *expected.object_id());
            assert_eq!(shared.version(), expected.version());
            assert_eq!(shared.mutability().is_mutable(), expected_mutable);
        };
        let expect_clock = |argument: &sui::types::Argument| {
            let sui::types::Argument::Input(index) = argument else {
                panic!("expected input argument, got {argument:?}");
            };
            let Some(sui::types::Input::Shared(shared)) = inputs.get(*index as usize) else {
                panic!("expected shared clock input at index {index}");
            };
            assert_eq!(shared.object_id(), sui_framework::CLOCK_OBJECT_ID);
            assert_eq!(shared.version(), 1);
            assert!(
                !shared.mutability().is_mutable(),
                "clock object must be immutable"
            );
        };

        expect_shared(&call.arguments[0], &tool_gas, true);
        expect_shared(&call.arguments[1], &dag, false);
        expect_shared(&call.arguments[2], &execution, true);
        expect_shared(&call.arguments[3], &objects.tool_registry, false);
        expect_shared(&call.arguments[4], &objects.leader_registry, false);
        expect_clock(&call.arguments[5]);
    }

    #[test]
    fn test_refund_payment_state_for_vertex() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        let __placeholder_arg_11 = tx.pure(&12u64);
        let __placeholder_arg_12 = tx.pure(&13u64);
        let __placeholder_arg_13 = tx.pure(&14u64);
        let __placeholder_arg_14 = tx.pure(&15u64);
        refund_payment_state_for_vertex(
            &mut tx,
            &objects,
            __placeholder_arg_11,
            __placeholder_arg_12,
            __placeholder_arg_13,
            __placeholder_arg_14,
        );
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to refund payment state for a vertex");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Gas::REFUND_PAYMENT_STATE_FOR_VERTEX.module
        );
        assert_eq!(
            call.function,
            workflow::Gas::REFUND_PAYMENT_STATE_FOR_VERTEX.name
        );
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn test_enable_limited_invocations() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_gas = sui_mocks::mock_sui_object_ref();
        let tool = sui_mocks::mock_sui_object_ref();
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let cost_per_invocation = 50;
        let min_invocations = 10;
        let max_invocations = 100;

        let mut tx = sui::tx::TransactionBuilder::new();
        enable_limited_invocations(
            &mut tx,
            &objects,
            &tool_gas,
            &tool,
            &owner_cap,
            cost_per_invocation,
            min_invocations,
            max_invocations,
        )
        .unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to enable limited invocations");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.module
        );
        assert_eq!(
            call.function,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.name
        );
    }

    #[test]
    fn test_disable_limited_invocations() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_gas = sui_mocks::mock_sui_object_ref();
        let tool = sui_mocks::mock_sui_object_ref();
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        disable_limited_invocations(&mut tx, &objects, &tool_gas, &tool, &owner_cap).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to disable limited invocations");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::GasExtension::DISABLE_LIMITED_INVOCATIONS.module
        );
        assert_eq!(
            call.function,
            workflow::GasExtension::DISABLE_LIMITED_INVOCATIONS.name
        );
    }

    #[test]
    fn test_buy_limited_invocations_gas_ticket() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_gas = sui_mocks::mock_sui_object_ref();
        let tool = sui_mocks::mock_sui_object_ref();
        let pay_with = sui_mocks::mock_sui_object_ref();
        let invocations = 100;

        let mut tx = sui::tx::TransactionBuilder::new();
        buy_limited_invocations_gas_ticket(
            &mut tx,
            &objects,
            &tool_gas,
            &tool,
            &pay_with,
            invocations,
        )
        .unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to buy limited invocations gas ticket");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::GasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET.module
        );
        assert_eq!(
            call.function,
            workflow::GasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET.name
        );
    }
}
