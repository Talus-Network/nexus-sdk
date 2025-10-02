use {
    crate::{prelude::*, Cli},
    std::io::Write,
};

#[derive(Args)]
pub(crate) struct CompletionCommand {
    #[arg(value_enum)]
    pub(crate) shell: clap_complete::Shell,
}

pub(crate) fn handle(command: CompletionCommand) -> AnyResult<(), NexusCliError> {
    let mut cli_command = Cli::command();
    let bin_name = env!("CARGO_CRATE_NAME").to_string();

    // Generate into an in-memory buffer to avoid panicking on BrokenPipe when writing directly to stdout.
    let mut buffer: Vec<u8> = Vec::new();
    clap_complete::generate(command.shell, &mut cli_command, bin_name, &mut buffer);

    // Best-effort write to stdout; ignore EPIPE/BrokenPipe to avoid crashing when the reader closes early.
    let _ = std::io::stdout().write_all(&buffer);

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, crate::Command};

    #[test]
    fn test_all_shell_completions() {
        // Simulate the completion command line for all the supported shells.
        // ... and run the command line.

        for shell in clap_complete::Shell::value_variants() {
            let shell_string = shell.to_string();
            let args = vec!["nexus", "completion", shell_string.as_str()];
            let cli = Cli::parse_from(&args);
            match cli.command {
                Command::Completion(cc) => {
                    handle(cc).unwrap();
                }
                _ => unreachable!("This should have been a completion command!"),
            }
        }
    }
}
