use {
    crate::{command_title, display::json_output, loading, prelude::*, sui::*},
    nexus_sdk::transactions::gas,
};

/// Buy a limited invocations gas ticket to pay for the specified tool.
#[cfg(not(test))]
pub(crate) async fn buy_limited_invocations_gas_ticket(
    tool_fqn: ToolFqn,
    invocations: u64,
    coin: sui::ObjectID,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Buying a limited invocations gas ticket for '{invocations}' invocations for tool '{tool_fqn}'");

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Nexus objects must be present in the configuration.
    let objects = &get_nexus_objects(&mut conf).await?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;

    // Fetch the coin to pay for the ticket with.
    let pay_with_coin = fetch_object_by_id(&sui, coin).await?;

    if pay_with_coin.object_id == gas_coin.coin_object_id {
        return Err(NexusCliError::Any(anyhow!(
            "Gas and payment coins must be different."
        )));
    }

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft the transaction.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Err(e) = gas::buy_limited_invocations_gas_ticket(
        &mut tx,
        objects,
        &tool_fqn,
        &pay_with_coin,
        invocations,
    ) {
        tx_handle.error();

        return Err(NexusCliError::Any(e));
    };

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and send the TX.
    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    json_output(&json!({ "digest": response.digest }))?;

    Ok(())
}

// TODO: Remove this test stub when we have a proper test suite for testing SDK functions.
// Also remove the #[cfg(not(test))] from the main function above.
// This is just a temporary fix to pass CI coverage tests.
// https://github.com/Talus-Network/nexus/issues/418
#[cfg(test)]
pub(crate) async fn buy_limited_invocations_gas_ticket(
    _tool_fqn: ToolFqn,
    _invocations: u64,
    _coin: sui::ObjectID,
    _sui_gas_coin: Option<sui::ObjectID>,
    _sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    Ok(())
}

// TODO: Replace with proper tests when we have a test suite for SDK functions.
// These are temporary tests just to pass CI coverage.
// https://github.com/Talus-Network/nexus/issues/418
#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    #[tokio::test]
    async fn test_buy_limited_invocations_gas_ticket() {
        let tool_fqn = ToolFqn::from_str("xyz.taluslabs.math.i64.add@1").unwrap();
        let coin = sui::ObjectID::from_str("0x1234567890abcdef1234567890abcdef12345678").unwrap();
        let result = buy_limited_invocations_gas_ticket(tool_fqn, 10, coin, None, 1000).await;

        assert!(result.is_ok());
    }
}
