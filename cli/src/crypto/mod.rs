use crate::prelude::*;

mod crypto_auth;
use crypto_auth::crypto_auth;

#[derive(clap::Subcommand, Clone, Debug)]
pub(crate) enum CryptoCommand {
    #[command(about = "Establish a secure session with the network.")]
    Auth {
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Handle the provided crypto command.
pub(crate) async fn handle(cmd: CryptoCommand) -> AnyResult<(), NexusCliError> {
    match cmd {
        CryptoCommand::Auth { gas } => crypto_auth(gas).await,
    }
}
