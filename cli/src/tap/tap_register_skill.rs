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

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact_without_endpoint() -> TapPublishArtifact {
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: nexus_sdk::types::TapSkillRequirements {
                input_schema_commitment: vec![1],
                workflow_commitment: vec![2],
                metadata_commitment: vec![3],
                payment_policy: nexus_sdk::types::TapPaymentPolicy::default(),
                schedule_policy: nexus_sdk::types::TapSchedulePolicy::default(),
                vertex_authorization_schema:
                    nexus_sdk::types::TapVertexAuthorizationSchema::default(),
            },
            shared_objects: Vec::new(),
            interface_revision: nexus_sdk::types::InterfaceRevision(1),
            active_for_new_executions: true,
        };

        TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
        )
        .expect("artifact")
    }

    #[tokio::test]
    async fn register_requires_endpoint_metadata_before_rpc_client() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let artifact_path = tempdir.path().join("artifact.json");
        tokio::fs::write(
            &artifact_path,
            serde_json::to_string(&artifact_without_endpoint()).expect("serialize artifact"),
        )
        .await
        .expect("write artifact");

        let error = register_skill(
            artifact_path,
            sui::types::Address::from_static("0xa"),
            None,
            None,
            DEFAULT_GAS_BUDGET,
        )
        .await
        .expect_err("endpoint metadata is required");

        assert!(
            error
                .to_string()
                .contains("TAP endpoint object ID is required"),
            "unexpected error: {error}"
        );
    }
}
