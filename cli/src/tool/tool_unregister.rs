use crate::{
    command_title,
    confirm,
    display::json_output,
    loading,
    notify_success,
    prelude::*,
    sui::*,
};

pub(crate) async fn unregister_tool(
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    skip_confirmation: bool,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Unregistering Tool '{tool_fqn}'");
    if !skip_confirmation {
        confirm!(
            "Unregistering this Tool stops new executions. Existing obligations may still drain. Continue?"
        );
    }
    let client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();
    let owner_cap = owner_cap
        .or_else(|| conf.tools.get(&tool_fqn).map(|tool| tool.over_tool))
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No OwnerCap<OverTool> object ID found for Tool '{tool_fqn}'."
            ))
        })?;
    let progress = loading!("Unregistering Tool...");
    let result = client
        .tool()
        .unregister(&tool_fqn, owner_cap)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();
    notify_success!("Transaction digest: {digest}", digest = result.tx_digest);
    json_output(&json!({
        "action": "unregister",
        "tool_fqn": tool_fqn,
        "digest": result.tx_digest,
    }))
}
