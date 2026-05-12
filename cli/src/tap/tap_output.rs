use super::*;

pub(crate) fn create_agent_result_json(
    operator: sui::types::Address,
    result: &CreateAgentResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::CREATE_AGENT.name.to_string(),
        "agent_id": result.agent_id,
        "operator": operator,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
    })
}

pub(crate) fn publish_skill_result_json(result: &PublishSkillResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": "publish_skill",
        "tap_package_id": result.tap_package.package_id,
        "tap_package_digest": result.tap_package.tx_digest,
        "tap_package_checkpoint": result.tap_package.tx_checkpoint,
        "dag_id": result.dag.dag_object_id,
        "dag_digest": result.dag.tx_digest,
        "dag_checkpoint": result.dag.tx_checkpoint,
        "endpoint_object_id": result.endpoint.endpoint_object.object_id(),
        "endpoint_object_version": result.endpoint.endpoint_object.version(),
        "endpoint_object_digest": result.endpoint.endpoint_object.digest(),
        "endpoint_digest": result.endpoint.tx_digest,
        "endpoint_checkpoint": result.endpoint.tx_checkpoint,
        "artifact": result.artifact,
    })
}

pub(crate) fn register_skill_result_json(
    artifact: &TapPublishArtifact,
    endpoint_object_id: sui::types::Address,
    result: &RegisterSkillResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::REGISTER_SKILL.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "dag_id": artifact.dag_id,
        "tap_package_id": artifact.tap_package_id,
        "endpoint_object_id": endpoint_object_id,
    })
}

pub(crate) fn announce_result_json(
    artifact: &TapPublishArtifact,
    result: &AnnounceEndpointRevisionResult,
) -> anyhow::Result<serde_json::Value> {
    Ok(json!({
        "standard_tap": true,
        "function": TapStandard::ANNOUNCE_ENDPOINT_REVISION.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "endpoint_key": result.endpoint_key,
        "endpoint_object_id": result.endpoint_object.object_id(),
        "endpoint_object_version": result.endpoint_object.version(),
        "tap_package_id": artifact.tap_package_id,
        "config_digest_hex": hex::encode(&result.config_digest),
        "config_digest_input": result.config_digest_input,
    }))
}

pub(crate) fn requirements_result_json(result: &GetSkillRequirementsResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::GET_SKILL_REQUIREMENTS.name.to_string(),
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "active_endpoint_key": result.active_endpoint_key,
        "requirements": result.requirements,
    })
}

pub(crate) fn schedule_result_json(
    long_term_gas_coin_id: sui::types::Address,
    result: &ScheduleSkillExecutionResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::SCHEDULE_SKILL_EXECUTION.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "long_term_gas_coin_id": long_term_gas_coin_id,
    })
}
