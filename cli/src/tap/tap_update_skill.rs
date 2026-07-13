use {super::*, nexus_sdk::types::AgentId};

pub(crate) async fn update_skill_from_artifact(
    artifact: PathBuf,
    agent_id: AgentId,
    skill_id: SkillId,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    command_title!("Updating TAP skill {skill_id} for agent '{agent_id}' from publish artifact");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    ensure_cli_mutable_agent(&nexus_client, agent_id).await?;
    let result = nexus_client
        .tap()
        .update_skill_from_artifact(agent_id, skill_id, &artifact)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Updated TAP skill {skill_id} to revision {revision}",
        skill_id = result.skill_id.to_string().truecolor(100, 100, 100),
        revision = result
            .current_interface_revision
            .inner
            .to_string()
            .truecolor(100, 100, 100),
    );
    json_output(&update_skill_result_json(&artifact, &result))
}
