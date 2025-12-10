use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::transactions::gas,
};

/// Buy an expiry gas ticket to pay for the specified tool.
/// TODO: https://github.com/Talus-Network/nexus-sdk/issues/243
pub(crate) async fn buy_expiry_gas_ticket(
    tool_fqn: ToolFqn,
    minutes: u64,
    coin: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Buying an expiry gas ticket for '{minutes}' minutes for tool '{tool_fqn}'");

    if Some(coin) == sui_gas_coin {
        return Err(NexusCliError::Any(anyhow!(
            "The coin used to pay for the ticket cannot be the same as the gas coin."
        )));
    }

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let signer = nexus_client.signer();
    let gas_config = nexus_client.gas_config();
    let address = signer.get_active_address();
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let crawler = nexus_client.crawler();

    let pay_with_coin = crawler
        .get_object_metadata(coin)
        .await
        .map(|resp| resp.object_ref())
        .map_err(|e| {
            NexusCliError::Any(anyhow!(
                "Failed to fetch coin object metadata for '{coin}': {e}"
            ))
        })?;

    // Craft the transaction.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::tx::TransactionBuilder::new();

    if let Err(e) =
        gas::buy_expiry_gas_ticket(&mut tx, nexus_objects, &tool_fqn, &pay_with_coin, minutes)
    {
        tx_handle.error();

        return Err(NexusCliError::Any(e));
    };

    tx_handle.success();

    let mut gas_coin = gas_config.acquire_gas_coin().await;

    tx.set_sender(address);
    tx.set_gas_budget(gas_config.get_budget());
    tx.set_gas_price(nexus_client.get_reference_gas_price());

    tx.add_gas_objects(vec![sui::tx::Input::owned(
        *gas_coin.object_id(),
        gas_coin.version(),
        *gas_coin.digest(),
    )]);

    let tx = tx.finish().map_err(|e| NexusCliError::Any(e.into()))?;

    let signature = signer.sign_tx(&tx).await.map_err(NexusCliError::Nexus)?;

    let response = signer
        .execute_tx(tx, signature, &mut gas_coin)
        .await
        .map_err(NexusCliError::Nexus)?;

    gas_config.release_gas_coin(gas_coin).await;

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.digest.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.digest }))?;

    Ok(())
}
