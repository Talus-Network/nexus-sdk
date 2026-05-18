use super::*;

pub(crate) async fn handle_payments_command(
    command: PaymentsCommand,
) -> AnyResult<(), NexusCliError> {
    match command {
        PaymentsCommand::List {
            alias,
            agent_id,
            completed,
            pending,
            all: _,
        } => {
            let conf = CliConf::load().await.unwrap_or_default();
            let agent_id = if alias.is_some() || agent_id.is_some() {
                Some(agent_id_from_alias_or_arg(&conf, alias, agent_id)?)
            } else {
                None
            };
            let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
            let owner = nexus_client.signer().get_active_address();
            let history = fetch_execution_payment_history(
                nexus_client.crawler(),
                &nexus_client.get_nexus_objects(),
                owner,
                agent_id,
            )
            .await
            .map_err(NexusCliError::Any)?;
            let include = |receipt: &&TapExecutionPaymentReceipt| {
                (!completed && !pending)
                    || (completed && receipt.resolved)
                    || (pending && !receipt.resolved)
            };
            let wallet_receipts = history
                .wallet_receipts
                .iter()
                .filter(include)
                .cloned()
                .collect::<Vec<_>>();
            let vault_receipts = history
                .vault_receipts
                .iter()
                .filter(include)
                .cloned()
                .collect::<Vec<_>>();
            json_output(&json!({
                "owner": owner,
                "agent_id": agent_id,
                "wallet_receipts": wallet_receipts,
                "vault_receipts": vault_receipts,
                "unresolved_execution_ids": history.unresolved_execution_ids,
                "resolved_execution_ids": history.resolved_execution_ids
            }))
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
    async fn payments_alias_filter_errors_before_rpc_client() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::with_home(temp_home.path());

        let error = handle_payments_command(PaymentsCommand::List {
            alias: Some("missing".to_string()),
            agent_id: None,
            completed: true,
            pending: false,
            all: false,
        })
        .await
        .expect_err("missing alias should fail");

        assert!(
            error.to_string().contains("No Talus agent alias"),
            "unexpected error: {error}"
        );
    }
}
