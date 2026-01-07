mod task_create;
mod task_inspect;
mod task_metadata;
mod task_state;

use {
    self::task_state::TaskStateRequest,
    crate::prelude::*,
    nexus_sdk::{nexus::scheduler::GeneratorKind, types::DEFAULT_ENTRY_GROUP},
};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum TaskGeneratorArg {
    Queue,
    Periodic,
}

impl From<TaskGeneratorArg> for GeneratorKind {
    fn from(value: TaskGeneratorArg) -> Self {
        match value {
            TaskGeneratorArg::Queue => GeneratorKind::Queue,
            TaskGeneratorArg::Periodic => GeneratorKind::Periodic,
        }
    }
}

#[derive(Args, Debug, Clone)]
#[group(id = "schedule-start", multiple = false)]
pub(crate) struct ScheduleStartOptions {
    /// Absolute start time in milliseconds since epoch for the first occurrence.
    #[arg(long = "schedule-start-ms", value_name = "MILLIS")]
    schedule_start_ms: Option<u64>,
    /// Start offset in milliseconds from now for the first occurrence.
    #[arg(long = "schedule-start-offset-ms", value_name = "MILLIS")]
    schedule_start_offset_ms: Option<u64>,
}

#[derive(Args, Debug, Clone)]
#[group(id = "schedule-deadline", multiple = false)]
pub(crate) struct ScheduleDeadlineOptions {
    /// Deadline offset in milliseconds after the scheduled start for the first occurrence.
    #[arg(long = "schedule-deadline-offset-ms", value_name = "MILLIS")]
    schedule_deadline_offset_ms: Option<u64>,
}

#[derive(Subcommand)]
pub(crate) enum TaskCommand {
    #[command(about = "Create a new scheduler task")]
    Create {
        /// DAG object ID providing the execution definition.
        #[arg(long = "dag-id", short = 'd', value_name = "OBJECT_ID")]
        dag_id: sui::types::Address,
        /// Entry group to invoke when executing the DAG.
        #[arg(
            long = "entry-group",
            short = 'e',
            default_value = DEFAULT_ENTRY_GROUP,
            value_name = "NAME",
        )]
        entry_group: String,
        /// Initial input JSON for the DAG execution.
        #[arg(
            long = "input-json",
            short = 'i',
            value_parser = ValueParser::from(parse_json_string),
            value_name = "JSON",
        )]
        input_json: Option<serde_json::Value>,
        /// Which input json keys should be stored remotely.
        #[arg(
            long = "remote",
            short = 'r',
            help = "Which input json keys should be stored remotely. Provide a comma-separated list of {vertex}.{port} values. By default, all fields are stored inline.",
            value_delimiter = ',',
            value_name = "VERTEX.PORT"
        )]
        remote: Vec<String>,
        /// Metadata entries to attach to the task as key=value pairs.
        #[arg(long = "metadata", short = 'm', value_name = "KEY=VALUE")]
        metadata: Vec<String>,
        /// Priority fee per gas unit for DAG executions launched by the task.
        #[arg(
            long = "execution-priority-fee-per-gas-unit",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        execution_priority_fee_per_gas_unit: u64,
        #[command(flatten)]
        schedule_start: ScheduleStartOptions,
        #[command(flatten)]
        schedule_deadline: ScheduleDeadlineOptions,
        /// Priority fee per gas unit for the initial occurrence.
        #[arg(
            long = "schedule-priority-fee-per-gas-unit",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        schedule_priority_fee_per_gas_unit: u64,
        /// Generator responsible for producing future occurrences for this task.
        #[arg(
            long = "generator",
            value_enum,
            default_value_t = TaskGeneratorArg::Queue,
            value_name = "KIND"
        )]
        generator: TaskGeneratorArg,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Inspect a scheduler task")]
    Inspect {
        /// Task object ID to inspect.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
    },
    #[command(about = "Update scheduler task metadata")]
    Metadata {
        /// Task object ID to update.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        /// Metadata entries to write as key=value pairs.
        #[arg(
            long = "metadata",
            short = 'm',
            value_name = "KEY=VALUE",
            required = true
        )]
        metadata: Vec<String>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Pause task scheduling")]
    Pause {
        /// Task object ID to mutate.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Resume task scheduling")]
    Resume {
        /// Task object ID to mutate.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Cancel task scheduling")]
    Cancel {
        /// Task object ID to mutate.
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
            entry_group,
            input_json,
            remote,
            metadata,
            execution_priority_fee_per_gas_unit,
            schedule_start,
            schedule_deadline,
            schedule_priority_fee_per_gas_unit,
            generator,
            gas,
        } => {
            let ScheduleStartOptions {
                schedule_start_ms,
                schedule_start_offset_ms,
            } = schedule_start;
            let ScheduleDeadlineOptions {
                schedule_deadline_offset_ms,
            } = schedule_deadline;

            task_create::create_task(
                dag_id,
                entry_group,
                input_json,
                remote,
                metadata,
                execution_priority_fee_per_gas_unit,
                schedule_start_ms,
                schedule_start_offset_ms,
                schedule_deadline_offset_ms,
                schedule_priority_fee_per_gas_unit,
                generator.into(),
                gas,
            )
            .await
        }
        TaskCommand::Inspect { task_id } => task_inspect::inspect_task(task_id).await,
        TaskCommand::Metadata {
            task_id,
            metadata,
            gas,
        } => task_metadata::update_task_metadata(task_id, metadata, gas).await,
        TaskCommand::Pause { task_id, gas } => {
            task_state::set_task_state(task_id, gas, TaskStateRequest::Pause).await
        }
        TaskCommand::Resume { task_id, gas } => {
            task_state::set_task_state(task_id, gas, TaskStateRequest::Resume).await
        }
        TaskCommand::Cancel { task_id, gas } => {
            task_state::set_task_state(task_id, gas, TaskStateRequest::Cancel).await
        }
    }
}
