use crate::{
    idents::{move_std, pure_arg, sui_framework, workflow},
    sui,
    types::NexusObjects,
    ToolFqn,
};

/// PTB template to add gas budget to a transaction.
pub fn add_budget(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    invoker_address: sui::types::Address,
    coin: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `scope: Scope`
    let scope = workflow::Gas::scope_invoker_address_from_object_id(
        tx,
        objects.workflow_pkg_id,
        invoker_address,
    )?;

    // `balance: Balance<SUI>`
    let coin = tx.input(sui::tx::Input::owned(
        *coin.object_id(),
        coin.version(),
        *coin.digest(),
    ));

    let sui = sui_framework::into_type_tag(sui_framework::Sui::SUI);

    let balance = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Coin::INTO_BALANCE.module,
            sui_framework::Coin::INTO_BALANCE.name,
            vec![sui],
        ),
        vec![coin],
    );

    // `nexus_workflow::gas::add_gas_budget`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::ADD_GAS_BUDGET.module,
            workflow::Gas::ADD_GAS_BUDGET.name,
            vec![],
        ),
        vec![gas_service, scope, balance],
    ))
}

/// PTB template to enable the expiry gas extension for a tool.
pub fn enable_expiry(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::types::ObjectReference,
    cost_per_minute: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
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

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::enable_expiry`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::ENABLE_EXPIRY.module,
            workflow::GasExtension::ENABLE_EXPIRY.name,
            vec![],
        ),
        vec![gas_service, tool_registry, owner_cap, cost_per_minute, fqn],
    ))
}

/// PTB template to disable the expiry gas extension for a tool.
pub fn disable_expiry(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::disable_expiry`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::DISABLE_EXPIRY.module,
            workflow::GasExtension::DISABLE_EXPIRY.name,
            vec![],
        ),
        vec![gas_service, tool_registry, owner_cap, fqn],
    ))
}

/// PTB template to buy an expiry gas ticket.
pub fn buy_expiry_gas_ticket(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    pay_with: &sui::types::ObjectReference,
    minutes: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

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
        vec![gas_service, tool_registry, fqn, minutes, pay_with, clock],
    ))
}

/// PTB template to enable the limited invocations gas extension for a tool.
pub fn enable_limited_invocations(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::types::ObjectReference,
    cost_per_invocation: u64,
    min_invocations: u64,
    max_invocations: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
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

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::enable_limited_invocations`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.module,
            workflow::GasExtension::ENABLE_LIMITED_INVOCATIONS.name,
            vec![],
        ),
        vec![
            gas_service,
            tool_registry,
            owner_cap,
            cost_per_invocation,
            min_invocations,
            max_invocations,
            fqn,
        ],
    ))
}

/// PTB template to disable the limited invocations gas extension for a tool.
pub fn disable_limited_invocations(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `owner_cap: OwnerCap<OverGas>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `nexus_workflow::gas_extension::disable_limited_invocations`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::GasExtension::DISABLE_LIMITED_INVOCATIONS.module,
            workflow::GasExtension::DISABLE_LIMITED_INVOCATIONS.name,
            vec![],
        ),
        vec![gas_service, tool_registry, owner_cap, fqn],
    ))
}

/// PTB template to buy a limited invocations gas ticket.
pub fn buy_limited_invocations_gas_ticket(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    pay_with: &sui::types::ObjectReference,
    invocations: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `fqn: ToolFqn`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

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
        vec![
            gas_service,
            tool_registry,
            fqn,
            invocations,
            pay_with,
            clock,
        ],
    ))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{fqn, test_utils::sui_mocks},
    };

    /// Default cost per minute for gas expiry
    const DEFAULT_COST_PER_MINUTE: u64 = 100;

    #[test]
    fn test_add_budget() {
        let rng = &mut rand::thread_rng();
        let objects = sui_mocks::mock_nexus_objects();
        let invoker_address = sui::types::Address::generate(rng);
        let coin = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        add_budget(&mut tx, &objects, invoker_address, &coin).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to add gas budget");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Gas::ADD_GAS_BUDGET.module);
        assert_eq!(call.function, workflow::Gas::ADD_GAS_BUDGET.name);
    }

    #[test]
    fn test_enable_expiry() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let cost_per_minute = DEFAULT_COST_PER_MINUTE;

        let mut tx = sui::tx::TransactionBuilder::new();
        enable_expiry(&mut tx, &objects, &tool_fqn, &owner_cap, cost_per_minute).unwrap();
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
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        disable_expiry(&mut tx, &objects, &tool_fqn, &owner_cap).unwrap();
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
        let tool_fqn = fqn!("xyz.test.tool@1");
        let pay_with = sui_mocks::mock_sui_object_ref();
        let minutes = 60;

        let mut tx = sui::tx::TransactionBuilder::new();
        buy_expiry_gas_ticket(&mut tx, &objects, &tool_fqn, &pay_with, minutes).unwrap();
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
    fn test_enable_limited_invocations() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let cost_per_invocation = 50;
        let min_invocations = 10;
        let max_invocations = 100;

        let mut tx = sui::tx::TransactionBuilder::new();
        enable_limited_invocations(
            &mut tx,
            &objects,
            &tool_fqn,
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
        let tool_fqn = fqn!("xyz.test.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        disable_limited_invocations(&mut tx, &objects, &tool_fqn, &owner_cap).unwrap();
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
        let tool_fqn = fqn!("xyz.test.tool@1");
        let pay_with = sui_mocks::mock_sui_object_ref();
        let invocations = 100;

        let mut tx = sui::tx::TransactionBuilder::new();
        buy_limited_invocations_gas_ticket(&mut tx, &objects, &tool_fqn, &pay_with, invocations)
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
