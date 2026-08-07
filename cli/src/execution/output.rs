use nexus_sdk::{
    events::{NexusEvent, NexusEventKind},
    scheduler::OccurrenceRef,
};

pub(super) fn event_result_json(event: &NexusEvent) -> anyhow::Result<serde_json::Value> {
    let serialized = serde_json::to_value(&event.data)?;
    let payload = match serialized {
        serde_json::Value::Object(mut object) => object
            .remove("event")
            .unwrap_or(serde_json::Value::Object(object)),
        value => value,
    };
    let generics = event
        .generics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let distribution = event.distribution.as_ref().map(|metadata| {
        serde_json::json!({
            "deadline_ms": metadata.deadline.num_milliseconds(),
            "requested_at": metadata.requested_at,
            "leaders": metadata.leaders,
            "task_id": metadata.task_id,
        })
    });

    Ok(serde_json::json!({
        "transaction_digest": event.id.0,
        "event_sequence": event.id.1,
        "emitting_package": event.emitting_package,
        "generics": generics,
        "event_type": event.data.name(),
        "event": normalize_move_value(payload, None),
        "distribution": distribution,
        "summary": render_event(&event.data),
    }))
}

fn normalize_move_value(value: serde_json::Value, field: Option<&str>) -> serde_json::Value {
    if field == Some("vertex") {
        if let Some(vertex) = normalize_runtime_vertex(&value) {
            return vertex;
        }
    }

    match value {
        serde_json::Value::Object(mut object) if object.len() == 1 => {
            if let Some(bytes) = object.remove("bytes") {
                return match bytes {
                    serde_json::Value::String(address) => serde_json::Value::String(address),
                    bytes => normalize_byte_value(bytes, field).1,
                };
            }
            if let Some(inner) = object.remove("inner") {
                return normalize_move_value(inner, field);
            }
            if let Some(serde_json::Value::Array(mut values)) = object.remove("vec") {
                return match values.len() {
                    0 => serde_json::Value::Null,
                    1 => normalize_move_value(values.remove(0), field),
                    _ => serde_json::Value::Array(
                        values
                            .into_iter()
                            .map(|value| normalize_move_value(value, field))
                            .collect(),
                    ),
                };
            }

            normalize_object(object)
        }
        serde_json::Value::Object(object) => normalize_object(object),
        serde_json::Value::Array(values) => {
            if json_bytes(&values).is_some() {
                normalize_byte_value(serde_json::Value::Array(values), field).1
            } else {
                serde_json::Value::Array(
                    values
                        .into_iter()
                        .map(|value| normalize_move_value(value, field))
                        .collect(),
                )
            }
        }
        value => value,
    }
}

fn normalize_object(object: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    let mut normalized = serde_json::Map::new();
    for (field, value) in object {
        let (field, value) = normalize_named_value(field, value);
        normalized.insert(field, value);
    }
    serde_json::Value::Object(normalized)
}

fn normalize_named_value(field: String, value: serde_json::Value) -> (String, serde_json::Value) {
    if let Some(bytes) = byte_value(&value) {
        let text_field = matches!(
            field.as_str(),
            "description"
                | "function"
                | "function_name"
                | "message"
                | "module"
                | "module_name"
                | "name"
                | "reason"
                | "result_key"
                | "tool_fqn"
                | "url"
                | "vertex_key"
        );
        if text_field {
            if let Ok(text) = String::from_utf8(bytes.clone()) {
                return (field, serde_json::Value::String(text));
            }
        }

        let field = if field.ends_with("_hex") {
            field
        } else {
            format!("{field}_hex")
        };
        return (field, serde_json::Value::String(hex::encode(bytes)));
    }

    let normalized = normalize_move_value(value, Some(&field));
    (field, normalized)
}

fn normalize_byte_value(
    value: serde_json::Value,
    field: Option<&str>,
) -> (Option<String>, serde_json::Value) {
    let Some(bytes) = byte_value(&value) else {
        return (None, normalize_move_value(value, field));
    };
    if field.is_some_and(|field| {
        matches!(
            field,
            "description"
                | "function"
                | "function_name"
                | "message"
                | "module"
                | "module_name"
                | "name"
                | "reason"
                | "result_key"
                | "tool_fqn"
                | "url"
                | "vertex_key"
        )
    }) {
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            return (
                field.map(ToOwned::to_owned),
                serde_json::Value::String(text),
            );
        }
    }

    (
        field.map(|field| format!("{field}_hex")),
        serde_json::Value::String(hex::encode(bytes)),
    )
}

fn byte_value(value: &serde_json::Value) -> Option<Vec<u8>> {
    match value {
        serde_json::Value::Array(values) => json_bytes(values),
        serde_json::Value::Object(object) if object.len() == 1 => object
            .get("bytes")
            .and_then(serde_json::Value::as_array)
            .and_then(|values| json_bytes(values)),
        _ => None,
    }
}

fn json_bytes(values: &[serde_json::Value]) -> Option<Vec<u8>> {
    if values.is_empty() {
        return None;
    }

    values
        .iter()
        .map(|value| value.as_u64().and_then(|byte| u8::try_from(byte).ok()))
        .collect()
}

fn normalize_runtime_vertex(value: &serde_json::Value) -> Option<serde_json::Value> {
    let object = value.as_object()?;
    if let Some(plain) = object.get("Plain") {
        return runtime_vertex_name(plain).map(serde_json::Value::String);
    }
    let iterator = object.get("WithIterator")?.as_object()?;
    Some(serde_json::json!({
        "name": runtime_vertex_name(&serde_json::Value::Object(iterator.clone()))?,
        "iteration": iterator.get("iteration")?,
        "out_of": iterator.get("out_of")?,
    }))
}

fn runtime_vertex_name(value: &serde_json::Value) -> Option<String> {
    let bytes = value
        .get("vertex")?
        .get("name")?
        .get("bytes")?
        .as_array()
        .and_then(|values| json_bytes(values))?;
    String::from_utf8(bytes).ok()
}

pub(super) fn render_occurrence_command(reference: &OccurrenceRef) -> String {
    format!(
        "\nNext command\nnexus task occurrence inspect --task-id {} --occurrence-id {}\n",
        reference.task_id(),
        reference.occurrence_id(),
    )
}

pub(super) fn render_event(event: &NexusEventKind) -> Option<String> {
    match event {
        NexusEventKind::OccurrenceDispatched(event) => Some(format!(
            "Task {} occurrence {} dispatched this Execution",
            event.task_id.bytes, event.occurrence_id,
        )),
        NexusEventKind::WalkAdvanced(event) => Some(format!(
            "Vertex {} produced variant {}",
            event.vertex,
            event.variant.name.as_str(),
        )),
        NexusEventKind::EndStateReached(event) => Some(format!(
            "End state {} produced variant {}",
            event.vertex,
            event.variant.name.as_str(),
        )),
        NexusEventKind::WalkFailed(event) => Some(format!(
            "Vertex {} failed: {}",
            event.vertex,
            event.reason.as_str(),
        )),
        NexusEventKind::SubmissionFailureEvidenceRecorded(event) => Some(format!(
            "Vertex {} rejected submission evidence: {}",
            event.vertex,
            event.reason.as_str(),
        )),
        NexusEventKind::TerminalErrEvalRecorded(event) => Some(format!(
            "Vertex {} recorded {}: {}",
            event.vertex,
            event.failure_class,
            event.reason.as_str(),
        )),
        NexusEventKind::WalkPendingAbort(event) => Some(format!(
            "Vertex {} is pending abort\nNext command\nnexus tap execution resolve-expired-walk --execution-id {} --walk-index {}",
            event.vertex, event.execution.bytes, event.walk_index,
        )),
        NexusEventKind::WalkAborted(event) => {
            Some(format!("Vertex {} was aborted after timeout", event.vertex))
        }
        NexusEventKind::WalkCancelled(event) => Some(format!(
            "Vertex {} was canceled because another walk was aborted",
            event.vertex,
        )),
        NexusEventKind::ExecutionPaymentInsufficientSettlement(event) => Some(format!(
            "Walk {} needs {} more MIST for settlement\nNext command\nnexus tap payments refill --execution-id {} --amount {}",
            event.walk_index,
            event.required_shortfall,
            event.execution.bytes,
            event.required_shortfall,
        )),
        NexusEventKind::ExecutionFinished(event) if event.was_aborted => {
            Some("Execution finished after an abort".to_owned())
        }
        NexusEventKind::ExecutionFinished(event) if event.has_any_walk_failed => {
            Some("Execution finished with failures".to_owned())
        }
        NexusEventKind::ExecutionFinished(event) if event.has_any_walk_succeeded => {
            Some("Execution finished successfully".to_owned())
        }
        NexusEventKind::ExecutionFinished(_) => {
            Some("Execution finished without a walk result".to_owned())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{event_result_json, render_event},
        nexus_sdk::{
            events::{NexusEvent, NexusEventKind},
            move_bindings::{
                interface::graph::{PostFailureAction, RuntimeVertex},
                move_std::{ascii::String as MoveString, option::Option as MoveOption},
                sui_framework::object::ID,
                workflow::{
                    execution_events::{ExecutionFinishedEvent, TerminalErrEvalRecordedEvent},
                    execution_failure::WorkflowFailureClass,
                },
            },
            sui,
        },
    };

    #[test]
    fn event_output_explains_failure_and_terminal_state() {
        let failure = NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
            dag: ID::new(sui::types::Address::ZERO),
            execution: ID::new(sui::types::Address::TWO),
            walk_index: 1,
            vertex: RuntimeVertex::plain("failable"),
            leader: sui::types::Address::THREE,
            failure_class: WorkflowFailureClass::TerminalToolFailure,
            outcome: MoveOption::from_option(None::<PostFailureAction>),
            reason: MoveString::from("tool failed"),
            err_eval_hash: vec![9, 8, 7],
            duplicate: false,
        });
        let finished = NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
            dag: ID::new(sui::types::Address::ZERO),
            execution: ID::new(sui::types::Address::TWO),
            has_any_walk_failed: true,
            has_any_walk_succeeded: false,
            was_aborted: false,
        });

        assert!(render_event(&failure).unwrap().contains("tool failed"));
        assert_eq!(
            render_event(&finished).as_deref(),
            Some("Execution finished with failures")
        );
    }

    #[test]
    fn event_result_json_normalizes_move_values_without_losing_payload() {
        let event = NexusEvent {
            id: (sui::types::Digest::from([4; 32]), 7),
            emitting_package: sui::types::Address::from_static("0xa"),
            generics: Vec::new(),
            data: NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
                dag: ID::new(sui::types::Address::ZERO),
                execution: ID::new(sui::types::Address::TWO),
                walk_index: 1,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalToolFailure,
                outcome: MoveOption::from_option(None::<PostFailureAction>),
                reason: MoveString::from("tool failed"),
                err_eval_hash: vec![9, 8, 7],
                duplicate: false,
            }),
            distribution: None,
        };

        let value = event_result_json(&event).expect("event output should serialize");

        assert_eq!(value["event_type"], "TerminalErrEvalRecordedEvent");
        assert_eq!(value["event_sequence"], 7);
        assert_eq!(
            value["event"]["execution"],
            sui::types::Address::TWO.to_string()
        );
        assert_eq!(value["event"]["vertex"], "failable");
        assert_eq!(value["event"]["reason"], "tool failed");
        assert_eq!(value["event"]["err_eval_hash_hex"], "090807");
        assert!(!value.to_string().contains("\"bytes\""));
        assert!(!value.to_string().contains("\"inner\""));
    }
}
