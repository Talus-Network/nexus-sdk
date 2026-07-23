use {
    super::*,
    crate::tap::tap_output::payment_refill_result_json,
    nexus_sdk::nexus::tap::{
        fetch_execution_payment,
        RefillExecutionPaymentFromAgentVaultParams,
        RefillExecutionPaymentParams,
    },
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
        PaymentsCommand::Refill {
            execution_id,
            amount,
            alias,
            agent_id,
            gas,
        } => {
            refill_payment(
                execution_id,
                amount,
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

async fn refill_payment(
    execution_id: sui::types::Address,
    amount: u64,
    alias: Option<String>,
    agent_id: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Refilling standard TAP execution payment for DAGExecution '{execution_id}'");

    let conf = CliConf::load().await.unwrap_or_default();
    let resolved_agent_id = if alias.is_some() || agent_id.is_some() {
        Some(agent_id_from_alias_or_arg(&conf, alias, agent_id)?)
    } else {
        None
    };

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = if let Some(agent_id) = resolved_agent_id {
        ensure_cli_mutable_agent(&nexus_client, agent_id).await?;
        nexus_client
            .tap()
            .refill_execution_payment_from_agent_vault(RefillExecutionPaymentFromAgentVaultParams {
                execution_id,
                agent_id,
                amount,
            })
            .await
    } else {
        nexus_client
            .tap()
            .refill_execution_payment(RefillExecutionPaymentParams {
                execution_id,
                amount,
            })
            .await
    }
    .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Refill transaction submitted: {digest}",
        digest = result.tx_digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&payment_refill_result_json(&result))
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
    async fn payment_refill_alias_errors_before_rpc_client() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::with_home(temp_home.path());

        let error = handle_payments_command(PaymentsCommand::Refill {
            execution_id: sui::types::Address::from_static("0xee"),
            amount: 100,
            alias: Some("missing".to_string()),
            agent_id: None,
            gas: GasArgs {
                sui_gas_coin: None,
                sui_gas_budget: DEFAULT_GAS_BUDGET,
            },
        })
        .await
        .expect_err("missing alias should fail");

        assert!(
            error.to_string().contains("No Talus agent alias"),
            "unexpected error: {error}"
        );
    }
}
