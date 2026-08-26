use {super::*, nexus_sdk::nexus::tap::fetch_configured_agent_registry};

pub(crate) async fn show_registry() -> AnyResult<(), NexusCliError> {
    command_title!("Reading agent registry");

    let nexus_client = get_read_only_nexus_client().await?;
    let objects = nexus_client.get_nexus_objects();
    let context = nexus_client
        .context_for_root(&objects.agent_registry)
        .await
        .map_err(NexusCliError::Nexus)?;
    let registry = fetch_configured_agent_registry(&nexus_client, &context)
        .await
        .map_err(NexusCliError::Any)?
        .data;

    human_output(&render_registry(&registry));
    json_output(&registry_show_result_json(&registry))
}
