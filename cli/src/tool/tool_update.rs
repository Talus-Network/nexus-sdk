use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

pub(crate) async fn update_url(
    tool_fqn: ToolFqn,
    url: reqwest::Url,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let owner_cap = resolve_owner_cap(&tool_fqn, owner_cap).await?;
    let client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    command_title!("Updating endpoint for Tool '{tool_fqn}'");
    let progress = loading!("Updating Tool endpoint...");
    let result = client
        .tool()
        .update_url(&tool_fqn, url.as_str(), owner_cap)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();
    notify_success!("Transaction digest: {digest}", digest = result.tx_digest);
    json_output(&json!({
        "action": "update_url",
        "tool_fqn": tool_fqn,
        "url": url,
        "digest": result.tx_digest,
    }))
}

pub(crate) async fn update_metadata(
    tool_fqn: ToolFqn,
    description: String,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let owner_cap = resolve_owner_cap(&tool_fqn, owner_cap).await?;
    let client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    command_title!("Updating metadata for Tool '{tool_fqn}'");
    let progress = loading!("Updating Tool description...");
    let result = client
        .tool()
        .update_metadata(&tool_fqn, &description, owner_cap)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();
    notify_success!("Transaction digest: {digest}", digest = result.tx_digest);
    json_output(&json!({
        "action": "update_metadata",
        "tool_fqn": tool_fqn,
        "description": description,
        "digest": result.tx_digest,
    }))
}

pub(crate) async fn update_on_chain_package(
    tool_fqn: ToolFqn,
    package: sui::types::Address,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let owner_cap = resolve_owner_cap(&tool_fqn, owner_cap).await?;
    let client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    command_title!("Migrating package for Tool '{tool_fqn}'");
    let progress = loading!("Updating onchain Tool package...");
    let result = client
        .tool()
        .migrate_on_chain_package(&tool_fqn, package, owner_cap)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();
    notify_success!("Transaction digest: {digest}", digest = result.tx_digest);
    json_output(&json!({
        "action": "migrate_package",
        "tool_fqn": tool_fqn,
        "package": package,
        "digest": result.tx_digest,
    }))
}

async fn resolve_owner_cap(
    tool_fqn: &ToolFqn,
    owner_cap: Option<sui::types::Address>,
) -> AnyResult<sui::types::Address, NexusCliError> {
    let conf = CliConf::load().await.unwrap_or_default();
    owner_cap
        .or_else(|| conf.tools.get(tool_fqn).map(|tool| tool.over_tool))
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No OwnerCap<OverTool> object ID found for Tool '{tool_fqn}'."
            ))
        })
}
