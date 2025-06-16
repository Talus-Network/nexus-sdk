pub(crate) use {
    crate::{error::NexusCliError, utils::secrets::Secret},
    anyhow::{anyhow, bail, Error as AnyError, Result as AnyResult},
    clap::{builder::ValueParser, Args, CommandFactory, Parser, Subcommand, ValueEnum},
    colored::Colorize,
    nexus_sdk::{
        crypto::{session::Session, x3dh::IdentityKey},
        sui::traits::*,
        types::NexusObjects,
        *,
    },
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::atomic::{AtomicBool, Ordering},
    },
};

/// Where to find config file.
pub(crate) const CLI_CONF_PATH: &str = "~/.nexus/conf.toml";

/// objects.toml locations for each network.
pub(crate) const DEVNET_OBJECTS_TOML: &str =
    "https://storage.googleapis.com/production-talus-sui-packages/objects.devnet.toml";
pub(crate) const _TESTNET_OBJECTS_TOML: &str = "";
pub(crate) const _MAINNET_OBJECTS_TOML: &str = "";

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
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CliConf {
    pub(crate) sui: SuiConf,
    pub(crate) nexus: Option<NexusObjects>,
    #[serde(default)]
    pub(crate) tools: HashMap<ToolFqn, ToolOwnerCaps>,
    pub(crate) crypto: Option<Secret<CryptoConf>>,
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

    pub(crate) async fn save(&self) -> AnyResult<()> {
        let conf_path = expand_tilde(CLI_CONF_PATH)?;

        self.save_to_path(&conf_path).await
    }

    pub(crate) async fn save_to_path(&self, path: &PathBuf) -> AnyResult<()> {
        let parent_folder = path.parent().expect("Parent folder must exist.");
        let conf = toml::to_string_pretty(&self)?;

        tokio::fs::create_dir_all(parent_folder).await?;
        tokio::fs::write(path, conf).await?;

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SuiConf {
    #[serde(default)]
    pub(crate) net: SuiNet,
    #[serde(default = "default_sui_wallet_path")]
    pub(crate) wallet_path: PathBuf,
    #[serde(default)]
    pub(crate) rpc_url: Option<reqwest::Url>,
}

impl Default for SuiConf {
    fn default() -> Self {
        Self {
            net: SuiNet::Localnet,
            wallet_path: default_sui_wallet_path(),
            rpc_url: None,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct CryptoConf {
    /// User's long-term identity key (None until first generated)
    pub(crate) identity_key: Option<IdentityKey>,
    /// Stored Double-Ratchet sessions keyed by their 32-byte session-id.
    #[serde(default)]
    pub(crate) sessions: HashMap<[u8; 32], Session>,
}

// Custom implementations because `IdentityKey` does not implement common traits.

impl std::fmt::Debug for CryptoConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CryptoConf")
            // Avoid printing sensitive material.
            .field("identity_key", &self.identity_key.is_some())
            .field("sessions", &self.sessions.len())
            .finish()
    }
}

impl PartialEq for CryptoConf {
    fn eq(&self, _other: &Self) -> bool {
        // All equal for now
        true
    }
}

impl Eq for CryptoConf {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ToolOwnerCaps {
    pub(crate) over_tool: sui::ObjectID,
    pub(crate) over_gas: sui::ObjectID,
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
        help = "The gas budget for the transaction.",
        value_name = "AMOUNT",
        default_value_t = sui::MIST_PER_SUI / 10
    )]
    pub(crate) sui_gas_budget: u64,
}

/// Whether to change the output format to JSON.
pub(crate) static JSON_MODE: AtomicBool = AtomicBool::new(false);

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
    let config_dir = sui::config_dir().expect("Unable to determine SUI config directory");
    config_dir.join(sui::CLIENT_CONFIG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let path = "~/test";
        let expanded = expand_tilde(path).unwrap();

        assert_eq!(expanded, home::home_dir().unwrap().join("test"));
    }

    #[test]
    fn test_parse_json_string() {
        let json = r#"{"key": "value"}"#;
        let parsed = parse_json_string(json).unwrap();

        assert_eq!(parsed, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_sui_net_display() {
        assert_eq!(SuiNet::Localnet.to_string(), "localnet");
        assert_eq!(SuiNet::Devnet.to_string(), "devnet");
        assert_eq!(SuiNet::Testnet.to_string(), "testnet");
        assert_eq!(SuiNet::Mainnet.to_string(), "mainnet");
    }
}
