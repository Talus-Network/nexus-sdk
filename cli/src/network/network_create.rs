use {
    crate::{command_title, loading, prelude::*, sui::*},
    nexus_sdk::{
        events::{NexusEvent, NexusEventKind},
        idents::workflow,
    },
};

/// Create a new Nexus network and assign `count_leader_caps` leader caps to
/// the provided addresses.
pub(crate) async fn create_network(
    addresses: Vec<sui::ObjectID>,
    count_leader_caps: u32,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Creating a new Nexus network for {} addresses",
        addresses.len()
    );

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id, ..
    } = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(conf.sui.net).await?;

    let address = match wallet.active_address() {
        Ok(address) => address,
        Err(e) => {
            return Err(NexusCliError::Any(e));
        }
    };

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, conf.sui.net, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft a TX to create a new network.
    let tx_handle = loading!("Crafting transaction...");

    let addresses = match serde_json::to_value(addresses).map(sui::SuiJsonValue::new) {
        Ok(Ok(addrs)) => addrs,
        _ => {
            tx_handle.error();

            return Err(NexusCliError::Any(anyhow!("Failed to serialize addresses")));
        }
    };

    let count_leader_caps = match sui::SuiJsonValue::new(count_leader_caps.to_string().into()) {
        Ok(count) => count,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    let tx_data = match sui
        .transaction_builder()
        .move_call(
            address,
            workflow_pkg_id,
            workflow::LeaderCap::CREATE_FOR_SELF_AND_ADDRESSES
                .module
                .as_str(),
            workflow::LeaderCap::CREATE_FOR_SELF_AND_ADDRESSES
                .name
                .as_str(),
            vec![],
            vec![count_leader_caps, addresses],
            Some(gas_coin.coin_object_id),
            sui_gas_budget,
            Some(reference_gas_price),
        )
        .await
    {
        Ok(tx_data) => tx_data,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tx_handle.success();

    // Sign the transaction and send it to the network.
    let response = sign_transaction(&sui, &wallet, tx_data).await?;

    // Parse network ID from the response.
    let Some(events) = response.events else {
        return Err(NexusCliError::Any(anyhow!("No events in the response")));
    };

    let Some(network_id) = events.data.into_iter().find_map(|event| {
        let nexus_event: NexusEvent = event.try_into().ok()?;

        match nexus_event.data {
            NexusEventKind::FoundingLeaderCapCreated(e) => Some(e.network),
            _ => None,
        }
    }) else {
        return Err(NexusCliError::Any(anyhow!("No network ID in the events")));
    };

    println!(
        "[{check}] New Nexus network created with ID: {id}",
        check = "✔".green().bold(),
        id = network_id.to_string().truecolor(100, 100, 100)
    );

    Ok(())
}
