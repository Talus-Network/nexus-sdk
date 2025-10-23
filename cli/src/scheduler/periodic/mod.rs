mod periodic_disable;
mod periodic_set;

use crate::prelude::*;

#[derive(Subcommand)]
pub(crate) enum PeriodicCommand {
    #[command(about = "Configure or update periodic scheduling")]
    Set(periodic_set::SetArgs),
    #[command(about = "Disable periodic scheduling")]
    Disable(periodic_disable::DisableArgs),
}

pub(crate) async fn handle(command: PeriodicCommand) -> AnyResult<(), NexusCliError> {
    match command {
        PeriodicCommand::Set(args) => periodic_set::set_periodic(args).await,
        PeriodicCommand::Disable(args) => periodic_disable::disable_periodic(args).await,
    }
}
