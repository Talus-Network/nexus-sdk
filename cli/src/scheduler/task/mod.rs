mod task_create;
mod task_inspect;
mod task_state;

use {
    self::{task_create::CreateTaskOptions, task_state::TaskStateRequest},
    crate::prelude::*,
    nexus_sdk::types::DEFAULT_ENTRY_GROUP,
};

#[derive(Args, Debug, Clone)]
#[group(id = "schedule-start", multiple = false)]
pub(crate) struct ScheduleStartOptions {
    /// Absolute start time in milliseconds since epoch.
    #[arg(
        id = "schedule_start_ms",
        long = "schedule-start-ms",
        value_name = "MILLIS"
    )]
    pub(crate) start_ms: Option<u64>,
    /// Start offset in milliseconds from the current chain time.
    #[arg(
        id = "schedule_start_offset_ms",
        long = "schedule-start-offset-ms",
        value_name = "MILLIS"
    )]
    pub(crate) start_offset_ms: Option<u64>,
}

#[derive(Args, Debug, Clone)]
#[group(id = "recurrence-start", multiple = false)]
pub(crate) struct RecurrenceStartOptions {
    /// Absolute start time for the first recurring occurrence.
    #[arg(
        id = "recurrence_start_ms",
        long = "recurrence-start-ms",
        value_name = "MILLIS"
    )]
    pub(crate) start_ms: Option<u64>,
    /// Start offset from the current chain time for the first recurring occurrence.
    #[arg(
        id = "recurrence_start_offset_ms",
        long = "recurrence-start-offset-ms",
        value_name = "MILLIS"
    )]
    pub(crate) start_offset_ms: Option<u64>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub(crate) enum TaskCommand {
    #[command(about = "Create and compose a scheduled task")]
    Create {
        /// DAG for default execution or the selected DAG for an Agent skill.
        #[arg(long = "dag-id", short = 'd', value_name = "OBJECT_ID")]
        dag_id: Option<sui::types::Address>,
        /// Agent whose registered skill executes each occurrence.
        #[arg(long = "agent-id", value_name = "OBJECT_ID", requires = "skill_id")]
        agent_id: Option<sui::types::Address>,
        /// Registered skill executed by the Agent.
        #[arg(long = "skill-id", value_name = "U64", requires = "agent_id")]
        skill_id: Option<u64>,
        /// Draw Task funding and control authority from the Agent vault.
        #[arg(long = "agent-funded", requires = "agent_id")]
        agent_funded: bool,
        /// Address that receives unused sender funded reserve.
        #[arg(
            long = "refund-recipient",
            value_name = "ADDRESS",
            conflicts_with = "agent_funded"
        )]
        refund_recipient: Option<sui::types::Address>,
        /// Entry group invoked for each occurrence.
        #[arg(
            long = "entry-group",
            short = 'e',
            default_value = DEFAULT_ENTRY_GROUP,
            value_name = "NAME",
        )]
        entry_group: String,
        /// Input JSON for every DAG execution.
        #[arg(
            long = "input-json",
            short = 'i',
            value_parser = ValueParser::from(parse_json_string),
            value_name = "JSON",
        )]
        input_json: Option<serde_json::Value>,
        /// Input fields stored remotely as comma separated vertex.port values.
        #[arg(
            long = "remote",
            short = 'r',
            value_delimiter = ',',
            value_name = "VERTEX.PORT"
        )]
        remote: Vec<String>,
        /// Include one manual occurrence in the creation transaction.
        #[arg(long = "schedule")]
        schedule: bool,
        #[command(flatten)]
        schedule_start: ScheduleStartOptions,
        /// Deadline offset from the manual occurrence start.
        #[arg(long = "schedule-deadline-offset-ms", value_name = "MILLIS")]
        schedule_deadline_offset_ms: Option<u64>,
        /// Priority fee percentage for the manual occurrence.
        #[arg(long = "schedule-priority-fee-percentage", value_name = "PERCENTAGE")]
        schedule_priority_fee_percentage: Option<u64>,
        /// Recurrence interval in milliseconds.
        #[arg(long = "recurrence-interval-ms", value_name = "MILLIS")]
        recurrence_interval_ms: Option<u64>,
        #[command(flatten)]
        recurrence_start: RecurrenceStartOptions,
        /// Deadline offset from each recurring occurrence start.
        #[arg(long = "recurrence-deadline-offset-ms", value_name = "MILLIS")]
        recurrence_deadline_offset_ms: Option<u64>,
        /// Total number of recurring occurrences. Omit for no limit.
        #[arg(long = "recurrence-occurrences", value_name = "COUNT")]
        recurrence_occurrences: Option<u64>,
        /// Priority fee percentage for recurring occurrences.
        #[arg(long = "recurrence-priority-fee-percentage", value_name = "PERCENTAGE")]
        recurrence_priority_fee_percentage: Option<u64>,
        /// Pause the Task after a failed occurrence settles.
        #[arg(long = "pause-on-failure")]
        pause_on_failure: bool,
        /// MIST placed in the Task reserve.
        #[arg(long = "prepay-amount-mist", value_name = "MIST")]
        prepay_amount_mist: u64,
        /// Maximum MIST reserved for each occurrence.
        #[arg(long = "occurrence-budget-mist", value_name = "MIST")]
        occurrence_budget_mist: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Inspect a scheduled task")]
    Inspect {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
    },
    #[command(about = "Pause task scheduling")]
    Pause {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Resume task scheduling")]
    Resume {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Cancel future task scheduling")]
    Cancel {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Refill a task payment reserve")]
    Refill {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long = "amount-mist", value_name = "MIST")]
        amount_mist: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Close a task after all occurrences settle")]
    Close {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: TaskCommand) -> AnyResult<(), NexusCliError> {
    match command {
        TaskCommand::Create {
            dag_id,
            agent_id,
            skill_id,
            agent_funded,
            refund_recipient,
            entry_group,
            input_json,
            remote,
            schedule,
            schedule_start,
            schedule_deadline_offset_ms,
            schedule_priority_fee_percentage,
            recurrence_interval_ms,
            recurrence_start,
            recurrence_deadline_offset_ms,
            recurrence_occurrences,
            recurrence_priority_fee_percentage,
            pause_on_failure,
            prepay_amount_mist,
            occurrence_budget_mist,
            gas,
        } => {
            task_create::create_task(CreateTaskOptions {
                dag_id,
                agent_id,
                skill_id,
                agent_funded,
                refund_recipient,
                entry_group,
                input_json,
                remote,
                schedule,
                schedule_start,
                schedule_deadline_offset_ms,
                schedule_priority_fee_percentage,
                recurrence_interval_ms,
                recurrence_start,
                recurrence_deadline_offset_ms,
                recurrence_occurrences,
                recurrence_priority_fee_percentage,
                pause_on_failure,
                prepay_amount_mist,
                occurrence_budget_mist,
                gas,
            })
            .await
        }
        TaskCommand::Inspect { task_id } => task_inspect::inspect_task(task_id).await,
        TaskCommand::Pause { task_id, gas } => {
            task_state::set_task_state(task_id, gas, TaskStateRequest::Pause).await
        }
        TaskCommand::Resume { task_id, gas } => {
            task_state::set_task_state(task_id, gas, TaskStateRequest::Resume).await
        }
        TaskCommand::Cancel { task_id, gas } => {
            task_state::set_task_state(task_id, gas, TaskStateRequest::Cancel).await
        }
        TaskCommand::Refill {
            task_id,
            amount_mist,
            gas,
        } => task_state::refill_task(task_id, amount_mist, gas).await,
        TaskCommand::Close { task_id, gas } => task_state::close_task(task_id, gas).await,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn create_accepts_default_execution_with_composed_schedule() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "scheduler",
            "task",
            "create",
            "--dag-id",
            "0x42",
            "--schedule",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
        ])
        .expect("scheduler task create arguments should parse");

        let crate::Command::Scheduler(crate::scheduler::SchedulerCommand::Task(
            TaskCommand::Create {
                dag_id, schedule, ..
            },
        )) = cli.command
        else {
            panic!("expected scheduler task create command");
        };

        assert_eq!(dag_id, Some(sui::types::Address::from_static("0x42")));
        assert!(schedule);
    }

    #[test]
    fn skill_and_agent_identifiers_are_required_together() {
        let result = crate::Cli::try_parse_from([
            "nexus",
            "scheduler",
            "task",
            "create",
            "--agent-id",
            "0x42",
            "--prepay-amount-mist",
            "50000000",
            "--occurrence-budget-mist",
            "50000000",
        ]);
        let error = match result {
            Ok(_) => panic!("skill id must be required"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("--skill-id"));
    }
}
