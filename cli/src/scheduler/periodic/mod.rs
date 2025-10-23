mod periodic_disable;
mod periodic_set;

use crate::prelude::*;

#[derive(Subcommand)]
pub(crate) enum PeriodicCommand {
    #[command(about = "Configure or update periodic scheduling")]
    Set {
        /// Task object ID.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::ObjectID,
        /// Period between occurrences in milliseconds.
        #[arg(long = "period-ms", value_name = "MILLIS")]
        period_ms: u64,
        /// Deadline offset from each start time in milliseconds.
        #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
        deadline_offset_ms: Option<u64>,
        /// Maximum number of generated occurrences (None for infinite).
        #[arg(long = "max-iterations", value_name = "COUNT")]
        max_iterations: Option<u64>,
        /// Gas price associated with occurrences.
        #[arg(long = "gas-price", value_name = "AMOUNT", default_value_t = 0u64)]
        gas_price: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Disable periodic scheduling")]
    Disable {
        /// Task object ID to update.
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: PeriodicCommand) -> AnyResult<(), NexusCliError> {
    match command {
        PeriodicCommand::Set {
            task_id,
            period_ms,
            deadline_offset_ms,
            max_iterations,
            gas_price,
            gas,
        } => {
            periodic_set::set_periodic_task(
                task_id,
                period_ms,
                deadline_offset_ms,
                max_iterations,
                gas_price,
                gas,
            )
            .await
        }
        PeriodicCommand::Disable { task_id, gas } => {
            periodic_disable::disable_periodic_task(task_id, gas).await
        }
    }
}
