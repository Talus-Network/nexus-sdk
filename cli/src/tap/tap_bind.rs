use {
    super::*,
    nexus_sdk::nexus::tap::{BindAgentSkillParams, BindAgentSkillResult},
};

pub(crate) async fn bind_agent_skill(
    artifact_path: PathBuf,
    operator: sui::types::Address,
    endpoint_object_id: Option<sui::types::Address>,
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
            endpoint_object_id,
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

pub(crate) fn bind_result_json(
    artifact: &nexus_sdk::types::TapPublishArtifact,
    operator: sui::types::Address,
    result: &BindAgentSkillResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": "bind_agent_skill",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "operator": operator,
        "agent_id": result.agent_id,
        "agent_object_id": result.agent_object.object_id(),
        "agent_object_version": result.agent_object.version(),
        "skill_id": result.skill_id,
        "dag_id": artifact.dag_id,
        "tap_package_id": artifact.tap_package_id,
        "endpoint_object_id": result.endpoint_object.object_id(),
        "endpoint_object_version": result.endpoint_object.version(),
        "endpoint_object_digest_hex": hex::encode(result.endpoint_object.digest().inner()),
        "config_digest_hex": hex::encode(&result.config_digest),
        "config_digest_input": result.config_digest_input,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            nexus::tap::BindAgentSkillResult,
            types::{
                InterfaceRevision,
                TapConfigDigestInput,
                TapPaymentPolicy,
                TapPublishArtifact,
                TapSchedulePolicy,
                TapSkillConfig,
                TapSkillRequirements,
                TapVertexAuthorizationSchema,
            },
        },
    };

    fn fixture_artifact() -> TapPublishArtifact {
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: TapSkillRequirements {
                input_schema_commitment: vec![1],
                workflow_commitment: vec![2],
                metadata_commitment: vec![3],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
            },
            shared_objects: vec![],
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };
        TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xdd"),
            sui::types::Address::from_static("0xee"),
        )
        .expect("artifact builds")
    }

    #[test]
    fn bind_result_json_exposes_combined_evidence() {
        let artifact = fixture_artifact();
        let endpoint_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xfe"),
            5,
            sui::types::Digest::from([4u8; 32]),
        );
        let result = BindAgentSkillResult {
            tx_digest: sui::types::Digest::from([7u8; 32]),
            tx_checkpoint: 100,
            agent_id: sui::types::Address::from_static("0xa1"),
            agent_object: sui::types::ObjectReference::new(
                sui::types::Address::from_static("0xa1"),
                3,
                sui::types::Digest::from([5u8; 32]),
            ),
            skill_id: 7,
            endpoint_object: endpoint_ref,
            config_digest: vec![9u8; 32],
            config_digest_input: TapConfigDigestInput {
                package_id: artifact.tap_package_id,
                endpoint_object_id: Some(sui::types::Address::from_static("0xfe")),
                interface_revision: InterfaceRevision(1),
                shared_objects: vec![],
                requirements: artifact.requirements.clone(),
            },
        };
        let json = bind_result_json(
            &artifact,
            sui::types::Address::from_static("0x2"),
            &result,
        );
        assert_eq!(json["function"], "bind_agent_skill");
        assert_eq!(
            json["agent_id"],
            serde_json::json!(sui::types::Address::from_static("0xa1").to_string())
        );
        assert_eq!(json["skill_id"], serde_json::json!(7));
        assert_eq!(
            json["endpoint_object_id"],
            serde_json::json!(sui::types::Address::from_static("0xfe").to_string())
        );
        assert_eq!(json["endpoint_object_version"], serde_json::json!(5));
        assert_eq!(json["config_digest_hex"].as_str().unwrap().len(), 64);
        assert_eq!(json["tx_checkpoint"], serde_json::json!(100));
    }
}
