pub(crate) use {
    crate::error::NexusCliError,
    anyhow::{anyhow, bail, Error as AnyError, Result as AnyResult},
    clap::{builder::ValueParser, Args, Parser, Subcommand, ValueEnum},
    colored::Colorize,
    serde::{Deserialize, Deserializer, Serialize},
    std::{
        path::{Path, PathBuf},
        str::FromStr,
    },
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

/// Non-optional version of [NexusConf].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct NexusObjects {
    pub(crate) workflow_pkg_id: sui::ObjectID,
    pub(crate) tool_registry_object_id: sui::ObjectID,
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
                SUI_CLOCK_OBJECT_ID as CLOCK_OBJECT_ID,
                SUI_CLOCK_OBJECT_SHARED_VERSION as CLOCK_OBJECT_SHARED_VERSION,
            },
            wallet_context::WalletContext,
            SuiClient as Client,
            SuiClientBuilder as ClientBuilder,
        },
    };
}

/// Struct that holds a structured tool FQN. This FQN consists of the tool
/// creator domain, the tool name and the tool version.
// TODO: <https://github.com/Talus-Network/nexus-sdk/issues/17>
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToolFqn {
    domain: String,
    name: String,
    version: u32,
}

/// Regex to match all the rules highlighted in the [FromStr] implementation.
static FQN_REGEX: lazy_regex::Lazy<regex::Regex> = lazy_regex::lazy_regex!(
    r"(?x)                                               # Enable verbose mode
    ^                                                    # Start of string
    (?P<domain>[a-z][a-z0-9_-]+(?:\.[a-z][a-z0-9_-]+)+)+ # Tool domain
    \.                                                   # '.' literal
    (?P<name>[a-z][a-z0-9_-]+)                           # Tool name
    @                                                    # '@' literal
    (?P<version>[0-9]+)                                  # Tool version
    $                                                    # End of string
    "
);

impl FromStr for ToolFqn {
    type Err = AnyError;

    /// This [FromStr] implementation expects a string with the following
    /// format:
    ///
    /// `xyz.taluslabs.example@1`
    ///
    /// Where:
    /// - `xyz.taluslabs` is the tool creator domain
    /// - `example` is the tool name
    /// - `1` is the tool version
    ///
    /// Constraints:
    /// 1. Splitting by `.` must yield at least 3 parts.
    /// 2. Each part must satisfy the `[a-z0-9_-]{2,}` regex.
    /// 3. Each part must not start with a digit, an underscore or a hyphen.
    /// 4. First N-1 parts when joined by `.` make the domain.
    /// 5. N-th part is the tool name and its version separated by `@`.
    /// 6. The version must be a positive u32 integer.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((domain, name, version)) = FQN_REGEX.captures(s).map(|captures| {
            (
                captures["domain"].to_string(),
                captures["name"].to_string(),
                captures["version"].to_string(),
            )
        }) else {
            bail!("Invalid tool FQN format");
        };

        // This only fails if u32 overflows as the format is already validated.
        // If this happens, kudos to the tool devs.
        let Ok(version) = version.parse::<u32>() else {
            bail!("Tool version too large");
        };

        Ok(Self {
            domain,
            name,
            version,
        })
    }
}

impl std::fmt::Display for ToolFqn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}@{}", self.domain, self.name, self.version)
    }
}

impl Serialize for ToolFqn {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ToolFqn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let fqn = value.parse::<ToolFqn>().map_err(serde::de::Error::custom)?;

        Ok(fqn)
    }
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
