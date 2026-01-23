use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{transactions::gas, types::Tool},
};

/// Disable the limited invocations gas extension for the specified tool.
/// TODO: https://github.com/Talus-Network/nexus/issues/418
pub(crate) async fn disable_limited_invocations_extension(
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Disabling the limited invocations gas extension for tool '{tool_fqn}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let signer = nexus_client.signer();
    let gas_config = nexus_client.gas_config();
    let address = signer.get_active_address();
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let crawler = nexus_client.crawler();

    let conf = CliConf::load().await.unwrap_or_default();

    // Use the provided or saved `owner_cap` object ID and fetch the object.
    let Some(owner_cap) = owner_cap.or(conf.tools.get(&tool_fqn).and_then(|t| t.over_gas)) else {
        return Err(NexusCliError::Any(anyhow!(
            "No OwnerCap object ID found for tool '{tool_fqn}'."
        )));
    };

    let owner_cap = crawler
        .get_object_metadata(owner_cap)
        .await
        .map(|resp| resp.object_ref())
        .map_err(|e| {
            NexusCliError::Any(anyhow!(
                "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}"
            ))
        })?;

    // Resolve the tool derived object.
    let tool_handle = loading!("Resolving tool derived object for tool '{tool_fqn}'...");

    let tool_id = Tool::derive_id(*nexus_objects.tool_registry.object_id(), &tool_fqn)
        .map_err(NexusCliError::Any)?;

    let tool = match crawler.get_object_metadata(tool_id).await {
        Ok(resp) => {
            tool_handle.success();

            resp.object_ref()
        }
        Err(e) => {
            tool_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "Failed to fetch tool derived object for tool '{tool_fqn}': {e}"
            )));
        }
    };

    // Craft the transaction.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::tx::TransactionBuilder::new();

    if let Err(e) = gas::disable_limited_invocations(&mut tx, nexus_objects, &tool, &owner_cap) {
        tx_handle.error();

        return Err(NexusCliError::Any(e));
    }

    tx_handle.success();

    let mut gas_coin = gas_config.acquire_gas_coin().await;

    tx.set_sender(address);
    tx.set_gas_budget(gas_config.get_budget());
    tx.set_gas_price(nexus_client.get_reference_gas_price());

    tx.add_gas_objects(vec![sui::tx::Input::owned(
        *gas_coin.object_id(),
        gas_coin.version(),
        *gas_coin.digest(),
    )]);

    let tx = tx.finish().map_err(|e| NexusCliError::Any(e.into()))?;

    let signature = signer.sign_tx(&tx).await.map_err(NexusCliError::Nexus)?;

    let response = signer
        .execute_tx(tx, signature, &mut gas_coin)
        .await
        .map_err(NexusCliError::Nexus)?;

    gas_config.release_gas_coin(gas_coin).await;

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.digest.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.digest }))?;

    Ok(())
}
