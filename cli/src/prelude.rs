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
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    },
    tokio::sync::Mutex,
};

/// Where to find config files.
pub(crate) const CLI_CONF_PATH: &str = "~/.nexus/conf.toml";
pub(crate) const CRYPTO_CONF_PATH: &str = "~/.nexus/crypto.toml";

/// Various Nexus RPC URLs.
pub(crate) const DEVNET_NEXUS_RPC_URL: &str = "https://rpc.ssfn.devnet.production.taluslabs.dev";

/// objects.toml locations for each network.
pub(crate) const DEVNET_OBJECTS_TOML: &str =
    "https://storage.googleapis.com/production-talus-sui-objects/v0.4.0/objects.devnet.toml";
pub(crate) const _TESTNET_OBJECTS_TOML: &str = "";
pub(crate) const _MAINNET_OBJECTS_TOML: &str = "";

/// What is the default gas budget to use? (0.1 SUI)
pub(crate) const DEFAULT_GAS_BUDGET: u64 = sui::MIST_PER_SUI / 10;

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
    pub(crate) over_tool: sui::types::Address,
    pub(crate) over_gas: Option<sui::types::Address>,
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
    pub(crate) sui_gas_coin: Option<sui::types::Address>,
    #[arg(
        long = "sui-gas-budget",
        short = 'b',
        help = "The gas budget for the transaction.",
        value_name = "AMOUNT",
        default_value_t = DEFAULT_GAS_BUDGET
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
    use {super::*, serial_test::serial};

    #[test]
    #[serial]
    fn test_expand_tilde() {
        let original_home = std::env::var_os("HOME");
        let temp_home = tempfile::tempdir().expect("temp home directory");
        let temp_home_path = temp_home.path().to_path_buf();

        std::env::set_var("HOME", &temp_home_path);

        let expanded = expand_tilde("~/test").unwrap();
        assert_eq!(expanded, temp_home_path.join("test"));

        match original_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
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
