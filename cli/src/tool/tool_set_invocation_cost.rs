use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

/// Set the invocation cost in MIST for a tool based on its FQN.
pub(crate) async fn set_tool_invocation_cost(
    tool_fqn: ToolFqn,
    payment_admin: Option<sui::types::Address>,
    invocation_cost: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Setting '{invocation_cost}' invocation cost for tool '{tool_fqn}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;

    let conf = CliConf::load().await.unwrap_or_default();

    // Use the provided or saved payment admin capability.
    let Some(payment_admin) = payment_admin.or(conf
        .tools
        .get(&tool_fqn)
        .and_then(|tool| tool.payment_admin))
    else {
        return Err(NexusCliError::Any(anyhow!(
            "No payment admin capability was provided for tool '{tool_fqn}'. Pass --payment-admin or register the Tool with this CLI first."
        )));
    };

    let progress = loading!("Setting Tool invocation price...");
    let response = match nexus_client
        .tool()
        .set_invocation_cost(&tool_fqn, payment_admin, invocation_cost)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    progress.success();

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({
        "action": "set_invocation_cost",
        "tool_fqn": tool_fqn,
        "invocation_cost_mist": invocation_cost,
        "digest": response.tx_digest,
    }))?;

    Ok(())
}
