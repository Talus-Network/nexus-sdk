use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{
        events::NexusEventKind,
        idents::{sui_framework, workflow},
    },
};

/// Create a new Nexus network and assign `count_leader_caps` leader caps to
/// the provided addresses.
pub(crate) async fn create_network(
    addresses: Vec<sui::types::Address>,
    count_leader_caps: u32,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Creating a new Nexus network for {} addresses",
        addresses.len()
    );

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let signer = nexus_client.signer();
    let gas_config = nexus_client.gas_config();
    let address = signer.get_active_address();
    let nexus_objects = &*nexus_client.get_nexus_objects();

    // Craft a TX to create a new network.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::tx::TransactionBuilder::new();

    let addresses = match addresses
        .iter()
        .map(|addr| sui_framework::Address::address_from_type(&mut tx, *addr))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(addresses) => addresses,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "Failed to serialize addresses: {e}"
            )));
        }
    };

    let addresses = tx.make_move_vec(Some(sui::types::TypeTag::Address), addresses);

    let count_leader_caps = match idents::pure_arg(&count_leader_caps) {
        Ok(count_leader_caps) => tx.input(count_leader_caps),
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "Failed to serialize count_leader_caps: {e}"
            )));
        }
    };

    tx.move_call(
        sui::tx::Function::new(
            nexus_objects.workflow_pkg_id,
            workflow::LeaderCap::CREATE_FOR_SELF_AND_ADDRESSES.module,
            workflow::LeaderCap::CREATE_FOR_SELF_AND_ADDRESSES.name,
            vec![],
        ),
        vec![count_leader_caps, addresses],
    );

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

    let Some(network_id) = response.events.iter().find_map(|event| match &event.data {
        NexusEventKind::FoundingLeaderCapCreated(e) => Some(e.network),
        _ => None,
    }) else {
        return Err(NexusCliError::Any(anyhow!("No network ID in the events")));
    };

    notify_success!(
        "New Nexus network created with ID: {id}; digest: {digest}",
        id = network_id.to_string().truecolor(100, 100, 100),
        digest = response.digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&json!({ "digest": response.digest, "network_id": network_id }))?;

    Ok(())
}
