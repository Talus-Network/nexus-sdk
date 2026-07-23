pub(crate) mod helpers;
mod occurrence;
mod recurrence;
pub(crate) mod task;

use crate::prelude::*;

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub(crate) enum SchedulerCommand {
    #[command(subcommand, about = "Manage scheduled tasks")]
    Task(task::TaskCommand),
    #[command(subcommand, about = "Manage manual occurrences for a Task")]
    Occurrence(occurrence::OccurrenceCommand),
    #[command(subcommand, about = "Manage recurrence for a Task")]
    Recurrence(recurrence::RecurrenceCommand),
}

/// Handle scheduler commands dispatched from the CLI root.
pub(crate) async fn handle(command: SchedulerCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus scheduler task ...` ==
        SchedulerCommand::Task(cmd) => task::handle(cmd).await,
        // == `$ nexus scheduler occurrence ...` ==
        SchedulerCommand::Occurrence(cmd) => occurrence::handle(cmd).await,
        // == `$ nexus scheduler recurrence ...` ==
        SchedulerCommand::Recurrence(cmd) => recurrence::handle(cmd).await,
    }
}
