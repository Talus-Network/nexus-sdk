use crate::{prelude::*, Cli};

#[derive(Args)]
pub(crate) struct CompletionCommand {
    #[arg(value_enum)]
    pub(crate) shell: clap_complete::Shell,
}

pub(crate) async fn handle(command: CompletionCommand) -> AnyResult<(), NexusCliError> {
    let mut cli_command = Cli::command();
    let bin_name = std::env::args()
        .next()
        .unwrap_or(env!("CARGO_CRATE_NAME").to_string());
    clap_complete::generate(
        command.shell,
        &mut cli_command,
        bin_name,
        &mut std::io::stdout(),
    );

    Ok(())
}
