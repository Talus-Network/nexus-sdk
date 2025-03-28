mod conf;
mod dag;
mod display;
mod error;
mod network;
mod prelude;
mod sui;
mod tool;

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
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "Manage Nexus Tools")]
    Tool(tool::ToolCommand),
    #[command(about = "Manage Nexus Configuration")]
    Conf(conf::ConfCommand),
    #[command(subcommand, about = "Validate, publish and execute Nexus DAGs")]
    Dag(dag::DagCommand),
    #[command(subcommand, about = "Mange Nexus networks and leader caps")]
    Network(network::NetworkCommand),
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
                    println!("{}", e);

                    std::process::exit(0);
                }
                _ => (),
            }

            eprintln!(
                "{ballot} {error}",
                ballot = "✘".red().bold(),
                error = NexusCliError::Syntax(e)
            );

            std::process::exit(1);
        }
    };

    JSON_MODE.store(cli.json, Ordering::Relaxed);

    // Send each sub-command to the respective handler.
    let result = match cli.command {
        Command::Tool(tool) => tool::handle(tool).await,
        Command::Conf(conf) => conf::handle(conf).await,
        Command::Dag(dag) => dag::handle(dag).await,
        Command::Network(network) => network::handle(network).await,
    };

    // Handle any errors that occurred during command execution.
    if let Err(e) = result {
        eprintln!("\n{ballot} {e}", ballot = "✘".red().bold());

        std::process::exit(1);
    }
}
