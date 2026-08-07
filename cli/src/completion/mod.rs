use {
    crate::{prelude::*, Cli},
    std::io::{self, Write},
};

#[derive(Args)]
pub(crate) struct CompletionCommand {
    #[arg(value_enum)]
    pub(crate) shell: clap_complete::Shell,
}

pub(crate) fn handle(command: CompletionCommand, json: bool) -> AnyResult<(), NexusCliError> {
    handle_with_writer(command, json, &mut io::stdout())
}

fn handle_with_writer(
    command: CompletionCommand,
    json: bool,
    writer: &mut dyn Write,
) -> AnyResult<(), NexusCliError> {
    if json {
        return Err(NexusCliError::Any(anyhow!(
            "completion emits shell source and does not support --json"
        )));
    }

    let mut cli_command = Cli::command();
    let bin_name = env!("CARGO_CRATE_NAME").to_string();

    // Generate into an in-memory buffer to avoid panicking on BrokenPipe when writing directly to stdout.
    let mut buffer: Vec<u8> = Vec::new();
    clap_complete::generate(command.shell, &mut cli_command, bin_name, &mut buffer);

    // Best-effort write to stdout; ignore EPIPE/BrokenPipe to avoid crashing when the reader closes early.
    let _ = writer.write_all(&buffer);

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
                    let mut sink = std::io::sink();
                    handle_with_writer(cc, false, &mut sink).unwrap();
                }
                _ => unreachable!("This should have been a completion command!"),
            }
        }
    }

    #[test]
    fn completion_rejects_json_mode() {
        let error = handle_with_writer(
            CompletionCommand {
                shell: clap_complete::Shell::Bash,
            },
            true,
            &mut std::io::sink(),
        )
        .expect_err("completion emits shell source");

        assert!(error.to_string().contains("does not support --json"));
    }
}
