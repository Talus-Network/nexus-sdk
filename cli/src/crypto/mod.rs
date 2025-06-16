use crate::prelude::*;

mod crypto_auth;
mod crypto_generate_id_key;
mod crypto_init_key;
mod crypto_key_status;
mod crypto_set_passphrase;

use {
    crypto_auth::crypto_auth,
    crypto_generate_id_key::crypto_generate_identity_key,
    crypto_init_key::crypto_init_key,
    crypto_key_status::crypto_key_status,
    crypto_set_passphrase::crypto_set_passphrase,
};

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
            default_value = CLI_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        conf_path: PathBuf,
    },
    #[command(about = "Generate and store a random 32-byte master key in the OS key-ring.")]
    InitKey {
        /// Overwrite an existing key.
        /// Are you really sure you want to do this?
        /// Will lose all existing sessions
        #[arg(long)]
        force: bool,
    },
    #[command(about = "Prompt for a pass-phrase and store it securely in the key-ring.")]
    SetPassphrase {
        /// Read the pass-phrase from STDIN instead of an interactive prompt.
        /// Useful for automation scripts.
        #[arg(long)]
        stdin: bool,
        /// Overwrite an existing key.
        /// Are you really sure you want to do this?
        /// Will lose all existing sessions
        #[arg(long)]
        force: bool,
    },
    #[command(about = "Show where the key was loaded from.")]
    KeyStatus,
}

/// Handle the provided crypto command.
pub async fn handle(cmd: CryptoCommand) -> AnyResult<(), NexusCliError> {
    match cmd {
        CryptoCommand::Auth { gas } => crypto_auth(gas).await,
        CryptoCommand::GenerateIdentityKey { conf_path } => {
            crypto_generate_identity_key(conf_path).await
        }
        CryptoCommand::InitKey { force } => crypto_init_key(force).await,
        CryptoCommand::SetPassphrase { stdin, force } => crypto_set_passphrase(stdin, force).await,
        CryptoCommand::KeyStatus => crypto_key_status(),
    }
}
