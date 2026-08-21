use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

pub(crate) async fn update_timeout(
    tool_fqn: ToolFqn,
    timeout: std::time::Duration,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Updating timeout for Tool '{tool_fqn}'");
    let client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();
    let owner_cap = owner_cap
        .or_else(|| conf.tools.get(&tool_fqn).map(|tool| tool.over_tool))
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No OwnerCap<OverTool> object ID found for Tool '{tool_fqn}'."
            ))
        })?;
    let progress = loading!("Updating Tool timeout...");
    let result = client
        .tool()
        .update_timeout(&tool_fqn, timeout, owner_cap)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();
    notify_success!("Transaction digest: {digest}", digest = result.tx_digest);
    json_output(&json!({
        "action": "update_timeout",
        "tool_fqn": tool_fqn,
        "timeout_ms": timeout.as_millis(),
        "digest": result.tx_digest,
    }))
}
