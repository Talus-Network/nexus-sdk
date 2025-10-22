pub(crate) use {
    crate::{cli_conf::*, error::NexusCliError, utils::secrets::Secret},
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

/// Where to find config files.
pub(crate) const CLI_CONF_PATH: &str = "~/.nexus/conf.toml";
pub(crate) const CRYPTO_CONF_PATH: &str = "~/.nexus/crypto.toml";

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
