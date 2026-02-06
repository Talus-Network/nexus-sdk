use crate::prelude::*;

mod crypto_auth;
mod crypto_generate_id_key;

use {crypto_auth::crypto_auth, crypto_generate_id_key::crypto_generate_identity_key};

#[derive(clap::Subcommand, Clone, Debug)]
pub(crate) enum CryptoCommand {
    #[command(about = "Establish a secure session with the network.")]
    Auth {
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(
        about = "Generate and store a fresh identity key. WARNING: This will invalidate all existing sessions!"
    )]
    GenerateIdentityKey {
        /// Hidden argument used for testing to set the path of the configuration
        /// file.
        #[arg(
            long = "conf-path",
            hide = true,
            default_value = CRYPTO_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        conf_path: PathBuf,
    },
}

/// Handle the provided crypto command.
pub async fn handle(cmd: CryptoCommand) -> AnyResult<(), NexusCliError> {
    match cmd {
        CryptoCommand::Auth { gas } => crypto_auth(gas).await,
        CryptoCommand::GenerateIdentityKey { conf_path } => {
            crypto_generate_identity_key(conf_path).await
        }
    }
}
