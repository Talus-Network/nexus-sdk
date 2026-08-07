use {super::*, nexus_sdk::nexus::tap::fetch_configured_agent_registry};

pub(crate) async fn show_registry() -> AnyResult<(), NexusCliError> {
    command_title!("Reading agent registry");

    let nexus_client = get_read_only_nexus_client().await?;
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let registry = fetch_configured_agent_registry(nexus_client.crawler(), nexus_objects)
        .await
        .map_err(NexusCliError::Any)?
        .data;

    human_output(&render_registry(&registry));
    json_output(&registry_show_result_json(&registry))
}
