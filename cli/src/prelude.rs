pub(crate) use {
    crate::error::NexusCliError,
    anyhow::{anyhow, Result as AnyResult},
    clap::{builder::ValueParser, Args, Parser, Subcommand, ValueEnum},
    colored::Colorize,
    serde::{Deserialize, Serialize},
    std::path::{Path, PathBuf},
};

// Where to find config file.
pub(crate) const CLI_CONF_PATH: &str = "~/.nexus/conf.toml";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub(crate) enum SuiNet {
    #[default]
    Localnet,
    Devnet,
    Testnet,
    Mainnet,
}

impl std::fmt::Display for SuiNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuiNet::Localnet => write!(f, "localnet"),
            SuiNet::Devnet => write!(f, "devnet"),
            SuiNet::Testnet => write!(f, "testnet"),
            SuiNet::Mainnet => write!(f, "mainnet"),
        }
    }
}

/// Struct holding the config structure.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CliConf {
    pub(crate) sui: SuiConf,
    pub(crate) nexus: NexusConf,
}

impl CliConf {
    pub(crate) async fn load() -> AnyResult<Self> {
        let conf_path = expand_tilde(CLI_CONF_PATH)?;

        Self::load_from_path(&conf_path).await
    }

    pub(crate) async fn load_from_path(path: &PathBuf) -> AnyResult<Self> {
        let conf = tokio::fs::read_to_string(path).await?;

        Ok(toml::from_str(&conf)?)
    }

    pub(crate) async fn save(&self, path: &PathBuf) -> AnyResult<()> {
        let parent_folder = path.parent().expect("Parent folder must exist.");
        let conf = toml::to_string_pretty(&self)?;

        tokio::fs::create_dir_all(parent_folder).await?;
        tokio::fs::write(path, conf).await?;

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SuiConf {
    #[serde(default)]
    pub(crate) net: SuiNet,
    #[serde(default = "default_sui_wallet_path")]
    pub(crate) wallet_path: PathBuf,
}

impl Default for SuiConf {
    fn default() -> Self {
        Self {
            net: SuiNet::Localnet,
            wallet_path: default_sui_wallet_path(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct NexusConf {
    pub(crate) workflow_id: Option<sui::ObjectID>,
    pub(crate) tool_registry_id: Option<sui::ObjectID>,
}

/// Normalizing Sui sdk imports.
pub(crate) mod sui {
    pub(crate) use {
        move_core_types::identifier::IdentStr as MoveIdentStr,
        sui_sdk::{
            rpc_types::{
                Coin,
                SuiExecutionStatus as ExecutionStatus,
                SuiObjectDataOptions as ObjectDataOptions,
                SuiObjectRef as ObjectRef,
                SuiTransactionBlockEffects as TransactionBlockEffects,
                SuiTransactionBlockResponseOptions as TransactionBlockResponseOptions,
            },
            types::{
                base_types::{ObjectID, SuiAddress as Address},
                gas_coin::MIST_PER_SUI,
                object::Owner,
                programmable_transaction_builder::ProgrammableTransactionBuilder,
                quorum_driver_types::ExecuteTransactionRequestType,
                transaction::{ObjectArg, TransactionData},
                MOVE_STDLIB_PACKAGE_ID,
            },
            wallet_context::WalletContext,
            SuiClient as Client,
            SuiClientBuilder as ClientBuilder,
        },
    };
}

// == Used by clap ==

/// Expands `~/` to the user's home directory in path arguments.
pub(crate) fn expand_tilde(path: &str) -> AnyResult<PathBuf> {
    if let Some(path) = path.strip_prefix("~/") {
        match home::home_dir() {
            Some(home) => return Ok(home.join(path)),
            None => return Err(anyhow!("Could not find home directory")),
        }
    }

    Ok(path.into())
}

// == Used by serde ==

fn default_sui_wallet_path() -> PathBuf {
    home::home_dir()
        .expect("Home dir must exist.")
        .join(".sui/sui_config/client.yaml")
}
