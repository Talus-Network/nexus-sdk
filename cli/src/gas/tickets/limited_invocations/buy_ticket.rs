use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::ToolFqn,
};

/// Buy a limited invocations gas ticket to pay for the specified tool.
pub(crate) async fn buy_limited_invocations_gas_ticket(
    tool_fqn: ToolFqn,
    invocations: u64,
    coin: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Buying a limited invocations gas ticket for '{invocations}' invocations for tool '{tool_fqn}'");

    if Some(coin) == sui_gas_coin {
        return Err(NexusCliError::Any(anyhow!(
            "The coin used to pay for the ticket cannot be the same as the gas coin."
        )));
    }

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let tx_handle = loading!("Crafting and executing transaction...");

    let response = match nexus_client
        .gas()
        .buy_limited_invocations_ticket(tool_fqn, invocations, coin)
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
