use {
    crate::{display::*, prelude::*},
    nexus_sdk::{
        nexus::error::NexusError,
        scheduler::{ScheduleError, SchedulerError},
    },
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
    Rpc(anyhow::Error),
    #[error("{error}{separator}\n{0}", error = "Nexus Client Error".red().bold(), separator = separator())]
    Nexus(NexusError),
    #[error("{error}{separator}\n{0}", error = "Schedule Error".red().bold(), separator = separator())]
    Schedule(#[from] ScheduleError),
    #[error("{error}{separator}\n{0}", error = "Scheduler Error".red().bold(), separator = separator())]
    Scheduler(#[from] SchedulerError),
}

impl NexusCliError {
    pub(crate) fn protocol_upgrade_guidance(&self) -> Option<String> {
        let unsupported = match self {
            Self::Nexus(error) => unsupported_protocol(error),
            Self::Scheduler(error) => unsupported_protocol(error),
            _ => None,
        }?;

        Some(format!(
            "Protocol version {} requires a newer Nexus CLI. This CLI supports protocol versions \
             through {}. Upgrade the CLI and retry.",
            unsupported.0, unsupported.1,
        ))
    }
}

fn unsupported_protocol(error: &(dyn std::error::Error + 'static)) -> Option<(u64, u64)> {
    let mut source = Some(error);
    while let Some(error) = source {
        if let Some(NexusError::UnsupportedProtocolVersion {
            protocol_version,
            maximum,
        }) = error.downcast_ref::<NexusError>()
        {
            return Some((*protocol_version, *maximum));
        }
        source = error.source();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_protocol_explains_the_required_cli_action() {
        let error = NexusCliError::Nexus(NexusError::UnsupportedProtocolVersion {
            protocol_version: 3,
            maximum: 2,
        });

        assert_eq!(
            error.protocol_upgrade_guidance().as_deref(),
            Some(
                "Protocol version 3 requires a newer Nexus CLI. This CLI supports protocol \
                 versions through 2. Upgrade the CLI and retry."
            ),
        );
    }
}
