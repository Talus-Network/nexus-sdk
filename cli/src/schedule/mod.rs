pub(crate) mod helpers;
mod occurrence;
mod recurrence;
pub(crate) mod task;

use crate::prelude::*;

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub(crate) enum ScheduleCommand {
    #[command(subcommand, about = "Manage scheduled tasks")]
    Task(task::TaskCommand),
    #[command(subcommand, about = "Manage standalone occurrences for a Task")]
    Occurrence(occurrence::OccurrenceCommand),
    #[command(subcommand, about = "Manage recurrence for a Task")]
    Recurrence(recurrence::RecurrenceCommand),
}

/// Handles schedule commands dispatched from the CLI root.
pub(crate) async fn handle(command: ScheduleCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus schedule task ...` ==
        ScheduleCommand::Task(cmd) => task::handle(cmd).await,
        // == `$ nexus schedule occurrence ...` ==
        ScheduleCommand::Occurrence(cmd) => occurrence::handle(cmd).await,
        // == `$ nexus schedule recurrence ...` ==
        ScheduleCommand::Recurrence(cmd) => recurrence::handle(cmd).await,
    }
}
