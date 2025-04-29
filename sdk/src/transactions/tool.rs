use crate::{
    idents::{move_std, primitives, sui_framework, workflow},
    sui,
    types::ToolMeta,
    ToolFqn,
};

/// PTB template for registering a new Nexus Tool.
pub fn register_off_chain_for_self(
    tx: &mut sui::ProgrammableTransactionBuilder,
    meta: &ToolMeta,
    address: sui::ObjectID,
    collateral_coin: &sui::Coin,
    invocation_cost: u64,
    tool_registry: &sui::ObjectRef,
    gas_service: &sui::ObjectRef,
    workflow_pkg_id: sui::ObjectID,
    primitives_pkg_id: sui::ObjectID,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: tool_registry.object_id,
        initial_shared_version: tool_registry.version,
        mutable: true,
    })?;

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, meta.fqn.to_string())?;

    // `url: vector<u8>`
    let url = tx.pure(meta.url.to_string().as_bytes())?;

    // `input_schema: vector<u8>`
    let input_schema = tx.pure(meta.input_schema.to_string().as_bytes())?;

    // `output_schema: vector<u8>`
    let output_schema = tx.pure(meta.output_schema.to_string().as_bytes())?;

    // `pay_with: Coin<SUI>`
    let pay_with = tx.obj(sui::ObjectArg::ImmOrOwnedObject(
        collateral_coin.object_ref(),
    ))?;

    // `nexus_workflow::tool_registry::register_off_chain_tool()`
    let owner_cap_over_tool = tx.programmable_move_call(
        workflow_pkg_id,
        workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL
            .module
            .into(),
        workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL.name.into(),
        vec![],
        vec![
            tool_registry,
            fqn,
            url,
            input_schema,
            output_schema,
            pay_with,
        ],
    );

    // `nexus_workflow::gas::deescalate()`
    let owner_cap_over_gas = tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Gas::DEESCALATE.module.into(),
        workflow::Gas::DEESCALATE.name.into(),
        vec![],
        vec![tool_registry, owner_cap_over_tool, fqn],
    );

    // `gas_service: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: gas_service.object_id,
        initial_shared_version: gas_service.version,
        mutable: true,
    })?;

    // `single_invocation_cost_mist: u64`
    let single_invocation_cost_mist = tx.pure(invocation_cost)?;

    // `nexus_workflow::gas::set_single_invocation_cost_mist`
    tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.module.into(),
        workflow::Gas::SET_SINGLE_INVOCATION_COST_MIST.name.into(),
        vec![],
        vec![
            gas_service,
            tool_registry,
            owner_cap_over_gas,
            fqn,
            single_invocation_cost_mist,
        ],
    );

    // `CloneableOwnerCap<OverGas>`
    let over_gas_type = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
        address: *primitives_pkg_id,
        module: primitives::OwnerCap::CLONEABLE_OWNER_CAP.module.into(),
        name: primitives::OwnerCap::CLONEABLE_OWNER_CAP.name.into(),
        type_params: vec![sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *workflow_pkg_id,
            module: workflow::Gas::OVER_GAS.module.into(),
            name: workflow::Gas::OVER_GAS.name.into(),
            type_params: vec![],
        }))],
    }));

    // `CloneableOwnerCap<OverTool>`
    let over_tool_type = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
        address: *primitives_pkg_id,
        module: primitives::OwnerCap::CLONEABLE_OWNER_CAP.module.into(),
        name: primitives::OwnerCap::CLONEABLE_OWNER_CAP.name.into(),
        type_params: vec![sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *workflow_pkg_id,
            module: workflow::ToolRegistry::OVER_TOOL.module.into(),
            name: workflow::ToolRegistry::OVER_TOOL.name.into(),
            type_params: vec![],
        }))],
    }));

    // `recipient: address`
    let with_prefix = false;
    let recipient =
        sui_framework::Address::address_from_str(tx, address.to_canonical_string(with_prefix))?;

    // `sui::transfer::public_transfer`
    tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::Transfer::PUBLIC_TRANSFER.module.into(),
        sui_framework::Transfer::PUBLIC_TRANSFER.name.into(),
        vec![over_tool_type],
        vec![owner_cap_over_tool, recipient],
    );

    // `sui::transfer::public_transfer`
    Ok(tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::Transfer::PUBLIC_TRANSFER.module.into(),
        sui_framework::Transfer::PUBLIC_TRANSFER.name.into(),
        vec![over_gas_type],
        vec![owner_cap_over_gas, recipient],
    ))
}

/// PTB template for unregistering a Nexus Tool.
pub fn unregister(
    tx: &mut sui::ProgrammableTransactionBuilder,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::ObjectRef,
    tool_registry: &sui::ObjectRef,
    workflow_pkg_id: sui::ObjectID,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: tool_registry.object_id,
        initial_shared_version: tool_registry.version,
        mutable: true,
    })?;

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `owner_cap: &CloneableOwnerCap<OverTool>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `clock: &Clock`
    let clock = tx.obj(sui::ObjectArg::SharedObject {
        id: sui::CLOCK_OBJECT_ID,
        initial_shared_version: sui::CLOCK_OBJECT_SHARED_VERSION,
        mutable: false,
    })?;

    // `nexus::tool_registry::unregister_tool()`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::ToolRegistry::UNREGISTER_TOOL.module.into(),
        workflow::ToolRegistry::UNREGISTER_TOOL.name.into(),
        vec![],
        vec![tool_registry, owner_cap, fqn, clock],
    ))
}

/// PTB template for claiming collateral for a Nexus Tool. The funds are
/// transferred to the tx sender.
pub fn claim_collateral_for_self(
    tx: &mut sui::ProgrammableTransactionBuilder,
    tool_fqn: &ToolFqn,
    owner_cap: &sui::ObjectRef,
    tool_registry: &sui::ObjectRef,
    workflow_pkg_id: sui::ObjectID,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: tool_registry.object_id,
        initial_shared_version: tool_registry.version,
        mutable: true,
    })?;

    // `owner_cap: &CloneableOwnerCap<OverTool>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `clock: &Clock`
    let clock = tx.obj(sui::ObjectArg::SharedObject {
        id: sui::CLOCK_OBJECT_ID,
        initial_shared_version: sui::CLOCK_OBJECT_SHARED_VERSION,
        mutable: false,
    })?;

    // `nexus::tool_registry::claim_collateral_for_tool()`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF
            .module
            .into(),
        workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF
            .name
            .into(),
        vec![],
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
        let meta = ToolMeta {
            fqn: fqn!("xyz.dummy.tool@1"),
            url: "https://example.com".parse().unwrap(),
            input_schema: json!({}),
            output_schema: json!({}),
        };

        let collateral_coin = sui_mocks::mock_sui_coin(100);
        let tool_registry = sui_mocks::mock_sui_object_ref();
        let gas_service = sui_mocks::mock_sui_object_ref();
        let workflow_pkg_id = sui::ObjectID::random();
        let primitives_pkd_id = sui::ObjectID::random();
        let address = sui::ObjectID::random();
        let invocation_cost = 1000;

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        register_off_chain_for_self(
            &mut tx,
            &meta,
            address,
            &collateral_coin,
            invocation_cost,
            &tool_registry,
            &gas_service,
            workflow_pkg_id,
            primitives_pkd_id,
        )
        .expect("Failed to build PTB for registering a tool.");
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to register a tool");
        };

        assert_eq!(call.package, workflow_pkg_id);

        assert_eq!(
            call.module,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL_FOR_SELF
                .module
                .to_string(),
        );

        assert_eq!(
            call.function,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL_FOR_SELF
                .name
                .to_string()
        );

        assert_eq!(call.arguments.len(), 6);
    }

    #[test]
    fn test_unregister_tool() {
        let tool_fqn = fqn!("xyz.dummy.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let tool_registry = sui_mocks::mock_sui_object_ref();
        let workflow_pkg_id = sui::ObjectID::random();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        unregister(
            &mut tx,
            &tool_fqn,
            &owner_cap,
            &tool_registry,
            workflow_pkg_id,
        )
        .expect("Failed to build PTB for unregistering a tool.");
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to unregister a tool");
        };

        assert_eq!(call.package, workflow_pkg_id);

        assert_eq!(
            call.module,
            workflow::ToolRegistry::UNREGISTER_TOOL.module.to_string(),
        );

        assert_eq!(
            call.function,
            workflow::ToolRegistry::UNREGISTER_TOOL.name.to_string()
        );

        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn test_claim_collateral_for_self() {
        let tool_fqn = fqn!("xyz.dummy.tool@1");
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let tool_registry = sui_mocks::mock_sui_object_ref();
        let workflow_pkg_id = sui::ObjectID::random();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        claim_collateral_for_self(
            &mut tx,
            &tool_fqn,
            &owner_cap,
            &tool_registry,
            workflow_pkg_id,
        )
        .expect("Failed to build PTB for claiming collateral for a tool.");
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to claim collateral for a tool");
        };

        assert_eq!(call.package, workflow_pkg_id);

        assert_eq!(
            call.module,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF
                .module
                .to_string(),
        );

        assert_eq!(
            call.function,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_SELF
                .name
                .to_string()
        );

        assert_eq!(call.arguments.len(), 4);
    }
}
