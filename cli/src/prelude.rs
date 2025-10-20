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
use {
    nexus_sdk::types::{StorageConf, StorageKind},
    std::sync::Arc,
    tokio::sync::Mutex,
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

/// Struct holding the config structure.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CliConf {
    pub(crate) sui: SuiConf,
    pub(crate) nexus: Option<NexusObjects>,
    #[serde(default)]
    pub(crate) tools: HashMap<ToolFqn, ToolOwnerCaps>,
    #[serde(default)]
    pub(crate) data_storage: DataStorageConf,
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

/// Remote data storage configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataStorageConf {
    /// The preferred Walrus aggregator URL.
    pub(crate) walrus_aggregator_url: Option<reqwest::Url>,
    /// The preferred Walrus publisher URL.
    pub(crate) walrus_publisher_url: Option<reqwest::Url>,
    /// How many epochs to save remote data for?
    pub(crate) walrus_save_for_epochs: Option<u64>,
    /// What is the preferred remote storage backend?
    pub(crate) preferred_remote_storage: Option<StorageKind>,
}

impl Into<StorageConf> for DataStorageConf {
    fn into(self) -> StorageConf {
        StorageConf {
            walrus_aggregator_url: self.walrus_aggregator_url.map(|url| url.to_string()),
            walrus_publisher_url: self.walrus_publisher_url.map(|url| url.to_string()),
            walrus_save_for_epochs: self.walrus_save_for_epochs,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct CryptoConf {
    /// User's long-term identity key (None until first generated)
    pub(crate) identity_key: Option<Secret<IdentityKey>>,
    /// Stored Double-Ratchet sessions keyed by their 32-byte session-id.
    #[serde(default)]
    pub(crate) sessions: Secret<HashMap<[u8; 32], Session>>,
}

// Custom implementations because `IdentityKey` does not implement common traits.
impl std::fmt::Debug for CryptoConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CryptoConf")
            // Avoid printing sensitive material.
            .field("Key exists:", &self.identity_key.is_some())
            .field("# of sessions:", &self.sessions.value.len())
            .finish()
    }
}

impl CryptoConf {
    pub(crate) async fn load() -> AnyResult<Self> {
        let conf_path = expand_tilde(CRYPTO_CONF_PATH)?;

        Self::load_from_path(&conf_path).await
    }

    pub(crate) async fn load_from_path(path: &PathBuf) -> AnyResult<Self> {
        let conf = tokio::fs::read_to_string(path).await?;

        Ok(toml::from_str(&conf)?)
    }

    #[allow(dead_code)]
    pub(crate) async fn save(&self) -> AnyResult<()> {
        let conf_path = expand_tilde(CRYPTO_CONF_PATH)?;

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

/// Gets the active session for encryption/decryption
pub async fn get_active_session() -> Result<Arc<Mutex<Session>>, NexusCliError> {
    let crypto_conf = CryptoConf::load().await.unwrap_or_default();

    let session = crypto_conf
        .sessions
        .value
        .into_values()
        .next()
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "Authentication required — run `nexus crypto auth` first"
            ))
        })?;

    Ok(Arc::new(Mutex::new(session)))
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

// == Used by serde ==

fn default_sui_wallet_path() -> PathBuf {
    let config_dir = sui::config_dir().expect("Unable to determine SUI config directory");
    config_dir.join(sui::CLIENT_CONFIG)
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::crypto::x3dh::PreKeyBundle, serial_test::serial, tempfile};

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

    fn setup_env() -> tempfile::TempDir {
        std::env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "test_passphrase");
        let secret_home = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", secret_home.path());
        std::env::set_var("XDG_DATA_HOME", secret_home.path());
        secret_home
    }

    fn cleanup_env() {
        std::env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_DATA_HOME");
    }

    fn create_test_session() -> Session {
        // Create sender and receiver identities
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();

        let spk_secret = {
            use rand::{rngs::OsRng, RngCore};
            let mut rng = OsRng;
            let mut bytes = [0u8; 32];
            rng.fill_bytes(&mut bytes);
            nexus_sdk::crypto::x3dh::IdentityKey::generate()
                .secret()
                .clone()
        };
        let spk_id = 1;
        let bundle = PreKeyBundle::new(&receiver_id, spk_id, &spk_secret, None, None);

        // Initiate a session (sender side)
        let (_, sender_session) = Session::initiate(&sender_id, &bundle, b"test session message")
            .expect("Failed to initiate session");

        sender_session
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_crypto_conf_save_and_load() {
        let secret_home = setup_env();

        let mut sessions = HashMap::new();
        let dummy_session = create_test_session();
        sessions.insert([1u8; 32], dummy_session);

        let conf = CryptoConf {
            identity_key: Some(Secret::new(IdentityKey::generate())),
            sessions: Secret::new(sessions),
        };

        conf.save().await.unwrap();
        let loaded = CryptoConf::load().await.unwrap();

        assert!(loaded.identity_key.is_some());
        assert_eq!(loaded.sessions.value.len(), 1);
        assert!(loaded.sessions.value.contains_key(&[1u8; 32]));

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_crypto_conf_default() {
        let secret_home = setup_env();

        let conf = CryptoConf::load().await.unwrap_or_default();
        assert!(conf.identity_key.is_none());
        assert_eq!(conf.sessions.value.len(), 0);

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_get_active_session_success() {
        let secret_home = setup_env();

        let mut sessions = HashMap::new();
        let dummy_session = create_test_session();
        sessions.insert([2u8; 32], dummy_session);

        let conf = CryptoConf {
            identity_key: Some(Secret::new(IdentityKey::generate())),
            sessions: Secret::new(sessions),
        };
        conf.save().await.unwrap();

        let session = get_active_session().await;
        assert!(session.is_ok());

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_get_active_session_error() {
        let secret_home = setup_env();

        let conf = CryptoConf::default();
        conf.save().await.unwrap();

        let result = get_active_session().await;
        assert!(result.is_err());
        assert!(
            matches!(result, Err(NexusCliError::Any(e)) if e.to_string().contains("Authentication required"))
        );

        cleanup_env();
        drop(secret_home);
    }
}
