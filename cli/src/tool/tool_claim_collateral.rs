use {
    crate::{command_title, loading, prelude::*, sui::*},
    nexus_sdk::idents::{move_std, workflow},
};

/// Claim collateral for a Tool based on the provided FQN.
pub(crate) async fn claim_collateral(
    tool_fqn: ToolFqn,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Claiming collateral for Tool '{tool_fqn}'");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id,
        tool_registry_object_id,
        ..
    } = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(conf.sui.net).await?;

    let address = match wallet.active_address() {
        Ok(address) => address,
        Err(e) => {
            return Err(NexusCliError::Any(e));
        }
    };

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, conf.sui.net, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Fetch the tool registry object.
    let tool_registry = fetch_object_by_id(&sui, tool_registry_object_id).await?;

    // Craft a TX to claim the collaters for a Tool.
    let tx_handle = loading!("Crafting transaction...");

    let tx = match prepare_transaction(&tool_fqn, tool_registry, workflow_pkg_id) {
        Ok(tx) => tx,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and submit the TX.
    sign_transaction(&sui, &wallet, tx_data).await.map(|_| ())
}

/// Build a programmable transaction to claim the collateral for a tool.
fn prepare_transaction(
    tool_fqn: &ToolFqn,
    tool_registry: sui::ObjectRef,
    workflow_pkg_id: sui::ObjectID,
) -> AnyResult<sui::ProgrammableTransactionBuilder> {
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // `self: &mut ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: tool_registry.object_id,
        initial_shared_version: tool_registry.version,
        mutable: true,
    })?;

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(&mut tx, tool_fqn.to_string())?;

    // `clock: &Clock`
    let clock = tx.obj(sui::ObjectArg::SharedObject {
        id: sui::CLOCK_OBJECT_ID,
        initial_shared_version: sui::CLOCK_OBJECT_SHARED_VERSION,
        mutable: false,
    })?;

    // `nexus::tool_registry::claim_collateral_for_tool()`
    tx.programmable_move_call(
        workflow_pkg_id,
        workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_TOOL
            .module
            .into(),
        workflow::ToolRegistry::CLAIM_COLLATERAL_FOR_TOOL
            .name
            .into(),
        vec![],
        vec![tool_registry, fqn, clock],
    );

    Ok(tx)
}
