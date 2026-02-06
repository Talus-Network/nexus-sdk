use crate::prelude::*;

pub(crate) mod store;

mod helpers;
mod secrets_disable;
mod secrets_enable;
mod secrets_rotate;
mod secrets_status;
mod secrets_wipe;

use {
    secrets_disable::secrets_disable,
    secrets_enable::secrets_enable,
    secrets_rotate::secrets_rotate,
    secrets_status::secrets_status,
    secrets_wipe::secrets_wipe,
};

#[derive(clap::Subcommand, Clone, Debug)]
pub(crate) enum SecretsCommand {
    #[command(about = "Show local at-rest secrets status.")]
    Status {
        /// Hidden argument used for testing to set the path of the configuration file.
        #[arg(
            long = "conf-path",
            hide = true,
            default_value = CLI_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        conf_path: PathBuf,

        /// Hidden argument used for testing to set the path of the crypto configuration file.
        #[arg(
            long = "crypto-conf-path",
            hide = true,
            default_value = CRYPTO_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        crypto_conf_path: PathBuf,
    },

    #[command(about = "Enable at-rest encryption for locally stored secrets.")]
    Enable {
        /// Hidden argument used for testing to set the path of the configuration file.
        #[arg(
            long = "conf-path",
            hide = true,
            default_value = CLI_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        conf_path: PathBuf,

        /// Hidden argument used for testing to set the path of the crypto configuration file.
        #[arg(
            long = "crypto-conf-path",
            hide = true,
            default_value = CRYPTO_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        crypto_conf_path: PathBuf,
    },

    #[command(about = "Disable at-rest encryption and store secrets in plaintext.")]
    Disable {
        /// Skip the confirmation prompt.
        #[arg(long = "yes", short = 'y', help = "Skip the confirmation prompt")]
        skip_confirmation: bool,

        /// Hidden argument used for testing to set the path of the configuration file.
        #[arg(
            long = "conf-path",
            hide = true,
            default_value = CLI_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        conf_path: PathBuf,

        /// Hidden argument used for testing to set the path of the crypto configuration file.
        #[arg(
            long = "crypto-conf-path",
            hide = true,
            default_value = CRYPTO_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        crypto_conf_path: PathBuf,
    },

    #[command(about = "Rotate the master key and re-encrypt locally stored secrets.")]
    Rotate {
        /// Skip the confirmation prompt.
        #[arg(long = "yes", short = 'y', help = "Skip the confirmation prompt")]
        skip_confirmation: bool,

        /// Hidden argument used for testing to set the path of the configuration file.
        #[arg(
            long = "conf-path",
            hide = true,
            default_value = CLI_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        conf_path: PathBuf,

        /// Hidden argument used for testing to set the path of the crypto configuration file.
        #[arg(
            long = "crypto-conf-path",
            hide = true,
            default_value = CRYPTO_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        crypto_conf_path: PathBuf,
    },

    #[command(about = "Delete the master key and wipe all locally stored secret state.")]
    Wipe {
        /// Skip the confirmation prompt.
        #[arg(long = "yes", short = 'y', help = "Skip the confirmation prompt")]
        skip_confirmation: bool,

        /// Hidden argument used for testing to set the path of the configuration file.
        #[arg(
            long = "conf-path",
            hide = true,
            default_value = CLI_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        conf_path: PathBuf,

        /// Hidden argument used for testing to set the path of the crypto configuration file.
        #[arg(
            long = "crypto-conf-path",
            hide = true,
            default_value = CRYPTO_CONF_PATH,
            value_parser = ValueParser::from(expand_tilde)
        )]
        crypto_conf_path: PathBuf,
    },
}

/// Handle the provided secrets command.
pub async fn handle(cmd: SecretsCommand) -> AnyResult<(), NexusCliError> {
    match cmd {
        SecretsCommand::Status {
            conf_path,
            crypto_conf_path,
        } => secrets_status(conf_path, crypto_conf_path).await,
        SecretsCommand::Enable {
            conf_path,
            crypto_conf_path,
        } => secrets_enable(conf_path, crypto_conf_path).await,
        SecretsCommand::Disable {
            skip_confirmation,
            conf_path,
            crypto_conf_path,
        } => secrets_disable(skip_confirmation, conf_path, crypto_conf_path).await,
        SecretsCommand::Rotate {
            skip_confirmation,
            conf_path,
            crypto_conf_path,
        } => secrets_rotate(skip_confirmation, conf_path, crypto_conf_path).await,
        SecretsCommand::Wipe {
            skip_confirmation,
            conf_path,
            crypto_conf_path,
        } => secrets_wipe(skip_confirmation, conf_path, crypto_conf_path).await,
    }
}
