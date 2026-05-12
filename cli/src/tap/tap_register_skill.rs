use {super::*, nexus_sdk::types::AgentId};

pub(crate) async fn register_skill(
    artifact: PathBuf,
    agent_id: AgentId,
    endpoint_object_id: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    let resolved_endpoint_object_id = artifact
        .endpoint_object_id_or(endpoint_object_id)
        .map_err(NexusCliError::Any)?;
    command_title!("Registering TAP skill for agent '{}'", agent_id);

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .register_skill(agent_id, &artifact, endpoint_object_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Registered TAP skill {skill_id}",
        skill_id = result.skill_id.to_string().truecolor(100, 100, 100)
    );
    json_output(&register_skill_result_json(
        &artifact,
        resolved_endpoint_object_id,
        &result,
    ))
}
