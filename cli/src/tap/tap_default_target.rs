use {super::*, nexus_sdk::nexus::tap::fetch_configured_default_tap_dag_executor};

pub(crate) async fn show_default_target() -> AnyResult<(), NexusCliError> {
    command_title!("Reading standard TAP default DAG executor");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let record = fetch_configured_default_tap_dag_executor(nexus_client.crawler(), nexus_objects)
        .await
        .map_err(NexusCliError::Any)?
        .data;

    notify_success!(
        "Default executor agent={agent} skill={skill}",
        agent = record.target.agent_id.to_string().truecolor(100, 100, 100),
        skill = record.target.skill_id.to_string().truecolor(100, 100, 100),
    );

    json_output(&default_target_result_json(&record))
}

pub(crate) fn default_target_result_json(
    record: &nexus_sdk::types::DefaultDagExecutorRecord,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "agent_id": record.target.agent_id,
        "skill_id": record.target.skill_id,
        "dag_id": record.skill.dag_id,
        "tap_package_id": record.skill.tap_package_id,
        "interface_revision": record.endpoint.key.interface_revision,
        "endpoint_object_id": record.endpoint.endpoint_object.object_id(),
        "endpoint_object_version": record.endpoint.endpoint_object.version(),
        "endpoint_object_digest_hex": hex::encode(record.endpoint.endpoint_object.digest().inner()),
        "requirements": record.endpoint.requirements,
    })
}
