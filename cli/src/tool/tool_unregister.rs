use {
    crate::{command_title, confirm, display::json_output, loading, prelude::*, sui::*},
    nexus_sdk::transactions::tool,
};

/// Unregister a Tool based on the provided FQN.
pub(crate) async fn unregister_tool(
    tool_fqn: ToolFqn,
    owner_cap: sui::ObjectID,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
    skip_confirmation: bool,
) -> AnyResult<(), NexusCliError> {
    command_title!("Unregistering Tool '{tool_fqn}'");

    if !skip_confirmation {
        confirm!(
            "Unregistering a Tool will make all DAGs using it invalid. Do you want to proceed?"
        );
    }

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id,
        tool_registry,
        ..
    } = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;

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

    // Fetch the OwnerCap object.
    let owner_cap = fetch_object_by_id(&sui, owner_cap).await?;

    // Craft a TX to unregister the tool.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    match tool::unregister(
        &mut tx,
        &tool_fqn,
        &owner_cap,
        tool_registry,
        *workflow_pkg_id,
    ) {
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
    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    json_output(&json!({ "digest": response.digest }))?;

    Ok(())
}
