mod task_create;
mod task_inspect;
mod task_metadata;
mod task_state;

use {
    self::task_state::TaskStateRequest,
    crate::prelude::*,
    nexus_sdk::types::DEFAULT_ENTRY_GROUP,
};

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
    /// Absolute deadline time in milliseconds since epoch for the first occurrence.
    #[arg(long = "schedule-deadline-ms", value_name = "MILLIS")]
    schedule_deadline_ms: Option<u64>,
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
        dag_id: sui::ObjectID,
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
        /// Metadata entries to attach to the task as key=value pairs.
        #[arg(long = "metadata", short = 'm', value_name = "KEY=VALUE")]
        metadata: Vec<String>,
        /// Gas price paid as priority fee for the DAG execution.
        #[arg(
            long = "execution-gas-price",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        execution_gas_price: u64,
        #[command(flatten)]
        schedule_start: ScheduleStartOptions,
        #[command(flatten)]
        schedule_deadline: ScheduleDeadlineOptions,
        /// Gas price paid as priority fee associated with this occurrence.
        #[arg(
            long = "schedule-gas-price",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        schedule_gas_price: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Inspect a scheduler task")]
    Inspect {
        /// Task object ID to inspect.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::ObjectID,
    },
    #[command(about = "Update scheduler task metadata")]
    Metadata {
        /// Task object ID to update.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::ObjectID,
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
        task_id: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Resume task scheduling")]
    Resume {
        /// Task object ID to mutate.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Cancel task scheduling")]
    Cancel {
        /// Task object ID to mutate.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::ObjectID,
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
            metadata,
            execution_gas_price,
            schedule_start,
            schedule_deadline,
            schedule_gas_price,
            gas,
        } => {
            let ScheduleStartOptions {
                schedule_start_ms,
                schedule_start_offset_ms,
            } = schedule_start;
            let ScheduleDeadlineOptions {
                schedule_deadline_ms,
                schedule_deadline_offset_ms,
            } = schedule_deadline;

            task_create::create_task(
                dag_id,
                entry_group,
                input_json,
                metadata,
                execution_gas_price,
                schedule_start_ms,
                schedule_deadline_ms,
                schedule_start_offset_ms,
                schedule_deadline_offset_ms,
                schedule_gas_price,
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
