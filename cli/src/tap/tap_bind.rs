use {super::*, nexus_sdk::nexus::tap::BindAgentSkillParams};

pub(crate) async fn bind_agent_skill(
    artifact_path: PathBuf,
    operator: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Binding agent and skill from publish artifact");

    let artifact = read_artifact(artifact_path).await?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;

    let result = nexus_client
        .tap()
        .bind_agent_skill(BindAgentSkillParams {
            operator,
            artifact: artifact.clone(),
        })
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Agent {agent_id} bound with skill {skill_id}",
        agent_id = result.agent_id.to_string().truecolor(100, 100, 100),
        skill_id = result.skill_id.to_string().truecolor(100, 100, 100),
    );

    json_output(&bind_result_json(&artifact, operator, &result))
}
