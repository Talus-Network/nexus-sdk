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
    nexus_sdk::{events::NexusEventKind, sui, types::Storable},
};

fn terminal_err_eval_trace_entry(event: &NexusEventKind) -> serde_json::Value {
    match event {
        NexusEventKind::TerminalErrEvalRecorded(event) => json!({
            "terminal_err_eval": true,
            "vertex": event.vertex,
            "failure_class": event.failure_class.to_string(),
            "outcome": event.outcome.to_string(),
            "reason": event.reason,
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

/// Inspect a Nexus DAG execution process based on the provided object ID and
/// execution digest.
pub(crate) async fn inspect_dag_execution(
    dag_execution_id: sui::types::Address,
    execution_checkpoint: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Nexus DAG Execution '{dag_execution_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let mut result = nexus_client
        .workflow()
        .inspect_execution(dag_execution_id, execution_checkpoint, None)
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
                    variant = e.variant.name.truecolor(100, 100, 100),
                );

                let mut json_data = Vec::new();

                let fetched_data = e
                    .variant_ports_to_data
                    .fetch_all(&storage_conf)
                    .await
                    .map_err(|e| NexusCliError::Any(anyhow!(
                        "Failed to fetch data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                        e = e,
                        command = "$ nexus conf set --data-storage.walrus-aggregator-url <URL>",
                        testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                    )))?;

                for (port, data) in fetched_data {
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
                    "variant": e.variant.name,
                    "data": json_data,
                }));
            }

            NexusEventKind::EndStateReached(e) => {
                notify_success!(
                    "{end_state} Vertex '{vertex}' evaluated with output variant '{variant}'.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    variant = e.variant.name.truecolor(100, 100, 100),
                    end_state = "END STATE".truecolor(100, 100, 100)
                );

                let mut json_data = Vec::new();

                let fetched_data = e
                    .variant_ports_to_data
                    .fetch_all(&storage_conf)
                    .await
                    .map_err(|e| NexusCliError::Any(anyhow!(
                        "Failed to fetch data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                        e = e,
                        command = "$ nexus conf set --data-storage.walrus-aggregator-url <URL>",
                        testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                    )))?;

                for (port, data) in fetched_data {
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
                    "variant": e.variant.name,
                    "data": json_data,
                }));
            }

            NexusEventKind::WalkFailed(e) => {
                notify_error!(
                    "Vertex '{vertex}' failed to execute: '{reason}'.",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    reason = e.reason.truecolor(100, 100, 100),
                );
            }

            ref event_kind @ NexusEventKind::TerminalErrEvalRecorded(ref e) => {
                notify_error!(
                    "Terminal _err_eval recorded for vertex '{vertex}' as {failure_class} with outcome '{outcome}': '{reason}'.{duplicate_suffix}",
                    vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                    failure_class = e.failure_class.to_string().truecolor(100, 100, 100),
                    outcome = e.outcome.to_string().truecolor(100, 100, 100),
                    reason = e.reason.truecolor(100, 100, 100),
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

    json_output(&json_trace)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::types::{PostFailureAction, RuntimeVertex, TypeName, WorkflowFailureClass},
    };

    #[test]
    fn test_terminal_err_eval_trace_entry() {
        let event = NexusEventKind::TerminalErrEvalRecorded(
            nexus_sdk::events::TerminalErrEvalRecordedEvent {
                dag: sui::types::Address::ZERO,
                execution: sui::types::Address::TWO,
                walk_index: 5,
                vertex: RuntimeVertex::Plain {
                    vertex: TypeName::new("failable"),
                },
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalToolFailure,
                outcome: PostFailureAction::TransientContinue,
                reason: "tool failed".to_string(),
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
            nexus_sdk::events::TerminalErrEvalRecordedEvent {
                dag: sui::types::Address::ZERO,
                execution: sui::types::Address::TWO,
                walk_index: 0,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
                outcome: PostFailureAction::Terminate,
                reason: "timeout".to_string(),
                err_eval_hash: vec![],
                duplicate: true,
            },
        );

        let first = NexusEventKind::TerminalErrEvalRecorded(
            nexus_sdk::events::TerminalErrEvalRecordedEvent {
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
}
