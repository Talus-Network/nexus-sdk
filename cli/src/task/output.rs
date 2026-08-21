use {
    nexus_sdk::scheduler::{
        AbortReceipt,
        FailurePolicy,
        OccurrenceCost,
        OccurrenceSnapshot,
        OccurrenceSource,
        OccurrenceStatus,
        TaskController,
        TaskMutationReceipt,
        TaskPointer,
        TaskSnapshot,
        TaskStatus,
        WithdrawalReason,
    },
    std::fmt::Write as _,
};

const LABEL_WIDTH: usize = "Successful walks".len() + 1;

pub(super) fn render_task_list(pointers: &[TaskPointer], next_cursor: Option<&str>) -> String {
    let mut output = String::new();
    if pointers.is_empty() {
        output.push_str("No Tasks found.\n");
        output.push_str("\nNext command\nnexus task create --help\n");
    } else {
        for (index, pointer) in pointers.iter().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            write_field(&mut output, "Task", pointer.task_id());
            write_field(&mut output, "Pointer", pointer.task_pointer_id());
        }
    }
    if let Some(cursor) = next_cursor {
        output.push('\n');
        write_field(&mut output, "Next cursor", cursor);
    }
    output
}

pub(super) fn render_task(snapshot: &TaskSnapshot) -> String {
    let mut output = String::new();
    write_field(&mut output, "Task", snapshot.task_id());
    write_field(&mut output, "Status", task_status(snapshot.status()));
    match snapshot.controller() {
        TaskController::Address { address } => {
            write_field(&mut output, "Controller", format_args!("address {address}"));
        }
        TaskController::Agent { agent_id } => {
            write_field(&mut output, "Controller", format_args!("Agent {agent_id}"));
        }
    }
    write_field(
        &mut output,
        "Failure policy",
        failure_policy(snapshot.failure_policy()),
    );
    write_field(&mut output, "Version", snapshot.observed_version());
    output.push_str("\nOccurrences\n");
    write_field(&mut output, "Allocated", snapshot.allocated_occurrences());
    write_field(&mut output, "Pending", snapshot.pending_occurrences());
    write_field(&mut output, "Dispatched", snapshot.dispatched_occurrences());
    write_field(&mut output, "In flight", snapshot.in_flight_occurrences());
    write_field(
        &mut output,
        "Advertised",
        snapshot.advertised().map_or_else(
            || "none".to_owned(),
            |value| value.occurrence_id().to_string(),
        ),
    );
    if let Some(command) = task_next_command(snapshot) {
        output.push_str("\nNext command\n");
        writeln!(output, "{command}").expect("writing to a String cannot fail");
    }
    output
}

pub(super) fn render_task_receipt(
    receipt: &TaskMutationReceipt,
    occurrence_id: Option<u64>,
) -> String {
    let mut output = String::new();
    write_field(&mut output, "Task", receipt.task_id());
    write_field(&mut output, "Transaction", receipt.transaction().digest());
    write_field(
        &mut output,
        "Checkpoint",
        receipt.transaction().checkpoint(),
    );
    write_field(&mut output, "Scheduled", receipt.delta().scheduled().len());
    write_field(&mut output, "Withdrawn", receipt.delta().withdrawn().len());
    write_field(
        &mut output,
        "Advertised",
        receipt.delta().advertised().map_or_else(
            || "none".to_owned(),
            |reference| reference.occurrence_id().to_string(),
        ),
    );

    let command = occurrence_id
        .or_else(|| {
            receipt
                .delta()
                .scheduled()
                .first()
                .map(|occurrence| occurrence.reference().occurrence_id())
        })
        .or_else(|| {
            receipt
                .delta()
                .advertised()
                .map(|reference| reference.occurrence_id())
        })
        .map_or_else(
            || format!("nexus task inspect --task-id {}", receipt.task_id()),
            |occurrence_id| {
                format!(
                    "nexus task occurrence inspect --task-id {} --occurrence-id {occurrence_id}",
                    receipt.task_id(),
                )
            },
        );
    output.push_str("\nNext command\n");
    writeln!(output, "{command}").expect("writing to a String cannot fail");
    output
}

pub(super) fn render_abort_receipt(receipt: &AbortReceipt) -> String {
    let mut output = String::new();
    let reference = receipt.occurrence();
    write_field(&mut output, "Task", reference.task_id());
    write_field(&mut output, "Occurrence", reference.occurrence_id());
    write_field(&mut output, "Execution", receipt.execution_id());
    write_field(&mut output, "Transaction", receipt.transaction().digest());
    write_field(
        &mut output,
        "Checkpoint",
        receipt.transaction().checkpoint(),
    );
    output.push_str("\nNext command\n");
    writeln!(
        output,
        "nexus task occurrence inspect --task-id {} --occurrence-id {}",
        reference.task_id(),
        reference.occurrence_id(),
    )
    .expect("writing to a String cannot fail");
    output
}

pub(super) fn render_occurrence_list(
    task_id: nexus_sdk::sui::types::Address,
    occurrences: &[OccurrenceSnapshot],
    next_cursor: Option<&str>,
) -> String {
    let mut output = String::new();
    if occurrences.is_empty() {
        output.push_str("No occurrences found.\n");
    } else {
        writeln!(
            output,
            "{:<14}{:<14}{:<12}Execution",
            "Occurrence", "Status", "Result"
        )
        .expect("writing to a String cannot fail");
        for occurrence in occurrences {
            let execution = occurrence.execution().map_or_else(
                || "none".to_owned(),
                |value| value.execution_id().to_string(),
            );
            writeln!(
                output,
                "{:<14}{:<14}{:<12}{}",
                occurrence.reference().occurrence_id(),
                occurrence_status(occurrence.status()),
                occurrence_result(occurrence.status()).unwrap_or("none"),
                execution,
            )
            .expect("writing to a String cannot fail");
        }
    }
    if let Some(cursor) = next_cursor {
        output.push('\n');
        write_field(&mut output, "Next cursor", cursor);
    }
    let command = occurrences
        .iter()
        .find_map(next_command)
        .unwrap_or_else(|| format!("nexus task inspect --task-id {task_id}"));
    output.push_str("\nNext command\n");
    writeln!(output, "{command}").expect("writing to a String cannot fail");
    output
}

pub(super) fn render_occurrence(snapshot: &OccurrenceSnapshot) -> String {
    let mut output = String::new();
    let reference = snapshot.reference();
    write_field(&mut output, "Task", reference.task_id());
    write_field(&mut output, "Occurrence", reference.occurrence_id());
    write_field(&mut output, "Status", occurrence_status(snapshot.status()));
    if let Some(result) = occurrence_result(snapshot.status()) {
        write_field(&mut output, "Result", result);
    }
    write_field(&mut output, "Source", occurrence_source(snapshot.source()));
    write_field(
        &mut output,
        "Requested start",
        format_args!("{} ms", snapshot.requested_start_time_ms()),
    );
    write_field(
        &mut output,
        "Effective start",
        optional_time(snapshot.effective_start_time_ms()),
    );
    write_field(
        &mut output,
        "Deadline",
        optional_time(snapshot.deadline_ms()),
    );
    write_field(
        &mut output,
        "Priority fee",
        format_args!("{}%", snapshot.priority_fee_percentage()),
    );
    write_field(
        &mut output,
        "Dispatched",
        optional_time(snapshot.dispatched_at_ms()),
    );
    write_field(
        &mut output,
        "Settled",
        optional_time(snapshot.settled_at_ms()),
    );
    match snapshot.status() {
        OccurrenceStatus::Missed { missed_at_ms } => {
            write_field(&mut output, "Missed", format_args!("{missed_at_ms} ms"));
        }
        OccurrenceStatus::Withdrawn { reason } => {
            write_field(&mut output, "Reason", withdrawal_reason(reason));
        }
        _ => {}
    }
    write_field(
        &mut output,
        "Task version",
        snapshot.observed_task_version(),
    );

    if let Some(execution) = snapshot.execution() {
        output.push_str("\nExecution\n");
        write_field(&mut output, "ID", execution.execution_id());
        write_field(
            &mut output,
            "Created",
            optional_time(execution.created_at_ms()),
        );
        write_field(
            &mut output,
            "Active walks",
            optional_count(execution.active_walks()),
        );
        write_field(
            &mut output,
            "Successful walks",
            optional_count(execution.successful_walks()),
        );
        write_field(
            &mut output,
            "Failed walks",
            optional_count(execution.failed_walks()),
        );
        write_field(
            &mut output,
            "Aborted walks",
            optional_count(execution.aborted_walks()),
        );
        write_field(
            &mut output,
            "Pending abort",
            optional_count(execution.pending_abort_walks()),
        );
        write_field(
            &mut output,
            "Pending settle",
            optional_count(execution.pending_settlement_walks()),
        );
    }

    if let Some(command) = next_command(snapshot) {
        output.push_str("\nNext command\n");
        writeln!(output, "{command}").expect("writing to a String cannot fail");
    }
    output
}

pub(super) fn render_occurrence_cost(
    task_id: nexus_sdk::sui::types::Address,
    occurrence_id: u64,
    cost: &OccurrenceCost,
) -> String {
    let mut output = String::new();
    write_field(&mut output, "Payment", cost.payment_id());
    write_field(
        &mut output,
        "Maximum budget",
        format_args!("{} MIST", cost.max_budget_mist()),
    );
    write_field(
        &mut output,
        "Locked budget",
        format_args!("{} MIST", cost.locked_budget_mist()),
    );
    write_field(
        &mut output,
        "Consumed",
        format_args!("{} MIST", cost.consumed_mist()),
    );
    write_field(&mut output, "Outstanding", cost.outstanding_locks());
    write_field(&mut output, "Accomplished", yes_no(cost.accomplished()));
    write_field(&mut output, "Refunded", yes_no(cost.refunded()));
    output.push_str("\nNext command\n");
    writeln!(
        output,
        "nexus task occurrence inspect --task-id {task_id} --occurrence-id {occurrence_id}"
    )
    .expect("writing to a String cannot fail");
    output
}

fn write_field(output: &mut String, label: &str, value: impl std::fmt::Display) {
    writeln!(output, "{label:<LABEL_WIDTH$}{value}").expect("writing to a String cannot fail");
}

const fn task_status(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Active => "active",
        TaskStatus::Paused => "paused",
        TaskStatus::Canceled => "canceled",
        TaskStatus::Rejected { .. } => "rejected",
        TaskStatus::Finalized => "finalized",
    }
}

const fn failure_policy(policy: FailurePolicy) -> &'static str {
    match policy {
        FailurePolicy::Continue => "continue",
        FailurePolicy::Pause => "pause",
    }
}

fn occurrence_source(source: OccurrenceSource) -> String {
    match source {
        OccurrenceSource::Standalone => "standalone".to_owned(),
        OccurrenceSource::Recurring { iteration } => format!("recurring, iteration {iteration}"),
    }
}

const fn occurrence_status(status: OccurrenceStatus) -> &'static str {
    match status {
        OccurrenceStatus::Pending => "pending",
        OccurrenceStatus::Advertised => "advertised",
        OccurrenceStatus::Executing => "executing",
        OccurrenceStatus::Finished => "finished",
        OccurrenceStatus::Settled { .. } => "settled",
        OccurrenceStatus::Missed { .. } => "missed",
        OccurrenceStatus::Withdrawn { .. } => "withdrawn",
    }
}

const fn occurrence_result(status: OccurrenceStatus) -> Option<&'static str> {
    match status {
        OccurrenceStatus::Settled { succeeded: true } => Some("succeeded"),
        OccurrenceStatus::Settled { succeeded: false } => Some("failed"),
        _ => None,
    }
}

const fn withdrawal_reason(reason: WithdrawalReason) -> &'static str {
    match reason {
        WithdrawalReason::RecurrenceReplaced => "recurrence replaced",
        WithdrawalReason::RecurrenceCleared => "recurrence cleared",
        WithdrawalReason::TaskCanceled => "Task canceled",
        WithdrawalReason::TaskRejected => "Task rejected",
    }
}

fn optional_time(value: Option<u64>) -> String {
    value.map_or_else(|| "none".to_owned(), |value| format!("{value} ms"))
}

fn optional_count(value: Option<u64>) -> String {
    value.map_or_else(|| "not observed".to_owned(), |value| value.to_string())
}

const fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn next_command(snapshot: &OccurrenceSnapshot) -> Option<String> {
    let reference = snapshot.reference();
    match snapshot.status() {
        OccurrenceStatus::Finished => Some(format!(
            "nexus task occurrence settle --task-id {} --occurrence-id {}",
            reference.task_id(),
            reference.occurrence_id(),
        )),
        OccurrenceStatus::Pending | OccurrenceStatus::Advertised => Some(format!(
            "nexus task occurrence inspect --task-id {} --occurrence-id {} --follow",
            reference.task_id(),
            reference.occurrence_id(),
        )),
        OccurrenceStatus::Executing | OccurrenceStatus::Settled { succeeded: false } => {
            snapshot.execution().map(|_| {
                format!(
                    "nexus execution inspect --task-id {} --occurrence-id {}",
                    reference.task_id(),
                    reference.occurrence_id(),
                )
            })
        }
        _ => None,
    }
}

fn task_next_command(snapshot: &TaskSnapshot) -> Option<String> {
    let task_id = snapshot.task_id();
    match snapshot.status() {
        TaskStatus::Canceled if snapshot.in_flight_occurrences() == 0 => {
            Some(format!("nexus task close --task-id {task_id}"))
        }
        TaskStatus::Rejected { .. } | TaskStatus::Finalized => None,
        _ => snapshot.advertised().map_or_else(
            || match (snapshot.allocated_occurrences() > 0, snapshot.status()) {
                (true, _) => Some(format!("nexus task occurrence list --task-id {task_id}")),
                (false, TaskStatus::Active) => Some(format!(
                    "nexus task occurrence add --task-id {task_id} --now"
                )),
                (false, TaskStatus::Paused) => {
                    Some(format!("nexus task resume --task-id {task_id}"))
                }
                (
                    false,
                    TaskStatus::Canceled | TaskStatus::Rejected { .. } | TaskStatus::Finalized,
                ) => None,
            },
            |reference| {
                Some(format!(
                    "nexus task occurrence inspect --task-id {task_id} --occurrence-id {}",
                    reference.occurrence_id(),
                ))
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{
            render_abort_receipt,
            render_occurrence,
            render_occurrence_cost,
            render_occurrence_list,
            render_task,
            render_task_list,
            render_task_receipt,
        },
        nexus_sdk::{
            scheduler::{
                AbortReceipt,
                OccurrenceCost,
                OccurrenceSnapshot,
                TaskMutationReceipt,
                TaskPointer,
                TaskSnapshot,
            },
            sui,
        },
        serde_json::{json, Value},
    };

    fn occurrence_with_status(status: Value) -> OccurrenceSnapshot {
        serde_json::from_value(json!({
            "reference": { "task_id": "0x42", "occurrence_id": 7 },
            "source": { "kind": "standalone" },
            "requested_start_time_ms": 10,
            "effective_start_time_ms": 10,
            "deadline_ms": null,
            "priority_fee_percentage": 20,
            "dispatched_at_ms": 11,
            "settled_at_ms": 12,
            "status": status,
            "execution": {
                "execution_id": "0x99",
                "created_at_ms": 11,
                "active_walks": 0,
                "pending_abort_walks": 0,
                "pending_settlement_walks": 0,
                "successful_walks": 0,
                "failed_walks": 0,
                "aborted_walks": 1
            },
            "observed_task_version": 9
        }))
        .expect("the occurrence fixture should be valid")
    }

    #[test]
    fn failed_settled_occurrence_points_to_execution() {
        let snapshot = occurrence_with_status(json!({
            "status": "settled",
            "succeeded": false
        }));

        let output = render_occurrence(&snapshot);

        assert!(output.contains("Status           settled"));
        assert!(output.contains("Result           failed"));
        assert!(output.contains("Aborted walks    1"));
        assert!(output.contains("Successful walks 0"));
        assert!(output.contains(&format!(
            "nexus execution inspect --task-id {} --occurrence-id 7",
            snapshot.reference().task_id()
        )));
    }

    #[test]
    fn finished_occurrence_points_to_settlement() {
        let snapshot = occurrence_with_status(json!({ "status": "finished" }));

        let output = render_occurrence(&snapshot);

        assert!(output.contains(&format!(
            "nexus task occurrence settle --task-id {} --occurrence-id 7",
            snapshot.reference().task_id()
        )));
        assert!(!output.contains("nexus execution inspect"));
    }

    #[test]
    fn successful_settlement_has_no_recovery_command() {
        let snapshot = occurrence_with_status(json!({
            "status": "settled",
            "succeeded": true
        }));

        let output = render_occurrence(&snapshot);

        assert!(output.contains("Result           succeeded"));
        assert!(!output.contains("\nNext command\n"));
    }

    #[test]
    fn executing_occurrence_points_to_execution() {
        let snapshot = occurrence_with_status(json!({ "status": "executing" }));

        let output = render_occurrence(&snapshot);

        assert!(output.contains("Status           executing"));
        assert!(output.contains("nexus execution inspect --task-id"));
    }

    #[test]
    fn waiting_states_point_to_follow() {
        for status in [
            json!({ "status": "pending" }),
            json!({ "status": "advertised" }),
        ] {
            let snapshot = occurrence_with_status(status);
            let output = render_occurrence(&snapshot);
            assert!(output.contains(&format!(
                "nexus task occurrence inspect --task-id {} --occurrence-id 7 --follow",
                snapshot.reference().task_id()
            )));
        }
    }

    #[test]
    fn terminal_states_without_recovery_do_not_print_a_command() {
        for status in [
            json!({ "status": "missed", "missed_at_ms": 13 }),
            json!({ "status": "withdrawn", "reason": "task_canceled" }),
        ] {
            let snapshot = occurrence_with_status(status);
            let output = render_occurrence(&snapshot);
            assert!(!output.contains("\nNext command\n"));
        }
    }

    #[test]
    fn every_task_read_model_has_human_output() {
        let pointer: TaskPointer = serde_json::from_value(json!({
            "task_pointer_id": "0x20",
            "task_id": "0x21"
        }))
        .unwrap();
        let task: TaskSnapshot = serde_json::from_value(json!({
            "task_id": "0x21",
            "controller": { "kind": "address", "address": "0x22" },
            "status": "active",
            "failure_policy": "continue",
            "advertised": { "task_id": "0x21", "occurrence_id": 7 },
            "allocated_occurrences": 8,
            "pending_occurrences": 2,
            "dispatched_occurrences": 6,
            "in_flight_occurrences": 1,
            "observed_version": 9
        }))
        .unwrap();
        let occurrence = occurrence_with_status(json!({ "status": "executing" }));
        let cost: OccurrenceCost = serde_json::from_value(json!({
            "payment_id": "0x23",
            "max_budget_mist": 100,
            "locked_budget_mist": 30,
            "consumed_mist": 40,
            "outstanding_locks": 2,
            "accomplished": true,
            "refunded": false
        }))
        .unwrap();

        assert!(render_task_list(&[pointer], Some("0102")).contains("Next cursor"));
        let task_output = render_task(&task);
        assert!(task_output.contains("In flight        1"));
        assert!(task_output.contains(&format!(
            "nexus task occurrence inspect --task-id {} --occurrence-id 7",
            task.task_id(),
        )));
        assert!(render_occurrence_list(task.task_id(), &[occurrence], None).contains("executing"));
        assert!(
            render_occurrence_cost(task.task_id(), 7, &cost).contains("Consumed         40 MIST")
        );
    }

    #[test]
    fn empty_task_and_occurrence_views_keep_navigation_complete() {
        let task: TaskSnapshot = serde_json::from_value(json!({
            "task_id": "0x21",
            "controller": { "kind": "address", "address": "0x22" },
            "status": "active",
            "failure_policy": "continue",
            "advertised": null,
            "allocated_occurrences": 0,
            "pending_occurrences": 0,
            "dispatched_occurrences": 0,
            "in_flight_occurrences": 0,
            "observed_version": 9
        }))
        .unwrap();

        assert!(render_task(&task).contains(&format!(
            "nexus task occurrence add --task-id {} --now",
            task.task_id()
        )));
        assert!(render_occurrence_list(task.task_id(), &[], None)
            .contains(&format!("nexus task inspect --task-id {}", task.task_id())));
        assert!(render_task_list(&[], None).contains("nexus task create --help"));
    }

    #[test]
    fn mutation_receipts_preserve_confirmation_and_return_to_state() {
        let digest = sui::types::Digest::new([7; 32]);
        let receipt: TaskMutationReceipt = serde_json::from_value(json!({
            "transaction": { "digest": digest, "checkpoint": 12 },
            "task_id": "0x21",
            "delta": {
                "scheduled": [],
                "withdrawn": [],
                "advertised": null
            }
        }))
        .unwrap();
        let output = render_task_receipt(&receipt, Some(7));

        assert!(output.contains(&format!("Transaction      {digest}")));
        assert!(output.contains("Checkpoint       12"));
        assert!(output.contains(&format!(
            "nexus task occurrence inspect --task-id {} --occurrence-id 7",
            receipt.task_id(),
        )));

        let abort: AbortReceipt = serde_json::from_value(json!({
            "transaction": { "digest": digest, "checkpoint": 13 },
            "occurrence": { "task_id": "0x21", "occurrence_id": 7 },
            "execution_id": "0x99"
        }))
        .unwrap();
        let abort_output = render_abort_receipt(&abort);
        assert!(abort_output.contains(&format!("Execution        {}", abort.execution_id())));
        assert!(abort_output.contains(&format!(
            "nexus task occurrence inspect --task-id {} --occurrence-id 7",
            abort.occurrence().task_id(),
        )));

        let scheduled: TaskMutationReceipt = serde_json::from_value(json!({
            "transaction": { "digest": digest, "checkpoint": 14 },
            "task_id": "0x21",
            "delta": {
                "scheduled": [{
                    "reference": { "task_id": "0x21", "occurrence_id": 8 },
                    "start_time_ms": 10,
                    "deadline_ms": null,
                    "priority_fee_percentage": 20,
                    "source": { "kind": "standalone" }
                }],
                "withdrawn": [],
                "advertised": null
            }
        }))
        .unwrap();
        let scheduled_output = render_task_receipt(&scheduled, None);
        assert!(
            scheduled_output.contains(&format!(
                "nexus task occurrence inspect --task-id {} --occurrence-id 8",
                scheduled.task_id()
            )),
            "{scheduled_output}"
        );
    }
}
