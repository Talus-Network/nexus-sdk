use {
    crate::{
        command_title,
        confirm,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{transactions::tool, types::Tool},
};

/// Unregister a Tool based on the provided FQN.
pub(crate) async fn unregister_tool(
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
    skip_confirmation: bool,
) -> AnyResult<(), NexusCliError> {
    command_title!("Unregistering Tool '{tool_fqn}'");

    if !skip_confirmation {
        confirm!(
            "Unregistering a Tool will make all DAGs using it invalid. Do you want to proceed?"
        );
    }

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let signer = nexus_client.signer();
    let address = signer.get_active_address();
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let crawler = nexus_client.crawler();

    let conf = CliConf::load().await.unwrap_or_default();

    // Use the provided or saved `owner_cap` object ID and fetch the object.
    let Some(owner_cap) = owner_cap.or(conf.tools.get(&tool_fqn).map(|t| t.over_tool)) else {
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

    // Craft a TX to unregister the tool.
    let tx_handle = loading!("Crafting transaction...");

    let tx = match tool::unregister_ptb(nexus_objects, &tool, &owner_cap) {
        Ok(tx) => tx,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Any(e));
        }
    };

    let response = match nexus_client.submit_transaction(tx, address).await {
        Ok(response) => response,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(e));
        }
    };

    tx_handle.success();

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.digest.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.digest }))?;

    Ok(())
}
