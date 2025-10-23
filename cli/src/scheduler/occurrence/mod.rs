mod occurrence_add;

use crate::prelude::*;

#[derive(Subcommand)]
pub(crate) enum OccurrenceCommand {
    #[command(about = "Add a sporadic occurrence to a task")]
    Add {
        /// Task object ID receiving the occurrence.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::ObjectID,
        /// Absolute start time in milliseconds since epoch.
        #[arg(long = "start-ms", value_name = "MILLIS")]
        start_ms: Option<u64>,
        /// Absolute deadline time in milliseconds since epoch.
        #[arg(long = "deadline-ms", value_name = "MILLIS")]
        deadline_ms: Option<u64>,
        /// Start offset in milliseconds from now.
        #[arg(long = "start-offset-ms", value_name = "MILLIS")]
        start_offset_ms: Option<u64>,
        /// Deadline offset in milliseconds after the scheduled start.
        #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
        deadline_offset_ms: Option<u64>,
        /// Gas price paid as priority fee associated with the occurrence.
        #[arg(long = "gas-price", value_name = "AMOUNT", default_value_t = 0u64)]
        gas_price: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: OccurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        OccurrenceCommand::Add {
            task_id,
            start_ms,
            deadline_ms,
            start_offset_ms,
            deadline_offset_ms,
            gas_price,
            gas,
        } => {
            occurrence_add::add_occurrence_to_task(
                task_id,
                start_ms,
                deadline_ms,
                start_offset_ms,
                deadline_offset_ms,
                gas_price,
                gas,
            )
            .await
        }
    }
}
