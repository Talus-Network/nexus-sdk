mod conf_get;
mod conf_set;

use {
    crate::{display::json_output, prelude::*},
    conf_get::*,
    conf_set::*,
    nexus_sdk::{types::StorageKind, walrus::WALRUS_MAX_EPOCHS},
};

#[derive(Subcommand, Clone, Debug)]
pub(crate) enum ConfCommand {
    #[command(about = "Print the current Nexus CLI configuration")]
    Get {
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

    #[command(about = "Update the Nexus CLI configuration")]
    Set {
        #[arg(
            long = "sui.net",
            help = "Set the Sui network",
            value_enum,
            value_name = "NET"
        )]
        sui_net: Option<SuiNet>,
        #[arg(
            long = "sui.wallet-path",
            help = "Set the Sui wallet path",
            value_name = "PATH",
            value_parser = ValueParser::from(expand_tilde)
        )]
        sui_wallet_path: Option<PathBuf>,
        #[arg(
            long = "sui.rpc-url",
            help = "Set a custom RPC URL for the Sui node",
            value_name = "URL"
        )]
        sui_rpc_url: Option<reqwest::Url>,
        #[arg(
            long = "nexus.objects",
            help = "Path to a TOML file containing Nexus objects",
            value_name = "PATH",
            value_parser = ValueParser::from(expand_tilde)
        )]
        nexus_objects_path: Option<PathBuf>,
        #[arg(
            long = "data-storage.walrus-aggregator-url",
            help = "Set the Walrus aggregator URL for data storage",
            value_name = "URL"
        )]
        data_storage_walrus_aggregator_url: Option<reqwest::Url>,
        #[arg(
            long = "data-storage.walrus-publisher-url",
            help = "Set the Walrus publisher URL for data storage",
            value_name = "URL"
        )]
        data_storage_walrus_publisher_url: Option<reqwest::Url>,
        #[arg(
            long = "data-storage.walrus-save-for-epochs",
            help = "Set how many epochs to save data for in Walrus",
            value_name = "EPOCHS",
            value_parser = clap::value_parser!(u8).range(0..=WALRUS_MAX_EPOCHS as i64)
        )]
        data_storage_walrus_save_for_epochs: Option<u8>,
        #[arg(
            long = "data-storage.preferred-remote-storage",
            help = "Set the preferred remote storage backend",
            value_enum,
            value_name = "STORAGE_KIND"
        )]
        data_storage_preferred_remote_storage: Option<StorageKind>,
        #[arg(
            long = "data-storage.testnet",
            help = "Preset configuration for testnet (overrides other data storage settings)"
        )]
        data_storage_testnet: bool,

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
}

/// Handle the provided conf command. The [ConfCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: ConfCommand) -> AnyResult<(), NexusCliError> {
    match command {
        ConfCommand::Get { conf_path } => {
            let conf = get_nexus_conf(conf_path).await?;

            json_output(&conf)?;

            if !JSON_MODE.load(std::sync::atomic::Ordering::Relaxed) {
                let conf = toml::to_string_pretty(&conf).map_err(|e| {
                    NexusCliError::Any(anyhow!("Failed to serialize configuration to JSON: {}", e))
                })?;

                println!("{conf}");
            }

            Ok(())
        }
        ConfCommand::Set {
            sui_net,
            sui_wallet_path,
            sui_rpc_url,
            nexus_objects_path,
            data_storage_walrus_aggregator_url,
            data_storage_walrus_publisher_url,
            data_storage_walrus_save_for_epochs,
            data_storage_preferred_remote_storage,
            data_storage_testnet,
            conf_path,
        } => {
            set_nexus_conf(
                sui_net,
                sui_wallet_path,
                sui_rpc_url,
                nexus_objects_path,
                data_storage_walrus_aggregator_url,
                data_storage_walrus_publisher_url,
                data_storage_walrus_save_for_epochs,
                data_storage_preferred_remote_storage,
                data_storage_testnet,
                conf_path,
            )
            .await
        }
    }
}
