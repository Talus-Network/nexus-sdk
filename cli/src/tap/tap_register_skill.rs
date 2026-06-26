use {super::*, nexus_sdk::types::AgentId};

pub(crate) async fn register_skill(
    artifact: PathBuf,
    agent_id: AgentId,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    command_title!("Registering TAP skill for agent '{}'", agent_id);

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    ensure_cli_mutable_agent(&nexus_client, agent_id).await?;
    let result = nexus_client
        .tap()
        .register_skill(agent_id, &artifact)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Registered TAP skill {skill_id}",
        skill_id = result.skill_id.to_string().truecolor(100, 100, 100)
    );
    json_output(&register_skill_result_json(&artifact, &result))
}

#[cfg(test)]
mod tests {
    use {super::*, std::ffi::OsString};

    struct EnvGuard {
        home: Option<OsString>,
        rpc: Option<OsString>,
        pk: Option<OsString>,
    }

    impl EnvGuard {
        fn without_sui_credentials(path: &std::path::Path) -> Self {
            let guard = Self {
                home: std::env::var_os("HOME"),
                rpc: std::env::var_os("SUI_RPC_URL"),
                pk: std::env::var_os("SUI_PK"),
            };
            std::env::set_var("HOME", path);
            std::env::remove_var("SUI_RPC_URL");
            std::env::remove_var("SUI_PK");
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.home.take() {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            match self.rpc.take() {
                Some(value) => std::env::set_var("SUI_RPC_URL", value),
                None => std::env::remove_var("SUI_RPC_URL"),
            }
            match self.pk.take() {
                Some(value) => std::env::set_var("SUI_PK", value),
                None => std::env::remove_var("SUI_PK"),
            }
        }
    }

    fn artifact_without_endpoint() -> TapPublishArtifact {
        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            requirements: nexus_sdk::types::SkillRequirements {
                input_schema_commitment: vec![1],
                payment_policy: nexus_sdk::types::SkillPaymentPolicy::default(),
                schedule_policy: nexus_sdk::types::SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: nexus_sdk::types::InterfaceVersion(1),
        };

        TapPublishArtifact::from_config(&config, sui::types::Address::from_static("0xd"))
            .expect("artifact")
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn register_reads_artifact_before_rpc_client() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::without_sui_credentials(temp_home.path());
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
            DEFAULT_GAS_BUDGET,
        )
        .await
        .expect_err("missing RPC is reached after artifact load");

        assert!(
            error.to_string().contains("Sui RPC URL is not configured"),
            "unexpected error: {error}"
        );
    }
}
