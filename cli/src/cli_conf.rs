use {
    crate::prelude::*,
    nexus_sdk::types::{SecretValue, StorageConf, StorageKind},
};

/// Struct holding the config structure.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CliConf {
    pub(crate) sui: SuiConf,
    pub(crate) nexus: Option<NexusObjects>,
    #[serde(default)]
    pub(crate) tools: HashMap<ToolFqn, ToolOwnerCaps>,
    #[serde(default)]
    pub(crate) secrets: SecretsConf,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct SuiConf {
    /// Sui private key base64 encoded bytes.
    #[serde(default)]
    pub(crate) pk: Option<SecretValue>,
    #[serde(default)]
    pub(crate) rpc_url: Option<reqwest::Url>,
    #[serde(default)]
    pub(crate) gql_url: Option<reqwest::Url>,
}

/// Local secrets configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SecretsConf {
    #[serde(default)]
    pub(crate) mode: SecretsMode,
}

impl Default for SecretsConf {
    fn default() -> Self {
        Self {
            mode: SecretsMode::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SecretsMode {
    #[default]
    Auto,
    Require,
    Off,
}

impl std::fmt::Display for SecretsMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretsMode::Auto => write!(f, "auto"),
            SecretsMode::Require => write!(f, "require"),
            SecretsMode::Off => write!(f, "off"),
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
    pub(crate) walrus_save_for_epochs: Option<u8>,
    /// What is the preferred remote storage backend?
    pub(crate) preferred_remote_storage: Option<StorageKind>,
}

impl From<DataStorageConf> for StorageConf {
    fn from(val: DataStorageConf) -> StorageConf {
        StorageConf {
            walrus_aggregator_url: val.walrus_aggregator_url.map(|url| url.to_string()),
            walrus_publisher_url: val.walrus_publisher_url.map(|url| url.to_string()),
            walrus_save_for_epochs: val.walrus_save_for_epochs,
        }
    }
}
