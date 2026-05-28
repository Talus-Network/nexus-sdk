use super::*;

pub(crate) async fn handle_vault_command(command: VaultCommand) -> AnyResult<(), NexusCliError> {
    match command {
        VaultCommand::Balance { alias, agent_id } => {
            let conf = CliConf::load().await.unwrap_or_default();
            let agent_id = agent_id_from_alias_or_arg(&conf, alias, agent_id)?;
            let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
            let vault = fetch_tap_agent_payment_vault_for_agent(nexus_client.crawler(), agent_id)
                .await
                .map_err(NexusCliError::Any)?;
            json_output(&json!({
                "agent_id": agent_id,
                "vault_id": vault.object_id,
                "available_balance": vault.data.available_balance,
                "locked_amount": vault.data.locked_amount,
                "unlocked_balance": vault.data.available_balance.saturating_sub(vault.data.locked_amount)
            }))
        }
        VaultCommand::Deposit {
            alias,
            agent_id,
            amount,
            gas,
        } => {
            deposit_agent_vault(
                alias,
                agent_id,
                amount,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::ffi::OsString};

    struct EnvGuard {
        home: Option<OsString>,
    }

    impl EnvGuard {
        fn with_home(path: &std::path::Path) -> Self {
            let guard = Self {
                home: std::env::var_os("HOME"),
            };
            std::env::set_var("HOME", path);
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.home.take() {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn vault_alias_lookup_errors_before_rpc_client() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::with_home(temp_home.path());

        let error = handle_vault_command(VaultCommand::Balance {
            alias: Some("missing".to_string()),
            agent_id: None,
        })
        .await
        .expect_err("missing alias should fail");

        assert!(
            error.to_string().contains("No Talus agent alias"),
            "unexpected error: {error}"
        );
    }
}
