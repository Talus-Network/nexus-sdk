use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

/// Claim collateral for a Tool based on the provided FQN.
pub(crate) async fn claim_collateral(
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Claiming collateral for Tool '{tool_fqn}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();

    // Use the provided or saved `owner_cap` object ID and fetch the object.
    let Some(owner_cap) = owner_cap.or(conf.tools.get(&tool_fqn).map(|t| t.over_tool)) else {
        return Err(NexusCliError::Any(anyhow!(
            "No OwnerCap object ID found for tool '{tool_fqn}'."
        )));
    };

    let progress = loading!("Submitting collateral claim transaction...");
    let response = nexus_client
        .tool()
        .claim_collateral(&tool_fqn, owner_cap)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.tx_digest }))?;

    Ok(())
}
