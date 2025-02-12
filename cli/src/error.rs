use {
    crate::{display::*, prelude::*},
    thiserror::Error,
};

/// Custom error definitions for the Nexus CLI. Takes care of displaying
/// a pretty summary in the console.
#[derive(Debug, Error)]
pub(crate) enum NexusCliError {
    #[error("{error}{separator}{0}", error = "Syntax Error".red().bold(), separator = separator())]
    SyntaxError(clap::error::Error),
    #[error("{error}{separator}{0}", error = "IO Error".red().bold(), separator = separator())]
    IoError(std::io::Error),
}
