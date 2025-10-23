mod task_create;
mod task_inspect;
mod task_metadata;
mod task_state;

use {self::task_state::TaskStateRequest, crate::prelude::*};

#[derive(Subcommand)]
pub(crate) enum TaskCommand {
    #[command(about = "Create a new scheduler task")]
    Create(task_create::CreateArgs),
    #[command(about = "Inspect a scheduler task")]
    Inspect(task_inspect::InspectArgs),
    #[command(about = "Update scheduler task metadata")]
    Metadata(task_metadata::MetadataArgs),
    #[command(about = "Pause task scheduling")]
    Pause(task_state::StateArgs),
    #[command(about = "Resume task scheduling")]
    Resume(task_state::StateArgs),
    #[command(about = "Cancel task scheduling")]
    Cancel(task_state::StateArgs),
}

pub(crate) async fn handle(command: TaskCommand) -> AnyResult<(), NexusCliError> {
    match command {
        TaskCommand::Create(args) => task_create::create_task(args).await,
        TaskCommand::Inspect(args) => task_inspect::inspect_task(args).await,
        TaskCommand::Metadata(args) => task_metadata::update_metadata(args).await,
        TaskCommand::Pause(args) => task_state::set_task_state(args, TaskStateRequest::Pause).await,
        TaskCommand::Resume(args) => {
            task_state::set_task_state(args, TaskStateRequest::Resume).await
        }
        TaskCommand::Cancel(args) => {
            task_state::set_task_state(args, TaskStateRequest::Cancel).await
        }
    }
}
