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
    async fn create_agent_reports_missing_rpc_before_network_use() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::without_sui_credentials(temp_home.path());

        let error = create_agent(
            sui::types::Address::from_static("0x2"),
            None,
            DEFAULT_GAS_BUDGET,
        )
        .await
        .expect_err("missing RPC should fail");

        assert!(
            error.to_string().contains("Sui RPC URL is not configured"),
            "unexpected error: {error}"
        );
    }
}
