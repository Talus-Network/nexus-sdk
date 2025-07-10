use crate::{
    idents::{move_std, sui_framework, workflow},
    sui,
    types::NexusObjects,
    ToolFqn,
};

/// PTB template to add gas budget to a transaction.
pub fn add_budget(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    invoker_address: sui::ObjectID,
    coin: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `scope: Scope`
    let scope = workflow::Gas::scope_invoker_address_from_object_id(
        tx,
        objects.workflow_pkg_id,
        invoker_address,
    )?;

    // `balance: Balance<SUI>`
    let coin = tx.obj(sui::ObjectArg::ImmOrOwnedObject(coin.to_object_ref()))?;
    let sui = sui_framework::into_type_tag(sui_framework::Sui::SUI);

    let balance = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::Coin::INTO_BALANCE.module.into(),
        sui_framework::Coin::INTO_BALANCE.name.into(),
        vec![sui],
        vec![coin],
    );

    // `nexus_workflow::gas::add_gas_budget`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Gas::ADD_GAS_BUDGET.module.into(),
        workflow::Gas::ADD_GAS_BUDGET.name.into(),
        vec![],
        vec![gas_service, scope, balance],
    ))
}

/// PTB template to enable the expiry gas extension for a tool.
pub fn enable_expiry(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::ObjectRef,
    cost_per_minute: u64,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.tool_registry.object_id,
        initial_shared_version: objects.tool_registry.version,
        mutable: false,
    })?;

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `cost_per_minute: u64`
    let cost_per_minute = tx.pure(cost_per_minute)?;

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::enable_expiry`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultGasExtension::ENABLE_EXPIRY.module.into(),
        workflow::DefaultGasExtension::ENABLE_EXPIRY.name.into(),
        vec![],
        vec![gas_service, tool_registry, owner_cap, cost_per_minute, fqn],
    ))
}

/// PTB template to disable the expiry gas extension for a tool.
pub fn disable_expiry(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.tool_registry.object_id,
        initial_shared_version: objects.tool_registry.version,
        mutable: false,
    })?;

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::disable_expiry`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultGasExtension::DISABLE_EXPIRY.module.into(),
        workflow::DefaultGasExtension::DISABLE_EXPIRY.name.into(),
        vec![],
        vec![gas_service, tool_registry, owner_cap, fqn],
    ))
}

/// PTB template to buy an expiry gas ticket.
pub fn buy_expiry_gas_ticket(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    pay_with: &sui::ObjectRef,
    minutes: u64,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.tool_registry.object_id,
        initial_shared_version: objects.tool_registry.version,
        mutable: false,
    })?;

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `minutes: u64`
    let minutes = tx.pure(minutes)?;

    // `pay_with: Coin<SUI>`
    let pay_with = tx.obj(sui::ObjectArg::ImmOrOwnedObject(pay_with.to_object_ref()))?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    // `nexus_workflow::gas_extension::buy_expiry_gas_ticket`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultGasExtension::BUY_EXPIRY_GAS_TICKET
            .module
            .into(),
        workflow::DefaultGasExtension::BUY_EXPIRY_GAS_TICKET
            .name
            .into(),
        vec![],
        vec![gas_service, tool_registry, fqn, minutes, pay_with, clock],
    ))
}

/// PTB template to enable the limited invocations gas extension for a tool.
pub fn enable_limited_invocations(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::ObjectRef,
    cost_per_invocation: u64,
    min_invocations: u64,
    max_invocations: u64,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.tool_registry.object_id,
        initial_shared_version: objects.tool_registry.version,
        mutable: false,
    })?;

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `cost_per_invocation: u64`
    let cost_per_invocation = tx.pure(cost_per_invocation)?;

    // `min_invocations: u64`
    let min_invocations = tx.pure(min_invocations)?;

    // `max_invocations: u64`
    let max_invocations = tx.pure(max_invocations)?;

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::enable_limited_invocations`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultGasExtension::ENABLE_LIMITED_INVOCATIONS.module.into(),
        workflow::DefaultGasExtension::ENABLE_LIMITED_INVOCATIONS.name.into(),
        vec![],
        vec![gas_service, tool_registry, owner_cap, cost_per_invocation, min_invocations, max_invocations, fqn],
    ))
}

/// PTB template to disable the limited invocations gas extension for a tool.
pub fn disable_limited_invocations(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.tool_registry.object_id,
        initial_shared_version: objects.tool_registry.version,
        mutable: false,
    })?;

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::disable_limited_invocations`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultGasExtension::DISABLE_LIMITED_INVOCATIONS.module.into(),
        workflow::DefaultGasExtension::DISABLE_LIMITED_INVOCATIONS.name.into(),
        vec![],
        vec![gas_service, tool_registry, owner_cap, fqn],
    ))
}

/// PTB template to buy a limited invocations gas ticket.
pub fn buy_limited_invocations_gas_ticket(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    pay_with: &sui::ObjectRef,
    invocations: u64,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.tool_registry.object_id,
        initial_shared_version: objects.tool_registry.version,
        mutable: false,
    })?;

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `invocations: u64`
    let invocations = tx.pure(invocations)?;

    // `pay_with: Coin<SUI>`
    let pay_with = tx.obj(sui::ObjectArg::ImmOrOwnedObject(pay_with.to_object_ref()))?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    // `nexus_workflow::gas_extension::buy_limited_invocations_gas_ticket`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultGasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET
            .module
            .into(),
        workflow::DefaultGasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET
            .name
            .into(),
        vec![],
        vec![gas_service, tool_registry, fqn, invocations, pay_with, clock],
    ))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{fqn, test_utils::sui_mocks},
    };

    #[test]
    fn test_add_budget() {
        let objects = sui_mocks::mock_nexus_objects();
        let invoker_address = sui::ObjectID::random();
        let coin = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        add_budget(&mut tx, &objects, invoker_address, &coin).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to add gas budget");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Gas::ADD_GAS_BUDGET.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::Gas::ADD_GAS_BUDGET.name.to_string()
        );
    }

    #[test]
    fn test_enable_expiry() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let cost_per_minute = 100;

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        enable_expiry(&mut tx, &objects, &tool_fqn, &owner_cap, cost_per_minute).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to enable expiry");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultGasExtension::ENABLE_EXPIRY
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::DefaultGasExtension::ENABLE_EXPIRY
                .name
                .to_string()
        );
    }

    #[test]
    fn test_disable_expiry() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        disable_expiry(&mut tx, &objects, &tool_fqn, &owner_cap).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to disable expiry");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultGasExtension::DISABLE_EXPIRY
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::DefaultGasExtension::DISABLE_EXPIRY
                .name
                .to_string()
        );
    }

    #[test]
    fn test_buy_expiry_gas_ticket() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let pay_with = sui_mocks::mock_sui_object_ref();
        let minutes = 60;

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        buy_expiry_gas_ticket(&mut tx, &objects, &tool_fqn, &pay_with, minutes).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to buy expiry gas ticket");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultGasExtension::BUY_EXPIRY_GAS_TICKET
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::DefaultGasExtension::BUY_EXPIRY_GAS_TICKET
                .name
                .to_string()
        );
    }

    #[test]
    fn test_enable_limited_invocations() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let cost_per_invocation = 50;
        let min_invocations = 10;
        let max_invocations = 100;

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        enable_limited_invocations(&mut tx, &objects, &tool_fqn, &owner_cap, cost_per_invocation, min_invocations, max_invocations).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to enable limited invocations");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultGasExtension::ENABLE_LIMITED_INVOCATIONS
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::DefaultGasExtension::ENABLE_LIMITED_INVOCATIONS
                .name
                .to_string()
        );
    }

    #[test]
    fn test_disable_limited_invocations() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        disable_limited_invocations(&mut tx, &objects, &tool_fqn, &owner_cap).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to disable limited invocations");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultGasExtension::DISABLE_LIMITED_INVOCATIONS
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::DefaultGasExtension::DISABLE_LIMITED_INVOCATIONS
                .name
                .to_string()
        );
    }

    #[test]
    fn test_buy_limited_invocations_gas_ticket() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let pay_with = sui_mocks::mock_sui_object_ref();
        let invocations = 100;

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        buy_limited_invocations_gas_ticket(&mut tx, &objects, &tool_fqn, &pay_with, invocations).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to buy limited invocations gas ticket");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultGasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::DefaultGasExtension::BUY_LIMITED_INVOCATIONS_GAS_TICKET
                .name
                .to_string()
        );
    }
}
