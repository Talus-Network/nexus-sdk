mod cli_conf;
mod completion;
mod conf;
mod crypto;
mod dag;
mod display;
mod error;
mod gas;
mod network;
mod prelude;
mod scheduler;
mod sui;
mod tool;
mod utils;
mod workflow;

use crate::prelude::*;

#[derive(Parser)]
#[command(version, about = "Nexus CLI")]
struct Cli {
    /// Whether to output JSON.
    #[arg(
        global = true,
        long = "json",
        help = "Change the output format to JSON"
    )]
    json: bool,

    #[command(flatten)]
    verbose: clap_verbosity::Verbosity<clap_verbosity::ErrorLevel>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "Manage Nexus Tools")]
    Tool(tool::ToolCommand),
    #[command(subcommand, about = "Manage Nexus Configuration")]
    Conf(conf::ConfCommand),
    #[command(subcommand, about = "Validate, publish and execute Nexus DAGs")]
    Dag(dag::DagCommand),
    #[command(subcommand, about = "Manage scheduler tasks and occurrences")]
    Scheduler(scheduler::SchedulerCommand),
    #[command(subcommand, about = "Manage Nexus gas budgets and tickets")]
    Gas(gas::GasCommand),
    #[command(subcommand, about = "Manage Nexus networks and leader caps")]
    Network(network::NetworkCommand),
    #[command(subcommand, about = "Manage Nexus crypto")]
    Crypto(crypto::CryptoCommand),
    #[command(about = "Provide shell completions")]
    Completion(completion::CompletionCommand),
}

#[tokio::main]
async fn main() {
    // Customize parsing error handling.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // These 2 are "not real errors" that are used to stop the execution
            // to display the CLI help or version.
            match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    println!("{e}");

                    std::process::exit(0);
                }
                _ => (),
            }

            eprintln!(
                "{ballot} {error}",
                ballot = "✖".red().bold(),
                error = NexusCliError::Syntax(e)
            );

            std::process::exit(1);
        }
    };

    env_logger::builder()
        .filter(None, cli.verbose.log_level_filter())
        .build();

    JSON_MODE.store(cli.json, Ordering::Relaxed);

    // Send each sub-command to the respective handler.
    let result = match cli.command {
        Command::Tool(tool) => tool::handle(tool).await,
        Command::Conf(conf) => conf::handle(conf).await,
        Command::Dag(dag) => dag::handle(dag).await,
        Command::Network(network) => network::handle(network).await,
        Command::Gas(gas) => gas::handle(gas).await,
        Command::Scheduler(scheduler) => scheduler::handle(scheduler).await,
        Command::Crypto(crypto) => crypto::handle(crypto).await,
        Command::Completion(completion) => completion::handle(completion),
    };

    // Handle any errors that occurred during command execution.
    if let Err(e) = result {
        eprintln!("\n{ballot} {e}", ballot = "X".red().bold());

        std::process::exit(1);
    }
}
