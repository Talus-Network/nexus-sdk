use crate::{
    idents::{pure_arg, sui_framework, workflow},
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
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.input(sui::tx::Input::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.input(sui::tx::Input::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `cost_per_minute: u64`
    let cost_per_minute = tx.input(pure_arg(&cost_per_minute)?);

    // `nexus_workflow::gas_extension::enable_expiry`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::ENABLE_EXPIRY.module,
            workflow::GasExtension::ENABLE_EXPIRY.name,
            vec![],
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
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.input(sui::tx::Input::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.input(sui::tx::Input::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.input(sui::tx::Input::owned(
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
            vec![],
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
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.input(sui::tx::Input::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.input(sui::tx::Input::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `minutes: u64`
    let minutes = tx.input(pure_arg(&minutes)?);

    // `pay_with: Coin<SUI>`
    let pay_with = tx.input(sui::tx::Input::owned(
        *pay_with.object_id(),
        pay_with.version(),
        *pay_with.digest(),
    ));

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
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
            vec![],
        ),
        vec![tool_gas, tool, minutes, pay_with, clock],
    ))
}

/// PTB template to snapshot all DAG tool costs into the execution payment.
#[allow(clippy::too_many_arguments)]
pub fn snapshot_dag_tool_costs(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    gas_service: sui::types::Argument,
    execution: sui::types::Argument,
    dag: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::SNAPSHOT_DAG_TOOL_COSTS.module,
            workflow::Gas::SNAPSHOT_DAG_TOOL_COSTS.name,
            vec![],
        ),
        vec![gas_service, execution, dag],
    )
}

/// PTB template to finalize payment settlement for a vertex.
#[allow(clippy::too_many_arguments)]
pub fn finalize_payment_state_for_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    expected_vertex: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::FINALIZE_PAYMENT_STATE_FOR_VERTEX.module,
            workflow::Gas::FINALIZE_PAYMENT_STATE_FOR_VERTEX.name,
            vec![],
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
    tool_gas: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    expected_vertex: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::SETTLE_PAYMENT_STATE_FOR_VERTEX.module,
            workflow::Gas::SETTLE_PAYMENT_STATE_FOR_VERTEX.name,
            vec![],
        ),
        vec![tool_gas, dag, execution, expected_vertex],
    )
}

/// PTB template to finalize an invoker-funded TAP payment.
pub fn accomplish_tap_execution_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT.module,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT.name,
            vec![],
        ),
        vec![execution],
    )
}

/// PTB template to accomplish an agent-vault-funded TAP payment.
pub fn accomplish_tap_execution_payment_from_agent_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
    execution: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name,
            vec![],
        ),
        vec![agent, execution],
    )
}

/// PTB template to withdraw a verified leader's claimable priority fee.
pub fn withdraw_priority_fee(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    priority_fee_vault: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    amount: u64,
) -> anyhow::Result<sui::types::Argument> {
    let priority_fee_vault = tx.input(sui::tx::Input::shared(
        *priority_fee_vault.object_id(),
        priority_fee_vault.version(),
        true,
    ));
    let leader_registry = tx.input(sui::tx::Input::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));
    let leader_cap = tx.input(sui::tx::Input::shared(
        *leader_cap.object_id(),
        leader_cap.version(),
        false,
    ));
    let amount = tx.input(pure_arg(&amount)?);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::PriorityFeeVault::WITHDRAW_PRIORITY_FEE.module,
            workflow::PriorityFeeVault::WITHDRAW_PRIORITY_FEE.name,
            vec![],
        ),
        vec![priority_fee_vault, leader_registry, leader_cap, amount],
    ))
}

/// PTB template to refund payment settlement for a vertex.
#[allow(clippy::too_many_arguments)]
pub fn refund_payment_state_for_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_gas: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    expected_vertex: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::REFUND_PAYMENT_STATE_FOR_VERTEX.module,
            workflow::Gas::REFUND_PAYMENT_STATE_FOR_VERTEX.name,
            vec![],
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
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.input(sui::tx::Input::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.input(sui::tx::Input::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `cost_per_invocation: u64`
    let cost_per_invocation = tx.input(pure_arg(&cost_per_invocation)?);

    // `min_invocations: u64`
    let min_invocations = tx.input(pure_arg(&min_invocations)?);

    // `max_invocations: u64`
    let max_invocations = tx.input(pure_arg(&max_invocations)?);

    // `nexus_workflow::gas_extension::enable_limited_invocations`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.module,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.name,
            vec![],
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
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.input(sui::tx::Input::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.input(sui::tx::Input::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.input(sui::tx::Input::owned(
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
            vec![],
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
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolGas`
    let tool_gas = tx.input(sui::tx::Input::shared(
        *tool_gas.object_id(),
        tool_gas.version(),
        true,
    ));

    // `tool: &Tool`
    let tool = tx.input(sui::tx::Input::shared(
        *tool.object_id(),
        tool.version(),
        false,
    ));

    // `invocations: u64`
    let invocations = tx.input(pure_arg(&invocations)?);

    // `pay_with: Coin<SUI>`
    let pay_with = tx.input(sui::tx::Input::owned(
        *pay_with.object_id(),
        pay_with.version(),
        *pay_with.digest(),
    ));

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
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
            vec![],
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
        snapshot_dag_tool_costs(
            &mut tx,
            &objects,
            sui::types::Argument::Input(0),
            sui::types::Argument::Input(1),
            sui::types::Argument::Input(2),
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
        finalize_payment_state_for_vertex(
            &mut tx,
            &objects,
            sui::types::Argument::Input(0),
            sui::types::Argument::Input(1),
            sui::types::Argument::Input(2),
            sui::types::Argument::Input(3),
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
        settle_payment_state_for_vertex(
            &mut tx,
            &objects,
            sui::types::Argument::Input(0),
            sui::types::Argument::Input(1),
            sui::types::Argument::Input(2),
            sui::types::Argument::Input(3),
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
    fn test_accomplish_tap_execution_payment() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        accomplish_tap_execution_payment(&mut tx, &objects, sui::types::Argument::Input(0));
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction {
                commands,
                inputs: _,
            },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to accomplish TAP payment");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT.name
        );
        assert_eq!(call.arguments.len(), 1);
    }

    #[test]
    fn test_accomplish_tap_execution_payment_from_agent_vault() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        accomplish_tap_execution_payment_from_agent_vault(
            &mut tx,
            &objects,
            sui::types::Argument::Input(0),
            sui::types::Argument::Input(1),
        );
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to accomplish agent-vault TAP payment");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name
        );
        assert_eq!(call.arguments.len(), 2);
    }

    #[test]
    fn test_withdraw_priority_fee() {
        let objects = sui_mocks::mock_nexus_objects();
        let priority_fee_vault = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        withdraw_priority_fee(&mut tx, &objects, &priority_fee_vault, &leader_cap, 123).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, inputs },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to withdraw priority fee");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::PriorityFeeVault::WITHDRAW_PRIORITY_FEE.module
        );
        assert_eq!(
            call.function,
            workflow::PriorityFeeVault::WITHDRAW_PRIORITY_FEE.name
        );
        assert_eq!(call.arguments.len(), 4);
        assert_shared_object(&inputs, &call.arguments[0], &priority_fee_vault, true);
        assert_shared_object(&inputs, &call.arguments[1], &objects.leader_registry, false);
        assert_shared_object(&inputs, &call.arguments[2], &leader_cap, false);
        assert_pure_u64(&inputs, &call.arguments[3], 123);
    }

    #[test]
    fn test_refund_payment_state_for_vertex() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        refund_payment_state_for_vertex(
            &mut tx,
            &objects,
            sui::types::Argument::Input(0),
            sui::types::Argument::Input(1),
            sui::types::Argument::Input(2),
            sui::types::Argument::Input(3),
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

    fn input<'a>(
        inputs: &'a [sui::types::Input],
        argument: &sui::types::Argument,
    ) -> &'a sui::types::Input {
        let sui::types::Argument::Input(index) = argument else {
            panic!("expected input argument, got {argument:?}");
        };

        inputs
            .get(*index as usize)
            .unwrap_or_else(|| panic!("missing input at index {index}"))
    }

    fn assert_shared_object(
        inputs: &[sui::types::Input],
        argument: &sui::types::Argument,
        expected: &sui::types::ObjectReference,
        expected_mutable: bool,
    ) {
        let sui::types::Input::Shared {
            object_id,
            initial_shared_version,
            mutable,
        } = input(inputs, argument)
        else {
            panic!("expected shared input, got {:?}", input(inputs, argument));
        };

        assert_eq!(object_id, expected.object_id());
        assert_eq!(*initial_shared_version, expected.version());
        assert_eq!(*mutable, expected_mutable);
    }

    fn assert_pure_u64(
        inputs: &[sui::types::Input],
        argument: &sui::types::Argument,
        expected: u64,
    ) {
        let sui::types::Input::Pure { value } = input(inputs, argument) else {
            panic!("expected pure input, got {:?}", input(inputs, argument));
        };

        assert_eq!(bcs::from_bytes::<u64>(value).unwrap(), expected);
    }
}
