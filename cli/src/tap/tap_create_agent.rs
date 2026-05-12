use super::*;

pub(crate) async fn create_agent(
    operator: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Creating Talus agent");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .create_agent(operator)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Created Talus agent {agent_id}",
        agent_id = result.agent_id.to_string().truecolor(100, 100, 100)
    );
    json_output(&create_agent_result_json(operator, &result))
}
