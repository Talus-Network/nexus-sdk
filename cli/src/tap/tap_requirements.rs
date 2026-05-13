use {super::*, nexus_sdk::types::AgentId};

pub(crate) async fn fetch_requirements(
    agent_id: AgentId,
    skill_id: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Fetching TAP skill requirements for '{agent_id}:{skill_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let result = nexus_client
        .tap()
        .get_skill_requirements(agent_id, skill_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&requirements_result_json(&result))
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

    #[tokio::test]
    #[serial_test::serial]
    async fn fetch_requirements_reports_missing_rpc_before_network_use() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::without_sui_credentials(temp_home.path());

        let error = fetch_requirements(sui::types::Address::from_static("0xa"), 11)
            .await
            .expect_err("missing RPC should fail");

        assert!(
            error.to_string().contains("Sui RPC URL is not configured"),
            "unexpected error: {error}"
        );
    }
}
