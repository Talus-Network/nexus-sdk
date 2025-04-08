use crate::{
    idents::{move_std, workflow},
    sui,
    types::ToolMeta,
    ToolFqn,
};

/// PTB template for registering a new Nexus Tool.
pub fn register_off_chain(
    tx: &mut sui::ProgrammableTransactionBuilder,
    meta: &ToolMeta,
    collateral_coin: sui::Coin,
    tool_registry: sui::ObjectRef,
    workflow_pkg_id: sui::ObjectID,
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

    // `description: vector<u8>`
    let description = tx.pure(meta.description.as_bytes())?;

    // `input_schema: vector<u8>`
    let input_schema = tx.pure(meta.input_schema.to_string().as_bytes())?;

    // `output_schema: vector<u8>`
    let output_schema = tx.pure(meta.output_schema.to_string().as_bytes())?;

    // `pay_with: Coin<SUI>`
    let pay_with = tx.obj(sui::ObjectArg::ImmOrOwnedObject(
        collateral_coin.object_ref(),
    ))?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    // `nexus_workflow::tool_registry::register_off_chain_tool()`
    Ok(tx.programmable_move_call(
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
            description,
            input_schema,
            output_schema,
            pay_with,
            clock,
        ],
    ))
}

/// PTB template for unregistering a Nexus Tool.
pub fn unregister(
    tx: &mut sui::ProgrammableTransactionBuilder,
    tool_fqn: &ToolFqn,
    tool_registry: sui::ObjectRef,
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

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    // `nexus::tool_registry::unregister_tool()`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::ToolRegistry::UNREGISTER_TOOL.module.into(),
        workflow::ToolRegistry::UNREGISTER_TOOL.name.into(),
        vec![],
        vec![tool_registry, fqn, clock],
    ))
}

/// PTB template for claiming collateral for a Nexus Tool.
pub fn claim_collateral(
    tx: &mut sui::ProgrammableTransactionBuilder,
    tool_fqn: &ToolFqn,
    tool_registry: sui::ObjectRef,
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

    // `clock: &Clock`
    let clock = tx.obj(sui::ObjectArg::SharedObject {
        id: sui::CLOCK_OBJECT_ID,
        initial_shared_version: sui::CLOCK_OBJECT_SHARED_VERSION,
        mutable: false,
    })?;

    // `nexus::tool_registry::claim_collateral_for_tool()`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_TOOL
            .module
            .into(),
        workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_TOOL
            .name
            .into(),
        vec![],
        vec![tool_registry, fqn, clock],
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
    fn test_register_off_chain_tool() {
        let meta = ToolMeta {
            fqn: fqn!("xyz.dummy.tool@1"),
            url: "https://example.com".parse().unwrap(),
            description: "a dummy tool".to_string(),
            input_schema: json!({}),
            output_schema: json!({}),
        };

        let collateral_coin = sui_mocks::mock_sui_coin(100);
        let tool_registry = sui_mocks::mock_sui_object_ref();
        let workflow_pkg_id = sui::ObjectID::random();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        register_off_chain(
            &mut tx,
            &meta,
            collateral_coin.clone(),
            tool_registry.clone(),
            workflow_pkg_id,
        )
        .expect("Failed to build PTB for registering a tool.");
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to register a tool");
        };

        assert_eq!(call.package, workflow_pkg_id);

        assert_eq!(
            call.module,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL
                .module
                .to_string(),
        );

        assert_eq!(
            call.function,
            workflow::ToolRegistry::REGISTER_OFF_CHAIN_TOOL
                .name
                .to_string()
        );

        assert_eq!(call.arguments.len(), 6);
    }

    #[test]
    fn test_unregister_tool() {
        let tool_fqn = fqn!("xyz.dummy.tool@1");
        let tool_registry = sui_mocks::mock_sui_object_ref();
        let workflow_pkg_id = sui::ObjectID::random();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        unregister(&mut tx, &tool_fqn, tool_registry.clone(), workflow_pkg_id)
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

        assert_eq!(call.arguments.len(), 3);
    }

    #[test]
    fn test_claim_collatera_for_tool() {
        let tool_fqn = fqn!("xyz.dummy.tool@1");
        let tool_registry = sui_mocks::mock_sui_object_ref();
        let workflow_pkg_id = sui::ObjectID::random();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        claim_collateral(&mut tx, &tool_fqn, tool_registry.clone(), workflow_pkg_id)
            .expect("Failed to build PTB for claiming collateral for a tool.");
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to claim collateral for a tool");
        };

        assert_eq!(call.package, workflow_pkg_id);

        assert_eq!(
            call.module,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_TOOL
                .module
                .to_string(),
        );

        assert_eq!(
            call.function,
            workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_TOOL
                .name
                .to_string()
        );

        assert_eq!(call.arguments.len(), 3);
    }
}
