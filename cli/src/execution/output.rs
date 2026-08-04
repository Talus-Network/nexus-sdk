use nexus_sdk::{events::NexusEventKind, scheduler::OccurrenceRef};

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
        super::render_event,
        nexus_sdk::{
            events::NexusEventKind,
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
}
