use {
    crate::{display::*, prelude::*},
    nexus_sdk::nexus::error::NexusError,
    thiserror::Error,
};

/// Custom error definitions for the Nexus CLI. Takes care of displaying
/// a pretty summary in the console.
#[derive(Debug, Error)]
pub(crate) enum NexusCliError {
    #[error("{error}{separator}\n{0}", error = "Syntax Error".red().bold(), separator = separator())]
    Syntax(clap::error::Error),
    #[error("{error}{separator}\n{0}", error = "IO Error".red().bold(), separator = separator())]
    Io(std::io::Error),
    #[error("{error}{separator}\n{0}", error = "Error".red().bold(), separator = separator())]
    Any(anyhow::Error),
    #[error("{error}{separator}\n{0}", error = "HTTP Error".red().bold(), separator = separator())]
    Http(reqwest::Error),
    #[error("{error}{separator}\n{0}", error = "Sui Error".red().bold(), separator = separator())]
    Sui(sui::Error),
    #[error("{error}{separator}\n{0}", error = "Nexus Client Error".red().bold(), separator = separator())]
    Nexus(NexusError),
}
