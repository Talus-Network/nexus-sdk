use {
    super::*,
    nexus_sdk::nexus::tap::{
        fetch_tap_execution_payment,
        payment_is_terminal,
        WaitForPaymentResult,
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

async fn show_payment(payment_id: sui::types::Address) -> AnyResult<(), NexusCliError> {
    command_title!("Reading standard TAP execution payment '{payment_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let payment = fetch_tap_execution_payment(nexus_client.crawler(), payment_id)
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

pub(crate) fn payment_show_result_json(
    payment: &nexus_sdk::types::TapExecutionPayment,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "payment_id": payment.id,
        "execution_id": payment.execution_id,
        "agent_id": payment.agent_id,
        "skill_id": payment.skill_id,
        "interface_revision": payment.interface_revision,
        "endpoint_object_id": payment.endpoint_object_id,
        "payer": payment.payer,
        "payment_mode": payment.payment_mode,
        "source_kind": payment.source_kind,
        "source_identity": payment.source_identity,
        "max_budget": payment.max_budget,
        "locked_budget": payment.locked_budget,
        "consumed": payment.consumed,
        "refund_mode": payment.refund_mode,
        "accomplished": payment.accomplished,
        "refunded": payment.refunded,
        "final_state": payment.final_state,
        "terminal": payment_is_terminal(payment),
        "locked_vertices": payment.locked_vertices,
    })
}

pub(crate) fn payment_wait_result_json(result: &WaitForPaymentResult) -> serde_json::Value {
    let mut base = payment_show_result_json(&result.payment);
    let object = base.as_object_mut().expect("payment show returns object");
    object.insert("elapsed_ms".to_string(), json!(result.elapsed_ms));
    object.insert("timed_out".to_string(), json!(result.timed_out));
    object.insert("terminal".to_string(), json!(result.terminal));
    base
}

#[cfg(test)]
mod json_shape_tests {
    use {
        super::*,
        nexus_sdk::types::{InterfaceRevision, TapExecutionPayment, TapPaymentMode},
    };

    fn fixture_payment(accomplished: bool, refunded: bool) -> TapExecutionPayment {
        TapExecutionPayment {
            id: sui::types::Address::from_static("0xaa"),
            execution_id: sui::types::Address::from_static("0xbb"),
            agent_id: sui::types::Address::from_static("0xcc"),
            skill_id: 11,
            interface_revision: InterfaceRevision(2),
            endpoint_object_id: sui::types::Address::from_static("0xdd"),
            payer: sui::types::Address::from_static("0xee"),
            payment_mode: TapPaymentMode::UserFunded,
            source_kind: None,
            source_identity: None,
            max_budget: 1_000,
            locked_budget: 0,
            consumed: 0,
            refund_mode: 0,
            payment_source_hash: vec![],
            accomplished,
            refunded,
            final_state: None,
            locked_vertices: vec![],
        }
    }

    #[test]
    fn payment_show_result_json_includes_terminal_flag() {
        let json = payment_show_result_json(&fixture_payment(true, false));
        assert_eq!(json["standard_tap"], serde_json::Value::Bool(true));
        assert_eq!(json["accomplished"], serde_json::Value::Bool(true));
        assert_eq!(json["refunded"], serde_json::Value::Bool(false));
        assert_eq!(json["terminal"], serde_json::Value::Bool(true));
        assert_eq!(json["skill_id"], serde_json::json!(11));
    }

    #[test]
    fn payment_wait_result_json_adds_elapsed_and_timeout_flags() {
        let wait = WaitForPaymentResult {
            payment: fixture_payment(false, false),
            terminal: false,
            elapsed_ms: 1234,
            timed_out: true,
        };
        let json = payment_wait_result_json(&wait);
        assert_eq!(json["elapsed_ms"], serde_json::json!(1234));
        assert_eq!(json["timed_out"], serde_json::Value::Bool(true));
        assert_eq!(json["terminal"], serde_json::Value::Bool(false));
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
