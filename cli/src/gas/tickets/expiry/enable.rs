use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::ToolFqn,
};

/// Enable the expiry gas extension for the specified tool.
pub(crate) async fn enable_expiry_extension(
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    cost_per_minute: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Enabling the expiry gas extension for tool '{tool_fqn}' with cost '{cost_per_minute}' MIST per minute");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();

    let Some(owner_cap) = owner_cap.or(conf.tools.get(&tool_fqn).and_then(|t| t.over_gas)) else {
        return Err(NexusCliError::Any(anyhow!(
            "No OwnerCap object ID found for tool '{tool_fqn}'."
        )));
    };

    let tx_handle = loading!("Crafting and executing transaction...");
    let response = match nexus_client
        .gas()
        .enable_expiry_extension(tool_fqn, owner_cap, cost_per_minute)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(e));
        }
    };

    tx_handle.success();

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.tx_digest }))?;

    Ok(())
}
