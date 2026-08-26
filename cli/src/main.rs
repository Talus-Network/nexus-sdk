mod cli_conf;
mod completion;
mod conf;
mod dag;
mod display;
mod error;
mod execution;
mod gas;
mod network;
mod nexus_data_json;
mod prelude;
mod sui;
mod tap;
mod task;
mod tool;
mod workflow;

use {crate::prelude::*, std::io::IsTerminal as _};

#[derive(Parser)]
#[command(version, about = "Nexus CLI")]
struct Cli {
    /// Whether to output JSON.
    #[arg(
        global = true,
        long = "json",
        help = "Emit machine readable JSON",
        help_heading = "Output"
    )]
    json: bool,

    #[command(flatten)]
    verbosity: CliVerbosity,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "Manage Nexus Tools")]
    Tool(tool::ToolCommand),
    #[command(subcommand, about = "Manage Nexus Configuration")]
    Conf(conf::ConfCommand),
    #[command(subcommand, about = "Validate and publish Nexus DAGs")]
    Dag(dag::DagCommand),
    #[command(subcommand, about = "Create and operate scheduled Tasks")]
    Task(Box<task::TaskCommand>),
    #[command(
        subcommand,
        about = "Inspect and authorize workflow executions",
        long_about = execution::EXECUTION_HELP
    )]
    Execution(execution::ExecutionCommand),
    #[command(subcommand, about = "Manage Sui transaction gas")]
    Gas(gas::GasCommand),
    #[command(subcommand, about = "Manage Nexus network fees")]
    Network(network::NetworkCommand),
    #[command(subcommand, about = "Prepare and operate standard TAP skills")]
    Tap(tap::TapCommand),
    #[command(about = "Provide shell completions")]
    Completion(completion::CompletionCommand),
}

#[derive(Args)]
struct CliVerbosity {
    #[arg(
        global = true,
        short = 'v',
        long,
        action = clap::ArgAction::Count,
        conflicts_with = "quiet",
        help = "Increase diagnostic detail",
        help_heading = "Output"
    )]
    verbose: u8,
    #[arg(
        global = true,
        short = 'q',
        long,
        action = clap::ArgAction::Count,
        conflicts_with = "verbose",
        help = "Show errors only",
        help_heading = "Output"
    )]
    quiet: u8,
}

impl CliVerbosity {
    fn log_level_filter(&self) -> log::LevelFilter {
        if self.quiet > 0 {
            return log::LevelFilter::Off;
        }
        match self.verbose {
            0 => log::LevelFilter::Error,
            1 => log::LevelFilter::Warn,
            2 => log::LevelFilter::Info,
            3 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }
    }

    fn is_quiet(&self) -> bool {
        self.quiet > 0
    }

    fn is_verbose(&self) -> bool {
        self.verbose > 0
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    env_logger::builder()
        .filter(None, cli.verbosity.log_level_filter())
        .init();

    JSON_MODE.store(cli.json, Ordering::Relaxed);
    let error_report_mode = if cli.json {
        error::ErrorReportMode::Json
    } else if cli.verbosity.is_quiet() {
        error::ErrorReportMode::Quiet
    } else {
        let verbose = cli.verbosity.is_verbose();
        let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        error::ErrorReportMode::Human { verbose, color }
    };

    // Send each sub-command to the respective handler.
    let result = match cli.command {
        Command::Tool(tool) => tool::handle(tool).await,
        Command::Conf(conf) => conf::handle(conf).await,
        Command::Dag(dag) => dag::handle(dag).await,
        Command::Gas(gas) => gas::handle(gas).await,
        Command::Network(network) => network::handle(network).await,
        Command::Tap(tap) => tap::handle(tap).await,
        Command::Task(task) => task::handle(*task).await,
        Command::Execution(execution) => execution::handle(execution).await,
        Command::Completion(completion) => completion::handle(completion, cli.json),
    };

    // Handle any errors that occurred during command execution.
    if let Err(e) = result {
        let report = error::render_error(&e, error_report_mode);
        if matches!(error_report_mode, error::ErrorReportMode::Human { .. }) {
            eprintln!();
        }
        eprint!("{report}");
        if !report.ends_with('\n') {
            eprintln!();
        }

        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_help_describes_diagnostic_output_without_domain_ambiguity() {
        let mut command = Cli::command();
        let mut help = Vec::new();
        command.write_long_help(&mut help).unwrap();
        let help = String::from_utf8(help).unwrap();

        assert!(help.contains("Increase diagnostic detail"));
        assert!(help.contains("Show errors only"));
        assert!(!help.contains("per occurrence"));
    }

    #[test]
    fn verbose_and_quiet_are_mutually_exclusive() {
        let error = match Cli::try_parse_from([
            "nexus",
            "-v",
            "-q",
            "task",
            "list",
            "--scheduler-package",
            "0xa5",
        ]) {
            Ok(_) => panic!("verbose and quiet should conflict"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn completion_rejects_json_mode() {
        let cli = Cli::try_parse_from(["nexus", "--json", "completion", "bash"]).unwrap();
        assert!(cli.json);
    }
}
