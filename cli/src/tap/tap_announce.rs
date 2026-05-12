use super::*;

pub(crate) async fn announce_endpoint_revision(
    artifact: PathBuf,
    agent_id: sui::types::Address,
    skill_id: u64,
    endpoint_object_id: Option<sui::types::Address>,
    active_for_new_executions: bool,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    command_title!("Announcing TAP endpoint revision for '{agent_id}:{skill_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .announce_endpoint_revision(
            agent_id,
            skill_id,
            &artifact,
            endpoint_object_id,
            active_for_new_executions,
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&announce_result_json(&artifact, &result).map_err(NexusCliError::Any)?)
}
