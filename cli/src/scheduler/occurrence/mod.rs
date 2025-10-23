mod occurrence_add;

use crate::prelude::*;

#[derive(Subcommand)]
pub(crate) enum OccurrenceCommand {
    #[command(about = "Add a sporadic occurrence to a task")]
    Add(occurrence_add::AddArgs),
}

pub(crate) async fn handle(command: OccurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        OccurrenceCommand::Add(args) => occurrence_add::add_occurrence(args).await,
    }
}
