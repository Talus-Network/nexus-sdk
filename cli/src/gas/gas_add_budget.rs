use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{
        events::{NexusEvent, NexusEventKind},
        idents::workflow,
    },
};

/// Create a new Nexus network and assign `count_leader_caps` leader caps to
/// the provided addresses.
pub(crate) async fn add_gas_budget(
    coin_id: sui::ObjectID,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Adding '{coin_id}' as gas budget for Nexus");

    // TX

    // notify_success!(
    //     "New Nexus network created with ID: {id}",
    //     id = network_id.to_string().truecolor(100, 100, 100)
    // );

    // json_output(&json!({ "digest": response.digest, "network_id": network_id }))?;

    Ok(())
}
