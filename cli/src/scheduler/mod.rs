mod helpers;
mod occurrence;
mod periodic;
mod task;

use crate::prelude::*;

#[derive(Subcommand)]
pub(crate) enum SchedulerCommand {
    #[command(subcommand, about = "Manage scheduler tasks")]
    Task(task::TaskCommand),
    #[command(subcommand, about = "Manage sporadic occurrences for a task")]
    Occurrence(occurrence::OccurrenceCommand),
    #[command(subcommand, about = "Manage periodic scheduling for a task")]
    Periodic(periodic::PeriodicCommand),
}

/// Handle scheduler commands dispatched from the CLI root.
pub(crate) async fn handle(command: SchedulerCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus scheduler task ...` ==
        SchedulerCommand::Task(cmd) => task::handle(cmd).await,
        // == `$ nexus scheduler occurrence ...` ==
        SchedulerCommand::Occurrence(cmd) => occurrence::handle(cmd).await,
        // == `$ nexus scheduler periodic ...` ==
        SchedulerCommand::Periodic(cmd) => periodic::handle(cmd).await,
    }
}
