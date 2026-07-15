mod occurrence_add;

use crate::prelude::*;

#[derive(Args, Debug, Clone)]
#[group(id = "occurrence-start", multiple = false)]
pub(crate) struct OccurrenceStartOptions {
    /// Absolute start time in milliseconds since epoch.
    #[arg(long = "start-ms", value_name = "MILLIS")]
    start_ms: Option<u64>,
    /// Start offset in milliseconds from now.
    #[arg(long = "start-offset-ms", value_name = "MILLIS")]
    start_offset_ms: Option<u64>,
}

#[derive(Args, Debug, Clone)]
#[group(id = "occurrence-deadline", multiple = false)]
pub(crate) struct OccurrenceDeadlineOptions {
    /// Deadline offset in milliseconds after the scheduled start.
    #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
    deadline_offset_ms: Option<u64>,
}

#[derive(Subcommand)]
pub(crate) enum OccurrenceCommand {
    #[command(about = "Add a sporadic occurrence to a task")]
    Add {
        /// Task object ID receiving the occurrence.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        start: OccurrenceStartOptions,
        #[command(flatten)]
        deadline: OccurrenceDeadlineOptions,
        /// Optional priority fee percentage applied to the occurrence.
        #[arg(long = "priority-fee-percentage", value_name = "PERCENTAGE")]
        priority_fee_percentage: Option<u64>,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: OccurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        OccurrenceCommand::Add {
            task_id,
            start,
            deadline,
            priority_fee_percentage,
            gas,
        } => {
            let OccurrenceStartOptions {
                start_ms,
                start_offset_ms,
            } = start;
            let OccurrenceDeadlineOptions { deadline_offset_ms } = deadline;

            occurrence_add::add_occurrence_to_task(
                task_id,
                start_ms,
                start_offset_ms,
                deadline_offset_ms,
                priority_fee_percentage,
                gas,
            )
            .await
        }
    }
}
