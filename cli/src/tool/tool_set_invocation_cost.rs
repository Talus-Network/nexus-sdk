use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

pub(crate) async fn set_invocation_cost(
    tool_fqn: ToolFqn,
    cost_mist: u64,
    cashier_admin: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Setting invocation cost for Tool '{tool_fqn}'");
    let client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();
    let cashier_admin = cashier_admin
        .or_else(|| {
            conf.tools
                .get(&tool_fqn)
                .and_then(|tool| tool.cashier_admin)
        })
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No cashier admin capability was provided for Tool '{tool_fqn}'."
            ))
        })?;
    let progress = loading!("Updating Tool invocation price...");
    let result = client
        .tool()
        .set_invocation_cost(&tool_fqn, cost_mist, cashier_admin)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();
    notify_success!("Transaction digest: {digest}", digest = result.tx_digest);
    json_output(&json!({
        "action": "set_invocation_cost",
        "tool_fqn": tool_fqn,
        "invocation_cost_mist": cost_mist,
        "digest": result.tx_digest,
    }))
}
