use {
    super::*,
    nexus_sdk::nexus::tap::{fetch_execution_payment, AccomplishExecutionPaymentParams},
    std::time::Duration,
};

pub(crate) async fn handle_payments_command(
    command: PaymentsCommand,
) -> AnyResult<(), NexusCliError> {
    match command {
        PaymentsCommand::Show { payment_id } => show_payment(payment_id).await,
        PaymentsCommand::Wait {
            payment_id,
            timeout_secs,
            poll_secs,
        } => wait_payment(payment_id, timeout_secs, poll_secs).await,
        PaymentsCommand::List {
            alias,
            agent_id,
            completed,
            pending,
            all: _,
        } => list_payments(alias, agent_id, completed, pending).await,
        PaymentsCommand::Resolve {
            execution_id,
            alias,
            agent_id,
            gas,
        } => {
            resolve_payment(
                execution_id,
                alias,
                agent_id,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}

async fn show_payment(payment_id: sui::types::Address) -> AnyResult<(), NexusCliError> {
    command_title!("Reading standard TAP execution payment '{payment_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let payment = fetch_execution_payment(nexus_client.crawler(), payment_id)
        .await
        .map_err(NexusCliError::Any)?
        .data;

    json_output(&payment_show_result_json(&payment))
}

async fn wait_payment(
    payment_id: sui::types::Address,
    timeout_secs: u64,
    poll_secs: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Waiting for standard TAP payment '{payment_id}' to settle");

    if poll_secs == 0 {
        return Err(NexusCliError::Any(anyhow!(
            "--poll-secs must be greater than zero"
        )));
    }

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let result = nexus_client
        .tap()
        .wait_for_payment_settled(
            payment_id,
            Duration::from_secs(timeout_secs),
            Duration::from_secs(poll_secs),
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&payment_wait_result_json(&result))
}

/// Fetch the local-wallet execution payment receipts and, optionally, the
/// receipts owned by an agent's vault. The `--completed` / `--pending` flags
/// post-filter the receipts before emission so scripted consumers do not
/// have to drop fields client-side.
async fn list_payments(
    alias: Option<String>,
    agent_id: Option<sui::types::Address>,
    completed: bool,
    pending: bool,
) -> AnyResult<(), NexusCliError> {
    let conf = CliConf::load().await.unwrap_or_default();
    let agent_id = if alias.is_some() || agent_id.is_some() {
        Some(agent_id_from_alias_or_arg(&conf, alias, agent_id)?)
    } else {
        None
    };
    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    if let Some(agent_id) = agent_id {
        ensure_cli_agent_owner(&nexus_client, agent_id).await?;
    }
    let owner = nexus_client.signer().get_active_address();
    let history = fetch_execution_payment_history(
        nexus_client.crawler(),
        &nexus_client.get_nexus_objects(),
        owner,
        agent_id,
    )
    .await
    .map_err(NexusCliError::Any)?;
    let include = |receipt: &&ExecutionPaymentReceipt| {
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
    json_output(&payments_list_result_json(
        owner,
        agent_id,
        &wallet_receipts,
        &vault_receipts,
        &history.unresolved_execution_ids,
        &history.resolved_execution_ids,
    ))
}

/// Wrap the on-chain `nexus_workflow::dag::accomplish_tap_execution_payment*`
/// PTBs. With no agent supplied, the SDK builds the invoker-funded PTB
/// (`accomplish_tap_execution_payment`) and the shared `DAGExecution` is
/// the only input. When `--alias` or `--agent-id` resolves to an agent,
/// the SDK additionally fetches the agent's object ref and routes through
/// `accomplish_tap_execution_payment_from_agent_vault` so the payment
/// settles out of the agent's vault.
async fn resolve_payment(
    execution_id: sui::types::Address,
    alias: Option<String>,
    agent_id: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Resolving standard TAP execution payment for DAGExecution '{execution_id}'");

    let conf = CliConf::load().await.unwrap_or_default();
    let resolved_agent_id = if alias.is_some() || agent_id.is_some() {
        Some(agent_id_from_alias_or_arg(&conf, alias, agent_id)?)
    } else {
        None
    };

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    if let Some(agent_id) = resolved_agent_id {
        ensure_cli_mutable_agent(&nexus_client, agent_id).await?;
    }
    let result = nexus_client
        .tap()
        .accomplish_execution_payment(AccomplishExecutionPaymentParams {
            execution_id,
            agent_id: resolved_agent_id,
        })
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Resolved execution payment (digest {digest})",
        digest = result.tx_digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&payment_resolve_result_json(&result))
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
