use {
    crate::{command_title, display::json_output, loading, prelude::*, sui::*},
    nexus_sdk::transactions::gas,
};

/// Enable the expiry gas extension for the specified tool.
pub(crate) async fn enable_expiry_extension(
    tool_fqn: ToolFqn,
    owner_cap: sui::ObjectID,
    cost_per_minute: u64,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Enabling the expiry gas extension for tool '{tool_fqn}' with cost '{cost_per_minute}' MIST per minute");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_default();

    // Nexus objects must be present in the configuration.
    let objects = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, conf.sui.net, address, sui_gas_coin).await?;

    // Fetch the OwnerCap<OverGas> object.
    let owner_cap = fetch_object_by_id(&sui, owner_cap).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft the transaction.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Err(e) = gas::enable_expiry(&mut tx, objects, &tool_fqn, &owner_cap, cost_per_minute) {
        tx_handle.error();

        return Err(NexusCliError::Any(e));
    };

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and send the TX.
    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    json_output(&json!({ "digest": response.digest }))?;

    Ok(())
}
