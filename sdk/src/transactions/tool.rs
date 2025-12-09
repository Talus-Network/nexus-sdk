use crate::{
    idents::{move_std, primitives, pure_arg, sui_framework, workflow},
    sui,
    types::{NexusObjects, ToolMeta},
    ToolFqn,
};

/// PTB template for registering a new Nexus Tool.
pub fn register_off_chain_for_self(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    meta: &ToolMeta,
    address: sui::types::Address,
    collateral_coin: &sui::types::ObjectReference,
    invocation_cost: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        true,
    ));

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, meta.fqn.to_string())?;

    // `url: vector<u8>`
    let url = tx.input(pure_arg(&meta.url)?);

    // `description: vector<u8>`
    let description = tx.input(pure_arg(&meta.description)?);

    // `input_schema: vector<u8>`
    let input_schema = tx.input(pure_arg(&meta.input_schema)?);

    // `output_schema: vector<u8>`
    let output_schema = tx.input(pure_arg(&meta.output_schema)?);

    // `pay_with: Coin<SUI>`
    let pay_with = tx.input(sui::tx::Input::owned(
        *collateral_coin.object_id(),
        collateral_coin.version(),
        *collateral_coin.digest(),
    ));

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus_workflow::tool_registry::register_off_chain_tool()`
    let owner_cap_over_tool = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL.module,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL.name,
            vec![],
        ),
        vec![
            tool_registry,
            fqn,
            url,
            description,
            input_schema,
            output_schema,
            pay_with,
            clock,
        ],
    );

    // `nexus_workflow::gas::deescalate()`
    let owner_cap_over_gas = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::DEESCALATE.module,
            workflow::Gas::DEESCALATE.name,
            vec![],
        ),
        vec![tool_registry, owner_cap_over_tool, fqn],
    );

    // `gas_service: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `single_invocation_cost_mist: u64`
    let single_invocation_cost_mist = tx.input(pure_arg(&invocation_cost)?);
    // `nexus_workflow::gas::set_single_invocation_cost_mist`
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.module,
            workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.name,
            vec![],
        ),
        vec![
            gas_service,
            tool_registry,
            owner_cap_over_gas,
            fqn,
            single_invocation_cost_mist,
        ],
    );

    // `CloneableOwnerCap<OverGas>`
    let over_gas_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.primitives_pkg_id,
        primitives::OwnerCap::CLONEABLE_OWNER_CAP.module,
        primitives::OwnerCap::CLONEABLE_OWNER_CAP.name,
        vec![sui::types::TypeTag::Struct(Box::new(
            sui::types::StructTag::new(
                objects.workflow_pkg_id,
                workflow::Gas::OVER_GAS.module,
                workflow::Gas::OVER_GAS.name,
                vec![],
            ),
        ))],
    )));

    // `CloneableOwnerCap<OverTool>`
    let over_tool_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.primitives_pkg_id,
        primitives::OwnerCap::CLONEABLE_OWNER_CAP.module,
        primitives::OwnerCap::CLONEABLE_OWNER_CAP.name,
        vec![sui::types::TypeTag::Struct(Box::new(
            sui::types::StructTag::new(
                objects.workflow_pkg_id,
                workflow::ToolRegistry::OVER_TOOL.module,
                workflow::ToolRegistry::OVER_TOOL.name,
                vec![],
            ),
        ))],
    )));

    // `recipient: address`
    let recipient = sui_framework::Address::address_from_type(tx, address)?;

    // `sui::transfer::public_transfer`
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_TRANSFER.module,
            sui_framework::Transfer::PUBLIC_TRANSFER.name,
            vec![over_tool_type],
        ),
        vec![owner_cap_over_tool, recipient],
    );

    // `sui::transfer::public_transfer`
    Ok(tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_TRANSFER.module,
            sui_framework::Transfer::PUBLIC_TRANSFER.name,
            vec![over_gas_type],
        ),
        vec![owner_cap_over_gas, recipient],
    ))
}

/// PTB template for registering a new onchain Nexus Tool.
#[allow(clippy::too_many_arguments)]
pub fn register_on_chain_for_self(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    package_address: sui::types::Address,
    module_name: String,
    input_schema: String,
    output_schema: String,
    fqn: &ToolFqn,
    description: String,
    witness_id: sui::types::Address,
    collateral_coin: &sui::types::ObjectReference,
    address: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        true,
    ));

    // `package_address: address`
    let package_addr = sui_framework::Address::address_from_type(tx, package_address)?;

    // `module_name: AsciiString`
    let module_name = move_std::Ascii::ascii_string_from_str(tx, module_name)?;

    // `input_schema: vector<u8>`
    let input_schema = tx.input(pure_arg(&input_schema)?);

    // `output_schema: vector<u8>`
    let output_schema = tx.input(pure_arg(&output_schema)?);

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;

    // `description: vector<u8>`
    let description = tx.input(pure_arg(&description)?);

    // `witness_id: ID`
    let witness_id = sui_framework::Address::address_from_type(tx, witness_id)?;

    // `pay_with: Coin<SUI>`
    let pay_with = tx.input(sui::tx::Input::owned(
        *collateral_coin.object_id(),
        collateral_coin.version(),
        *collateral_coin.digest(),
    ));

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus_workflow::tool_registry::register_on_chain_tool()`
    let owner_cap_over_tool = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ToolRegistry::REGISTER_ON_CHAIN_TOOL.module,
            workflow::ToolRegistry::REGISTER_ON_CHAIN_TOOL.name,
            vec![],
        ),
        vec![
            tool_registry,
            package_addr,
            module_name,
            input_schema,
            output_schema,
            fqn,
            description,
            witness_id,
            pay_with,
            clock,
        ],
    );

    // `CloneableOwnerCap<OverTool>`
    let over_tool_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.primitives_pkg_id,
        primitives::OwnerCap::CLONEABLE_OWNER_CAP.module,
        primitives::OwnerCap::CLONEABLE_OWNER_CAP.name,
        vec![sui::types::TypeTag::Struct(Box::new(
            sui::types::StructTag::new(
                objects.workflow_pkg_id,
                workflow::ToolRegistry::OVER_TOOL.module,
                workflow::ToolRegistry::OVER_TOOL.name,
                vec![],
            ),
        ))],
    )));

    // `recipient: address`
    let recipient = sui_framework::Address::address_from_type(tx, address)?;

    // `sui::transfer::public_transfer`
    Ok(tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_TRANSFER.module,
            sui_framework::Transfer::PUBLIC_TRANSFER.name,
            vec![over_tool_type],
        ),
        vec![owner_cap_over_tool, recipient],
    ))
}

/// PTB template for setting the invocation cost of a Nexus Tool.
pub fn set_invocation_cost(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::types::ObjectReference,
    invocation_cost: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `tool_registry: &mut ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        true,
    ));

    // `owner_cap: &CloneableOwnerCap<OverGas>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `single_invocation_cost_mist: u64`
    let single_invocation_cost_mist = tx.input(pure_arg(&invocation_cost)?);

    // `nexus_workflow::gas::set_single_invocation_cost_mist`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.module,
            workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.name,
            vec![],
        ),
        vec![
            gas_service,
            tool_registry,
            owner_cap,
            fqn,
            single_invocation_cost_mist,
        ],
    ))
}

/// PTB template for unregistering a Nexus Tool.
pub fn unregister(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        true,
    ));

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `owner_cap: &CloneableOwnerCap<OverTool>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus::tool_registry::unregister_tool()`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ToolRegistry::UNREGISTER_TOOL.module,
            workflow::ToolRegistry::UNREGISTER_TOOL.name,
            vec![],
        ),
        vec![tool_registry, owner_cap, fqn, clock],
    ))
}

/// PTB template for claiming collateral for a Nexus Tool. The funds are
/// transferred to the tx sender.
pub fn claim_collateral_for_self(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        true,
    ));

    // `owner_cap: &CloneableOwnerCap<OverTool>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap.object_id(),
        owner_cap.version(),
        *owner_cap.digest(),
    ));

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus::tool_registry::claim_collateral_for_self()`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF.module,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF.name,
            vec![],
        ),
        vec![tool_registry, owner_cap, fqn, clock],
    ))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{fqn, test_utils::sui_mocks},
        serde_json::json,
    };

    #[test]
    fn test_register_off_chain_for_self() {
        let rng = &mut rand::thread_rng();
        let meta = ToolMeta {
            fqn: fqn!("xyz.dummy.tool@1"),
            url: "https://example.com".parse().unwrap(),
            description: "a dummy tool".to_string(),
            input_schema: json!({}),
            output_schema: json!({}),
        };

        let objects = sui_mocks::mock_nexus_objects();
        let collateral_coin = sui_mocks::mock_sui_object_ref();
        let address = sui::types::Address::generate(rng);
        let invocation_cost = 1000;

        let mut tx = sui::tx::TransactionBuilder::new();
        register_off_chain_for_self(
            &mut tx,
            &objects,
            &meta,
            address,
            &collateral_coin,
            invocation_cost,
        )
        .expect("Failed to build PTB for registering a tool.");
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.get(1).unwrap() else {
            panic!("Expected a command to be a MoveCall to register a tool");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL.module,
        );
        assert_eq!(
            call.function,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL.name,
        );
        assert_eq!(call.arguments.len(), 8);
    }

    #[test]
    fn test_register_on_chain_for_self() {
        let mut rng = rand::thread_rng();
        let objects = sui_mocks::mock_nexus_objects();
        let package_address = sui::types::Address::generate(&mut rng);
        let module_name = "onchain_tool".to_string();
        let input_schema = json!({}).to_string();
        let output_schema = json!({}).to_string();
        let fqn = fqn!("xyz.dummy.tool@1");
        let description = "a dummy onchain tool".to_string();
        let witness_id = sui::types::Address::generate(&mut rng);
        let collateral_coin = sui_mocks::mock_sui_object_ref();
        let address = sui::types::Address::generate(&mut rng);

        let mut tx = sui::tx::TransactionBuilder::new();
        register_on_chain_for_self(
            &mut tx,
            &objects,
            package_address,
            module_name,
            input_schema,
            output_schema,
            &fqn,
            description,
            witness_id,
            &collateral_coin,
            address,
        )
        .expect("Failed to build PTB for registering an onchain tool.");
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.get(3).unwrap() else {
            panic!("Expected a command to be a MoveCall to register an onchain tool");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ToolRegistry::REGISTER_ON_CHAIN_TOOL.module
        );
        assert_eq!(
            call.function,
            workflow::ToolRegistry::REGISTER_ON_CHAIN_TOOL.name
        );
        assert_eq!(call.arguments.len(), 10);
    }

    #[test]
    fn test_unregister_tool() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.dummy.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        unregister(&mut tx, &objects, &tool_fqn, &owner_cap)
            .expect("Failed to build PTB for unregistering a tool.");
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to unregister a tool");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::ToolRegistry::UNREGISTER_TOOL.module);
        assert_eq!(call.function, workflow::ToolRegistry::UNREGISTER_TOOL.name);
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn test_claim_collateral_for_self() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.dummy.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        claim_collateral_for_self(&mut tx, &objects, &tool_fqn, &owner_cap)
            .expect("Failed to build PTB for claiming collateral for a tool.");
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to claim collateral for a tool");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF.module
        );
        assert_eq!(
            call.function,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF.name
        );
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn test_set_invocation_cost() {
        let tool_fqn = fqn!("xyz.dummy.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let objects = sui_mocks::mock_nexus_objects();
        let invocation_cost = 500;

        let mut tx = sui::tx::TransactionBuilder::new();
        set_invocation_cost(&mut tx, &objects, &tool_fqn, &owner_cap, invocation_cost)
            .expect("Failed to build PTB for setting invocation cost.");
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to set invocation cost");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.module
        );
        assert_eq!(
            call.function,
            workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.name
        );
        assert_eq!(call.arguments.len(), 5);
    }
}
