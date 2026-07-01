use {
    crate::{
        command_title,
        display::json_output,
        item,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{events::NexusEventKind, sui},
};

fn terminal_err_eval_trace_entry(event: &NexusEventKind) -> serde_json::Value {
    match event {
        NexusEventKind::TerminalErrEvalRecorded(event) => json!({
            "terminal_err_eval": true,
            "vertex": event.vertex,
            "failure_class": event.failure_class.to_string(),
            "outcome": event.outcome.0.as_ref().map(|outcome| outcome.to_string()),
            "reason": event.reason.as_str(),
            "duplicate": event.duplicate,
            "err_eval_hash": hex::encode(&event.err_eval_hash),
        }),
        _ => unreachable!("terminal_err_eval_trace_entry expects TerminalErrEvalRecorded"),
    }
}

fn terminal_err_eval_duplicate_suffix(event: &NexusEventKind) -> &'static str {
    match event {
        NexusEventKind::TerminalErrEvalRecorded(event) => {
            if event.duplicate {
                " Duplicate submission converged on the accepted terminal record."
            } else {
                ""
            }
        }
        _ => {
            unreachable!("terminal_err_eval_duplicate_suffix expects TerminalErrEvalRecorded")
        }
    }
}

/// Inspect a Nexus DAG execution process. The SDK derives the starting
/// checkpoint from the DAGExecution object's creation transaction (see
/// [`nexus_sdk::nexus::crawler::Crawler::get_object_creation_checkpoint`]),
/// so callers no longer need to thread it through.
pub(crate) async fn inspect_dag_execution(
    dag_execution_id: sui::types::Address,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Nexus DAG Execution '{dag_execution_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let mut result = nexus_client
        .workflow()
        .inspect_execution(dag_execution_id, None)
        .await
        .map_err(NexusCliError::Nexus)?;

    // Remote storage conf.
    let conf = CliConf::load().await.unwrap_or_default();
    let storage_conf = conf.data_storage.clone().into();

    let mut json_trace = Vec::new();

    while let Some(event) = result.next_event.recv().await {
        match event.data {
            NexusEventKind::WalkAdvanced(e) => {
                notify_success!(
                    "Vertex '{vertex}' evaluated with output variant '{variant}'.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    variant = e.variant.name.as_str().truecolor(100, 100, 100),
                );

                let mut json_data = Vec::new();

                let fetched_data = e.variant_ports_to_data
                    .clone()
                    .fetch_all(&storage_conf)
                    .await
                    .map_err(|e| NexusCliError::Any(anyhow!(
                        "Failed to fetch data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                        e = e,
                        command = "$ nexus conf set --data-storage.walrus-aggregator-url <URL>",
                        testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                    )))?;

                for (port, data) in fetched_data.into_map() {
                    let (display_data, json_data_value) = (
                        format!("{}", data.as_json()),
                        json!({ "port": port, "data": data.as_json(), "storage": data.storage_kind() }),
                    );

                    item!(
                        "Port '{port}' produced data: {data}",
                        port = port.truecolor(100, 100, 100),
                        data = display_data.truecolor(100, 100, 100),
                    );

                    json_data.push(json_data_value);
                }

                json_trace.push(json!({
                    "end_state": false,
                    "vertex": e.vertex,
                    "variant": e.variant.name.as_str(),
                    "data": json_data,
                }));
            }

            NexusEventKind::EndStateReached(e) => {
                notify_success!(
                    "{end_state} Vertex '{vertex}' evaluated with output variant '{variant}'.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    variant = e.variant.name.as_str().truecolor(100, 100, 100),
                    end_state = "END STATE".truecolor(100, 100, 100)
                );

                let mut json_data = Vec::new();

                let fetched_data = e.variant_ports_to_data
                    .clone()
                    .fetch_all(&storage_conf)
                    .await
                    .map_err(|e| NexusCliError::Any(anyhow!(
                        "Failed to fetch data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                        e = e,
                        command = "$ nexus conf set --data-storage.walrus-aggregator-url <URL>",
                        testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                    )))?;

                for (port, data) in fetched_data.into_map() {
                    let (display_data, json_data_value) = (
                        format!("{}", data.as_json()),
                        json!({ "port": port, "data": data.as_json(), "storage": data.storage_kind() }),
                    );

                    item!(
                        "Port '{port}' produced data: {data}",
                        port = port.truecolor(100, 100, 100),
                        data = display_data.truecolor(100, 100, 100),
                    );

                    json_data.push(json_data_value);
                }

                json_trace.push(json!({
                    "end_state": true,
                    "vertex": e.vertex,
                    "variant": e.variant.name.as_str(),
                    "data": json_data,
                }));
            }

            NexusEventKind::WalkFailed(e) => {
                notify_error!(
                    "Vertex '{vertex}' failed to execute: '{reason}'.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    reason = e.reason.as_str().truecolor(100, 100, 100),
                );
            }

            ref event_kind @ NexusEventKind::TerminalErrEvalRecorded(ref e) => {
                notify_error!(
                    "Terminal _err_eval recorded for vertex '{vertex}' as {failure_class} with outcome '{outcome}': '{reason}'.{duplicate_suffix}",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    failure_class = e.failure_class.to_string().truecolor(100, 100, 100),
                    outcome = e.outcome
                        .0
                        .as_ref()
                        .map(|outcome| outcome.to_string())
                        .unwrap_or_else(|| "none".to_string())
                        .truecolor(100, 100, 100),
                    reason = e.reason.as_str().truecolor(100, 100, 100),
                    duplicate_suffix = terminal_err_eval_duplicate_suffix(event_kind),
                );

                json_trace.push(terminal_err_eval_trace_entry(event_kind));
            }

            NexusEventKind::WalkAborted(e) => {
                notify_error!(
                    "Vertex '{vertex}' was aborted by a third party due to a timeout.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                );
            }

            NexusEventKind::WalkCancelled(e) => {
                notify_error!(
                    "Vertex '{vertex}' was cancelled because another walk was aborted.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                );
            }

            NexusEventKind::ExecutionFinished(e) => {
                if e.has_any_walk_failed {
                    notify_error!("DAG execution finished unsuccessfully");

                    break;
                }

                if e.was_aborted {
                    notify_error!("DAG execution was aborted by a third party due to a timeout");

                    break;
                }

                notify_success!("DAG execution finished successfully");

                break;
            }

            _ => {}
        }
    }

    await_poller_outcome(result.poller).await?;

    json_output(&json_trace)?;

    Ok(())
}

/// Drain the SDK's event-poller `JoinHandle` after the CLI's inspection
/// loop ends and surface any error it reported.
pub(crate) async fn await_poller_outcome(
    poller: tokio::task::JoinHandle<Result<(), nexus_sdk::nexus::error::NexusError>>,
) -> Result<(), NexusCliError> {
    let outcome = poller.await.map_err(|join_err| {
        NexusCliError::Any(anyhow!(
            "DAG execution inspection task failed to join: {join_err}"
        ))
    })?;

    outcome.map_err(NexusCliError::Nexus)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::types::{
            sui_address_to_id,
            MoveOption,
            MoveString,
            PostFailureAction,
            RuntimeVertex,
            TypeName,
            WorkflowFailureClass,
        },
    };

    #[test]
    fn test_terminal_err_eval_trace_entry() {
        let event = NexusEventKind::TerminalErrEvalRecorded(
            nexus_sdk::types::workflow::execution_events::TerminalErrEvalRecordedEvent {
                dag: sui_address_to_id(sui::types::Address::ZERO),
                execution: sui_address_to_id(sui::types::Address::TWO),
                walk_index: 5,
                vertex: RuntimeVertex::Plain {
                    vertex: TypeName::new("failable").into(),
                },
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalToolFailure,
                outcome: MoveOption(Some(PostFailureAction::TransientContinue)),
                reason: MoveString::from("tool failed"),
                err_eval_hash: vec![0xab, 0xcd],
                duplicate: true,
            },
        );

        assert_eq!(
            terminal_err_eval_trace_entry(&event),
            json!({
                "terminal_err_eval": true,
                "vertex": match &event {
                    NexusEventKind::TerminalErrEvalRecorded(event) => event.vertex.clone(),
                    _ => unreachable!("constructed as TerminalErrEvalRecorded"),
                },
                "failure_class": "terminal_tool_failure",
                "outcome": "continue",
                "reason": "tool failed",
                "duplicate": true,
                "err_eval_hash": "abcd",
            })
        );
    }

    #[test]
    fn test_terminal_err_eval_duplicate_suffix() {
        let duplicate = NexusEventKind::TerminalErrEvalRecorded(
            nexus_sdk::types::workflow::execution_events::TerminalErrEvalRecordedEvent {
                dag: sui_address_to_id(sui::types::Address::ZERO),
                execution: sui_address_to_id(sui::types::Address::TWO),
                walk_index: 0,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
                outcome: MoveOption(Some(PostFailureAction::Terminate)),
                reason: MoveString::from("timeout"),
                err_eval_hash: vec![],
                duplicate: true,
            },
        );

        let first = NexusEventKind::TerminalErrEvalRecorded(
            nexus_sdk::types::workflow::execution_events::TerminalErrEvalRecordedEvent {
                duplicate: false,
                ..match &duplicate {
                    NexusEventKind::TerminalErrEvalRecorded(event) => event.clone(),
                    _ => unreachable!("constructed as TerminalErrEvalRecorded"),
                }
            },
        );

        assert!(terminal_err_eval_duplicate_suffix(&duplicate).contains("Duplicate submission"));
        assert_eq!(terminal_err_eval_duplicate_suffix(&first), "");
    }

    /// Happy path: the SDK poller returned `Ok(())` (it saw
    /// `ExecutionFinished` and shut down cleanly). The CLI's
    /// `await_poller_outcome` must return `Ok(())` so the json trace is
    /// emitted to stdout.
    #[tokio::test]
    async fn await_poller_outcome_passes_through_ok_terminal() {
        let handle = tokio::spawn(async { Ok::<(), nexus_sdk::nexus::error::NexusError>(()) });
        let result = await_poller_outcome(handle).await;
        assert!(
            result.is_ok(),
            "successful poller termination must not surface as a CLI error: {result:?}"
        );
    }

    /// Pruned-checkpoint regression: when the SDK's catch-up loop hits a
    /// gRPC `NotFound` for a checkpoint and surfaces a
    /// `NexusError::Channel(...)` via `send_page`, the inspection task
    /// terminates with `Err(...)`. Without this propagation the CLI would
    /// just print `[]` and exit 0 — exactly the silent-empty-trace bug we
    /// hit on the TAP development guide walkthrough. Asserts the failure
    /// is bubbled up to `NexusCliError::Nexus` with the original message
    /// intact.
    #[tokio::test]
    async fn await_poller_outcome_surfaces_nexus_error_when_poller_fails() {
        let original = "Error fetching events: GRPC error: Failed to fetch checkpoint 108515 \
             during catch-up: code: 'Some requested entity was not found'";
        let original_owned = original.to_string();
        let handle = tokio::spawn(async move {
            Err::<(), nexus_sdk::nexus::error::NexusError>(
                nexus_sdk::nexus::error::NexusError::Channel(anyhow!(original_owned)),
            )
        });

        let err = await_poller_outcome(handle)
            .await
            .expect_err("poller error must reach the CLI caller");

        let rendered = format!("{err}");
        assert!(
            rendered.contains("Channel error"),
            "expected NexusCliError::Nexus(Channel(...)), got: {rendered}"
        );
        assert!(
            rendered.contains("Checkpoint 108515 not found")
                || rendered.contains("108515 during catch-up"),
            "underlying RPC reason must be preserved in the surfaced error: {rendered}"
        );
    }

    /// Sanity-check that a panicked poller task (not just an `Err`)
    /// surfaces as a clear join-failure error instead of being misrendered
    /// as a "no events" result.
    #[tokio::test]
    async fn await_poller_outcome_surfaces_join_error_when_task_panics() {
        let handle = tokio::spawn(async {
            panic!("simulated poller panic");
            #[allow(unreachable_code)]
            Ok::<(), nexus_sdk::nexus::error::NexusError>(())
        });

        let err = await_poller_outcome(handle)
            .await
            .expect_err("panicked poller must produce a CLI error");
        let rendered = format!("{err}");
        assert!(
            rendered.contains("failed to join"),
            "expected join-failure message, got: {rendered}"
        );
    }
}
