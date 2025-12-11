use {
    crate::prelude::*,
    nexus_sdk::types::{StorageConf, StorageKind},
    std::sync::Arc,
    tokio::sync::Mutex,
};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct SuiConf {
    #[serde(default)]
    pub(crate) pk: Option<PathBuf>,
    #[serde(default)]
    pub(crate) grpc_url: Option<reqwest::Url>,
    #[serde(default)]
    pub(crate) gql_url: Option<reqwest::Url>,
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

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct CryptoConf {
    /// User's long-term identity key (None until first generated)
    identity_key: Option<Secret<IdentityKey>>,
    /// Stored Double-Ratchet sessions keyed by their 32-byte session-id.
    #[serde(default)]
    sessions: Secret<HashMap<[u8; 32], Session>>,
}

// Custom implementations because `IdentityKey` does not implement common traits.
impl std::fmt::Debug for CryptoConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CryptoConf")
            // Avoid printing sensitive material.
            .field("Key exists: ", &self.identity_key.is_some())
            .field("# of sessions: ", &self.sessions.value.len())
            .finish()
    }
}

impl CryptoConf {
    /// Truncate the configuration (remove identity key and all sessions).
    pub(crate) async fn truncate(path: Option<&PathBuf>) -> AnyResult<()> {
        let default_path = expand_tilde(CRYPTO_CONF_PATH)?;
        let conf_path = path.unwrap_or(&default_path);

        CryptoConf::default().save_to_path(conf_path).await
    }

    /// Return the identity key if it exists.
    pub(crate) async fn get_identity_key(path: Option<&PathBuf>) -> AnyResult<Secret<IdentityKey>> {
        let default_path = expand_tilde(CRYPTO_CONF_PATH)?;
        let conf_path = path.unwrap_or(&default_path);

        let crypto_conf = CryptoConf::load_from_path(conf_path).await?;
        crypto_conf
            .identity_key
            .ok_or_else(|| anyhow!("No identity key found"))
    }

    /// Update the identity key in the configuration.
    pub(crate) async fn set_identity_key(
        identity_key: IdentityKey,
        path: Option<&PathBuf>,
    ) -> AnyResult<()> {
        let default_path = expand_tilde(CRYPTO_CONF_PATH)?;
        let conf_path = path.unwrap_or(&default_path);

        let mut crypto_conf = CryptoConf::load_from_path(conf_path)
            .await
            .unwrap_or_default();

        crypto_conf.identity_key = Some(Secret::new(identity_key));

        crypto_conf.save_to_path(conf_path).await
    }

    /// Get an [`Arc<Mutex>`] of an active session.
    pub(crate) async fn get_active_session(
        path: Option<&PathBuf>,
    ) -> AnyResult<Arc<Mutex<Session>>> {
        let default_path = expand_tilde(CRYPTO_CONF_PATH)?;
        let conf_path = path.unwrap_or(&default_path);

        let mut crypto_conf = CryptoConf::load_from_path(conf_path)
            .await
            .unwrap_or_default();

        let session_id = crypto_conf
            .sessions
            .keys()
            .cloned()
            .next()
            .ok_or_else(|| anyhow!("No active sessions found"))?;

        let session = crypto_conf
            .sessions
            .remove(&session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;

        // Save the crypto conf with the removed session to prevent reuse.
        crypto_conf.save_to_path(conf_path).await?;

        Ok(Arc::new(Mutex::new(session)))
    }

    /// Release the updated session back to the configuration.
    pub(crate) async fn release_session(
        session: Arc<Mutex<Session>>,
        path: Option<&PathBuf>,
    ) -> AnyResult<()> {
        let Some(session) = Arc::into_inner(session).map(|session| session.into_inner()) else {
            bail!("Failed to unwrap session Arc for saving");
        };

        CryptoConf::insert_session(session, path).await
    }

    /// Insert a session directly.
    pub(crate) async fn insert_session(session: Session, path: Option<&PathBuf>) -> AnyResult<()> {
        let default_path = expand_tilde(CRYPTO_CONF_PATH)?;
        let conf_path = path.unwrap_or(&default_path);

        let mut crypto_conf = CryptoConf::load_from_path(conf_path)
            .await
            .unwrap_or_default();

        crypto_conf.sessions.insert(*session.id(), session);

        crypto_conf.save_to_path(conf_path).await
    }

    /// Helper to load from a specific path.
    async fn load_from_path(path: &PathBuf) -> AnyResult<Self> {
        let conf = tokio::fs::read_to_string(path).await?;

        Ok(toml::from_str(&conf)?)
    }

    /// Helper to save to a specific path.
    async fn save_to_path(&self, path: &PathBuf) -> AnyResult<()> {
        let parent_folder = path.parent().expect("Parent folder must exist.");
        let conf = toml::to_string_pretty(&self)?;

        tokio::fs::create_dir_all(parent_folder).await?;
        tokio::fs::write(path, conf).await?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::single_component_path_imports)]
mod tests {
    use {super::*, nexus_sdk::crypto::x3dh::PreKeyBundle, serial_test::serial, std::fs};

    fn setup_env() -> tempfile::TempDir {
        std::env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "test_passphrase");
        let secret_home = tempfile::tempdir().unwrap();

        // Use dedicated sub-directories to avoid interfering with the caller's real home.
        let home_dir = secret_home.path().join("home");
        let xdg_config_dir = secret_home.path().join("xdg_config");
        let xdg_data_dir = secret_home.path().join("xdg_data");

        fs::create_dir_all(&home_dir).unwrap();
        fs::create_dir_all(&xdg_config_dir).unwrap();
        fs::create_dir_all(&xdg_data_dir).unwrap();

        std::env::set_var("HOME", &home_dir);
        std::env::set_var("XDG_CONFIG_HOME", &xdg_config_dir);
        std::env::set_var("XDG_DATA_HOME", &xdg_data_dir);
        secret_home
    }

    fn cleanup_env() {
        std::env::remove_var("NEXUS_CLI_STORE_PASSPHRASE");
        std::env::remove_var("HOME");
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

        // Save using the new interface
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        // Load using the new interface
        let loaded = CryptoConf::load_from_path(&path).await.unwrap();

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

        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        let conf = CryptoConf::load_from_path(&path).await.unwrap_or_default();
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
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        let session = CryptoConf::get_active_session(Some(&path)).await;
        assert!(session.is_ok());

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_get_active_session_error() {
        let secret_home = setup_env();

        let conf = CryptoConf::default();
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        let result = CryptoConf::get_active_session(Some(&path)).await;
        assert!(result.is_err());
        assert!(result
            .as_ref()
            .err()
            .unwrap()
            .to_string()
            .contains("No active sessions found"));

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_get_active_session_multiple_sessions() {
        let secret_home = setup_env();

        // Insert two sessions
        let mut sessions = HashMap::new();
        let dummy_session_1 = create_test_session();
        let session_id_1 = *dummy_session_1.id();
        sessions.insert(session_id_1, dummy_session_1);

        let conf = CryptoConf {
            identity_key: Some(Secret::new(IdentityKey::generate())),
            sessions: Secret::new(sessions),
        };
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        // Take out first session
        let _session1 = CryptoConf::get_active_session(Some(&path)).await.unwrap();

        // Try to take out third session (should fail)
        let result = CryptoConf::get_active_session(Some(&path)).await;
        assert!(result.is_err());
        assert!(result
            .as_ref()
            .err()
            .unwrap()
            .to_string()
            .contains("No active sessions found"));

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_crypto_conf_truncate() {
        let secret_home = setup_env();

        // Create and save a config with identity key and sessions
        let mut sessions = HashMap::new();
        let dummy_session = create_test_session();
        sessions.insert([3u8; 32], dummy_session);

        let conf = CryptoConf {
            identity_key: Some(Secret::new(IdentityKey::generate())),
            sessions: Secret::new(sessions),
        };
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        // Truncate the config
        CryptoConf::truncate(Some(&path)).await.unwrap();

        // Load and check that identity_key and sessions are cleared
        let loaded = CryptoConf::load_from_path(&path).await.unwrap();
        assert!(loaded.identity_key.is_none());
        assert_eq!(loaded.sessions.value.len(), 0);

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_crypto_conf_release_session() {
        let secret_home = setup_env();

        // Create and save a config with one session
        let mut sessions = HashMap::new();
        let dummy_session = create_test_session();
        let session_id = *dummy_session.id();
        sessions.insert(session_id, dummy_session);

        let conf = CryptoConf {
            identity_key: Some(Secret::new(IdentityKey::generate())),
            sessions: Secret::new(sessions),
        };
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        // Get and remove the active session
        let session = CryptoConf::get_active_session(Some(&path)).await.unwrap();

        // Release the session back to the config
        CryptoConf::release_session(session, Some(&path))
            .await
            .expect("Failed to release session");

        // Load and check that the session is present again
        let loaded = CryptoConf::load_from_path(&path).await.unwrap();

        println!("Loaded sessions: {:?}", loaded);
        assert_eq!(loaded.sessions.len(), 1);
        assert!(loaded.sessions.contains_key(&session_id));

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_insert_session() {
        let secret_home = setup_env();

        let session = create_test_session();
        let session_id = *session.id();

        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();

        // Start with empty config
        CryptoConf::truncate(Some(&path)).await.unwrap();

        // Insert session
        CryptoConf::insert_session(session, Some(&path))
            .await
            .unwrap();

        // Load and check session exists
        let loaded = CryptoConf::load_from_path(&path).await.unwrap();
        assert_eq!(loaded.sessions.len(), 1);
        assert!(loaded.sessions.contains_key(&session_id));

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_get_identity_key_success() {
        let secret_home = setup_env();

        let identity_key = IdentityKey::generate();
        let conf = CryptoConf {
            identity_key: Some(Secret::new(identity_key)),
            sessions: Secret::new(HashMap::new()),
        };
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        let _loaded_key = CryptoConf::get_identity_key(Some(&path))
            .await
            .expect("Failed to get identity key");

        cleanup_env();
        drop(secret_home);
    }

    #[tokio::test]
    #[serial(master_key_env)]
    async fn test_get_identity_key_error() {
        let secret_home = setup_env();

        let conf = CryptoConf::default();
        let path = expand_tilde(CRYPTO_CONF_PATH).unwrap();
        conf.save_to_path(&path).await.unwrap();

        let result = CryptoConf::get_identity_key(Some(&path)).await;
        assert!(result.is_err());
        assert!(result
            .as_ref()
            .err()
            .unwrap()
            .to_string()
            .contains("No identity key found"));

        cleanup_env();
        drop(secret_home);
    }
}
