mod occurrence_add;

use crate::prelude::*;

#[derive(Args, Debug, Clone)]
#[group(id = "occurrence-start", multiple = false)]
pub(crate) struct OccurrenceStartOptions {
    /// Absolute start time in milliseconds since epoch.
    #[arg(long = "start-ms", value_name = "MILLIS")]
    start_ms: Option<u64>,
    /// Start offset in milliseconds from the current chain time.
    #[arg(long = "start-offset-ms", value_name = "MILLIS")]
    start_offset_ms: Option<u64>,
}

#[derive(Subcommand)]
pub(crate) enum OccurrenceCommand {
    #[command(about = "Add a manual occurrence to a Task")]
    Add {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        start: OccurrenceStartOptions,
        /// Deadline offset from the occurrence start.
        #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
        deadline_offset_ms: Option<u64>,
        /// Priority fee percentage applied to the occurrence.
        #[arg(long = "priority-fee-percentage", value_name = "PERCENTAGE")]
        priority_fee_percentage: Option<u64>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Expire an advertised occurrence after its deadline")]
    Expire {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long = "occurrence-id", value_name = "U64")]
        occurrence_id: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: OccurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        OccurrenceCommand::Add {
            task_id,
            start,
            deadline_offset_ms,
            priority_fee_percentage,
            gas,
        } => {
            occurrence_add::add_occurrence_to_task(
                task_id,
                start.start_ms,
                start.start_offset_ms,
                deadline_offset_ms,
                priority_fee_percentage,
                gas,
            )
            .await
        }
        OccurrenceCommand::Expire {
            task_id,
            occurrence_id,
            gas,
        } => occurrence_add::expire_occurrence(task_id, occurrence_id, gas).await,
    }
}
