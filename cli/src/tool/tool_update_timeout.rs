use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

pub(crate) async fn update_tool_timeout(
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    timeout: std::time::Duration,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Setting '{timeout:?}' timeout for tool '{tool_fqn}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();

    // Use the provided or saved `owner_cap` object ID and fetch the object.
    let Some(owner_cap) = owner_cap.or(conf.tools.get(&tool_fqn).and_then(|t| t.over_gas)) else {
        return Err(NexusCliError::Any(anyhow!(
            "No OwnerCap object ID found for tool '{tool_fqn}'."
        )));
    };

    let tx_handle = loading!("Crafting and executing transaction...");

    let response = match nexus_client
        .tool()
        .update_timeout(&tool_fqn, timeout, owner_cap)
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
