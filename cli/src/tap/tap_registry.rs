use {super::*, nexus_sdk::nexus::tap::fetch_configured_agent_registry};

pub(crate) async fn show_registry() -> AnyResult<(), NexusCliError> {
    command_title!("Reading agent registry");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let registry = fetch_configured_agent_registry(nexus_client.crawler(), nexus_objects)
        .await
        .map_err(NexusCliError::Any)?
        .data;

    notify_success!(
        "Registry {id} ({agents} agents, {skills} skills, {endpoints} endpoint revisions)",
        id = registry.id.to_string().truecolor(100, 100, 100),
        agents = registry.agents.len(),
        skills = registry.skills.len(),
        endpoints = registry.endpoints.len(),
    );

    json_output(&registry_show_result_json(&registry))
}

pub(crate) fn registry_show_result_json(
    registry: &nexus_sdk::types::TapRegistry,
) -> serde_json::Value {
    json!({
        "id": registry.id,
        "default_executor": registry.default_executor,
        "agents": registry.agents,
        "skills": registry.skills,
        "endpoints": registry.endpoints,
    })
}
