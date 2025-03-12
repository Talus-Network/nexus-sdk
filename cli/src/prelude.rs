pub(crate) use {
    crate::error::NexusCliError,
    anyhow::{anyhow, bail, Error as AnyError, Result as AnyResult},
    clap::{builder::ValueParser, Args, Parser, Subcommand, ValueEnum},
    colored::Colorize,
    nexus_sdk::{sui::traits::*, *},
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
    pub(crate) workflow_pkg_id: Option<sui::ObjectID>,
    pub(crate) primitives_pkg_id: Option<sui::ObjectID>,
    pub(crate) tool_registry_object_id: Option<sui::ObjectID>,
    pub(crate) default_sap_object_id: Option<sui::ObjectID>,
    pub(crate) network_id: Option<sui::ObjectID>,
}

/// Non-optional version of [NexusConf].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct NexusObjects {
    pub(crate) workflow_pkg_id: sui::ObjectID,
    pub(crate) primitives_pkg_id: sui::ObjectID,
    pub(crate) tool_registry_object_id: sui::ObjectID,
    pub(crate) default_sap_object_id: sui::ObjectID,
    pub(crate) network_id: sui::ObjectID,
}

/// Reusable Sui gas command args.
#[derive(Args, Clone, Debug)]
pub(crate) struct GasArgs {
    #[arg(
        long = "sui-gas-coin",
        short = 'g',
        help = "The gas coin object ID. First coin object is chosen if not present.",
        value_name = "OBJECT_ID"
    )]
    pub(crate) sui_gas_coin: Option<sui::ObjectID>,
    #[arg(
        long = "sui-gas-budget",
        short = 'b',
        help = "The gas budget for the transaciton.",
        value_name = "AMOUNT",
        default_value_t = sui::MIST_PER_SUI / 10
    )]
    pub(crate) sui_gas_budget: u64,
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

/// Parses JSON string into a serde_json::Value.
pub(crate) fn parse_json_string(json: &str) -> AnyResult<serde_json::Value> {
    serde_json::from_str(json).map_err(AnyError::from)
}

// == Used by serde ==

fn default_sui_wallet_path() -> PathBuf {
    home::home_dir()
        .expect("Home dir must exist.")
        .join(".sui/sui_config/client.yaml")
}
