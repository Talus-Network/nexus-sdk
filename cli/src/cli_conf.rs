use {
    crate::prelude::*,
    anyhow::Context as _,
    nexus_sdk::{sui, types::SecretValue},
};

/// Struct holding the config structure.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CliConf {
    pub(crate) sui: SuiConf,
    pub(crate) nexus: Option<NexusObjects>,
    #[serde(default)]
    pub(crate) tools: HashMap<ToolFqn, ToolOwnerCaps>,
    #[serde(default)]
    pub(crate) agents: HashMap<String, sui::types::Address>,
    #[serde(default)]
    pub(crate) secrets: SecretsConf,
    #[serde(default)]
    pub(crate) data_storage: DataStorageConf,
}

impl CliConf {
    pub(crate) async fn load() -> AnyResult<Self> {
        let conf_path = cli_conf_path()?;

        Self::load_from_path(&conf_path).await
    }

    pub(crate) async fn load_from_path(path: &PathBuf) -> AnyResult<Self> {
        let conf = tokio::fs::read_to_string(path).await?;

        Ok(toml::from_str(&conf)?)
    }

    /// Loads the configuration, starting from defaults only when the file does
    /// not exist yet.
    ///
    /// Commands that modify and then save the configuration must use this
    /// rather than `load().unwrap_or_default()`: an unreadable or unparsable
    /// file is an error here, so a typo in `conf.toml` can never be replaced by
    /// a default configuration on the next save.
    pub(crate) async fn load_or_default() -> AnyResult<Self> {
        let conf_path = cli_conf_path()?;

        Self::load_from_path_or_default(&conf_path).await
    }

    /// Path-explicit variant of [`CliConf::load_or_default`].
    pub(crate) async fn load_from_path_or_default(path: &PathBuf) -> AnyResult<Self> {
        match tokio::fs::read_to_string(path).await {
            Ok(conf) => toml::from_str(&conf).with_context(|| {
                format!(
                    "Failed to parse Nexus CLI configuration at {}.\n\
                     Fix the file by hand, or move it away to start from a default \
                     configuration. Run with -v to see the parse error.",
                    path.display()
                )
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(AnyError::new(error).context(format!(
                "Failed to read Nexus CLI configuration at {}",
                path.display()
            ))),
        }
    }

    pub(crate) async fn save(&self) -> AnyResult<()> {
        let conf_path = cli_conf_path()?;

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
    /// Sui secret key in keytool export or legacy Base64 format.
    #[serde(default, alias = "pk")]
    pub(crate) sk: Option<SecretValue>,
    #[serde(default)]
    pub(crate) rpc_url: Option<reqwest::Url>,
    /// Pinned Cloud KMS key version used to sign instead of a local key.
    #[serde(default)]
    pub(crate) kms_key_version: Option<String>,
}

#[cfg(test)]
mod secret_key_tests {
    use super::*;

    #[test]
    fn legacy_key_is_loaded_and_saved_as_sk() {
        for field in ["pk", "sk"] {
            let conf: SuiConf = toml::from_str(&format!("{field} = 'test-key'\n")).unwrap();
            assert_eq!(conf.sk, Some(SecretValue::from("test-key")));
            let saved = toml::to_string(&conf).unwrap();
            assert!(saved.contains("sk = "));
            assert!(!saved.contains("pk = "));
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StorageKind {
    Inline,
    Walrus,
}

/// Remote data storage configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataStorageConf {
    /// Network associated with a custom aggregator.
    pub(crate) walrus_network: Option<nexus_sdk::walrus::WalrusNetwork>,
    /// The preferred Walrus aggregator URL.
    pub(crate) walrus_aggregator_url: Option<reqwest::Url>,
    /// How many epochs to save remote data for?
    pub(crate) walrus_save_for_epochs: Option<u8>,
    /// What is the preferred remote storage backend?
    pub(crate) preferred_remote_storage: Option<StorageKind>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn conf_round_trips_kms_key_version() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("conf.toml");
        let version = "projects/p/locations/l/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1";

        for value in [Some(version.to_string()), None] {
            let mut conf = CliConf::default();
            conf.sui.kms_key_version = value.clone();
            conf.save_to_path(&path).await.unwrap();
            let loaded = CliConf::load_from_path(&path).await.unwrap();
            assert_eq!(loaded.sui.kms_key_version, value);
        }

        // Files written before the field existed still load.
        tokio::fs::write(&path, "[sui]\nrpc_url = \"https://rpc.example.com\"\n")
            .await
            .unwrap();
        let loaded = CliConf::load_from_path(&path).await.unwrap();
        assert_eq!(loaded.sui.kms_key_version, None);
    }

    #[tokio::test]
    async fn load_from_path_or_default_starts_from_defaults_for_a_missing_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("missing").join("conf.toml");

        let conf = CliConf::load_from_path_or_default(&path)
            .await
            .expect("a missing file is not an error");

        assert_eq!(conf, CliConf::default());
    }

    #[tokio::test]
    async fn load_from_path_or_default_reads_an_existing_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("conf.toml");
        tokio::fs::write(&path, "[sui]\nrpc_url = \"https://rpc.example.com\"\n")
            .await
            .unwrap();

        let conf = CliConf::load_from_path_or_default(&path)
            .await
            .expect("a valid file loads");

        assert_eq!(
            conf.sui.rpc_url,
            Some(reqwest::Url::parse("https://rpc.example.com").unwrap())
        );
    }

    #[tokio::test]
    async fn load_from_path_or_default_rejects_an_unparsable_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("conf.toml");
        tokio::fs::write(&path, "[sui]\nrpc_url = \n")
            .await
            .unwrap();

        let error = CliConf::load_from_path_or_default(&path)
            .await
            .expect_err("invalid TOML must not fall back to defaults");

        let message = error.to_string();
        assert!(
            message.contains("Failed to parse Nexus CLI configuration"),
            "unexpected error: {message}"
        );
        assert!(message.contains(&path.display().to_string()));
    }
}
