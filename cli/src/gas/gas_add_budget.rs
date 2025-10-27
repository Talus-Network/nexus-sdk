use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

/// Upload `coin` as a gas budget for the Nexus workflow.
pub(crate) async fn add_gas_budget(
    coin: sui::ObjectID,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Adding '{coin}' as gas budget for Nexus");

    if Some(coin) == sui_gas_coin {
        return Err(NexusCliError::Any(anyhow!(
            "Gas and budget coins must be different."
        )));
    }

    let (nexus_client, sui_client) = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;

    let budget_coin = fetch_object_by_id(&sui_client, coin).await?;

    let tx_handle = loading!("Crafting and executing transaction...");

    let response = match nexus_client.gas().add_budget(&budget_coin).await {
        Ok(resp) => resp,
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
