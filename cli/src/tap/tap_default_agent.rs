//! Handler for inspecting the configured standard TAP default agent.

use {super::*, nexus_sdk::nexus::tap::fetch_configured_default_tap_dag_executor};

pub(crate) async fn show_default_agent() -> AnyResult<(), NexusCliError> {
    command_title!("Reading standard TAP default agent");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let record = fetch_configured_default_tap_dag_executor(nexus_client.crawler(), nexus_objects)
        .await
        .map_err(NexusCliError::Any)?
        .data;

    notify_success!(
        "Default agent={agent} skill={skill}",
        agent = record.target.agent_id.to_string().truecolor(100, 100, 100),
        skill = record.target.skill_id.to_string().truecolor(100, 100, 100),
    );

    json_output(&default_agent_result_json(&record))
}
