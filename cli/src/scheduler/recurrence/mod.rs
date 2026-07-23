mod recurrence_clear;
mod recurrence_set;

use crate::prelude::*;

#[derive(Subcommand)]
pub(crate) enum RecurrenceCommand {
    #[command(about = "Set or replace Task recurrence")]
    Set {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        /// Absolute start time for the first recurring occurrence.
        #[arg(long = "first-start-ms", value_name = "MILLIS")]
        first_start_ms: Option<u64>,
        /// Start offset from the current chain time.
        #[arg(
            long = "first-start-offset-ms",
            value_name = "MILLIS",
            conflicts_with = "first_start_ms"
        )]
        first_start_offset_ms: Option<u64>,
        /// Interval between recurring occurrences.
        #[arg(long = "interval-ms", value_name = "MILLIS")]
        interval_ms: u64,
        /// Deadline offset from each occurrence start.
        #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
        deadline_offset_ms: Option<u64>,
        /// Total number of recurring occurrences. Omit for no limit.
        #[arg(long = "occurrences", value_name = "COUNT")]
        occurrences: Option<u64>,
        /// Priority fee percentage for recurring occurrences.
        #[arg(long = "priority-fee-percentage", value_name = "PERCENTAGE")]
        priority_fee_percentage: Option<u64>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Clear future recurring work")]
    Clear {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: RecurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        RecurrenceCommand::Set {
            task_id,
            first_start_ms,
            first_start_offset_ms,
            interval_ms,
            deadline_offset_ms,
            occurrences,
            priority_fee_percentage,
            gas,
        } => {
            recurrence_set::set_recurrence(recurrence_set::SetRecurrenceOptions {
                task_id,
                first_start_ms,
                first_start_offset_ms,
                interval_ms,
                deadline_offset_ms,
                occurrences,
                priority_fee_percentage,
                gas,
            })
            .await
        }
        RecurrenceCommand::Clear { task_id, gas } => {
            recurrence_clear::clear_recurrence(task_id, gas).await
        }
    }
}
