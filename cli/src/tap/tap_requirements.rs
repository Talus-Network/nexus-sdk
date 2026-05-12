use {super::*, nexus_sdk::types::AgentId};

pub(crate) async fn fetch_requirements(
    agent_id: AgentId,
    skill_id: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Fetching TAP skill requirements for '{agent_id}:{skill_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let result = nexus_client
        .tap()
        .get_skill_requirements(agent_id, skill_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&requirements_result_json(&result))
}
