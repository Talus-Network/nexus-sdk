//! Toolkit runtime configuration.
//!
//! This module defines the single JSON config file consumed by the
//! `nexus-toolkit` HTTP runtime (see [`crate::bootstrap!`]).
//!
//! The goals are:
//! - Keep configuration deployment-friendly (one file mounted into the tool container/VM).
//! - Avoid per-tool env var naming schemes (like `NEXUS_TOOL_SIGNING_KEY_<SUFFIX>`).
//! - Tools only need a local allowlist of permitted Leaders (public keys) plus their own Tool signing key.
//!
//! # Loading
//! The runtime loads this config from the path stored in [`ENV_TOOLKIT_CONFIG_PATH`].
//! If the env var is not set, safe defaults are used (signed HTTP disabled).
//!
//! # DoS protection: request body size
//! `/invoke` requests MUST include a `Content-Length` header and it MUST be less than or equal to
//! [`ToolkitRuntimeConfig::invoke_max_body_bytes`]. This is enforced using
//! `warp::body::content_length_limit`, which rejects requests without `Content-Length`.
//!
//! # Signed HTTP (application-layer signatures)
//! If the config includes a `signed_http` section in `required` mode, the runtime:
//! - Rejects any `/invoke` request that does not carry valid signature headers.
//! - Rejects any request signed by a Leader not present in the local allowlist.
//! - Signs the tool response (including error responses after authentication) so the Leader can verify provenance.
//!
//! Operational note: if you run tools behind a gateway/proxy, ensure it forwards the `X-Nexus-Sig-*`
//! headers. If these headers are stripped, signed HTTP will fail closed.
//!
//! The signature protocol itself lives in `nexus-sdk` under
//! [`nexus_sdk::signed_http::v1`](nexus_sdk::signed_http::v1).
//!
//! # Example config (v1)
//! ```json
//! {
//!   "version": 1,
//!   "invoke_max_body_bytes": 10485760,
//!   "signed_http": {
//!     "mode": "required",
//!     "allowed_leaders_path": "./allowed_leaders.json",
//!     "tools": {
//!       "xyz.dummy.tool@1": {
//!         "tool_kid": 0,
//!         "tool_signing_key": "0000000000000000000000000000000000000000000000000000000000000000"
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! # Example (Rust)
//! This example parses a config from JSON (no filesystem required) by embedding the
//! allowlist inline.
//!
//! ```
//! use {
//!     ed25519_dalek::SigningKey,
//!     nexus_toolkit::ToolkitRuntimeConfig,
//!     serde_json::{json, Map},
//! };
//!
//! let tool_id = "xyz.dummy.tool@1";
//! let tool_sk_hex = hex::encode([7u8; 32]);
//! let leader_sk = SigningKey::from_bytes(&[9u8; 32]);
//! let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());
//!
//! let mut tools = Map::new();
//! tools.insert(
//!     tool_id.to_string(),
//!     json!({
//!         "tool_kid": 0,
//!         "tool_signing_key": tool_sk_hex,
//!     }),
//! );
//!
//! let cfg_json = serde_json::to_string_pretty(&json!({
//!     "version": 1,
//!     "invoke_max_body_bytes": 123,
//!     "signed_http": {
//!         "mode": "required",
//!         "allowed_leaders": {
//!             "version": 1,
//!             "leaders": [{
//!                 "leader_id": "0x1111",
//!                 "keys": [{
//!                     "kid": 0,
//!                     "public_key": leader_pk_hex,
//!                 }],
//!             }],
//!         },
//!         "tools": tools,
//!     },
//! }))
//! .unwrap();
//!
//! let cfg = ToolkitRuntimeConfig::from_json_str(&cfg_json).unwrap();
//! assert_eq!(cfg.invoke_max_body_bytes(), 123);
//! assert!(cfg.signed_http_is_required());
//! assert!(cfg.has_tool(tool_id));
//! ```

use {
    anyhow::Context as _,
    ed25519_dalek::SigningKey,
    nexus_sdk::signed_http::{
        keys::parse_ed25519_signing_key,
        v1::wire::{AllowedLeadersFileV1, AllowedLeadersV1},
    },
    notify::{Event, RecommendedWatcher, RecursiveMode, Watcher},
    serde::Deserialize,
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        sync::{Arc, RwLock},
        time::Duration,
    },
};

/// Env var read by the toolkit runtime to locate its JSON config file.
pub const ENV_TOOLKIT_CONFIG_PATH: &str = "NEXUS_TOOLKIT_CONFIG_PATH";

const DEFAULT_INVOKE_MAX_BODY_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB
const DEFAULT_MAX_CLOCK_SKEW_MS: u64 = 30_000;
const DEFAULT_MAX_VALIDITY_MS: u64 = 60_000;

/// Signed HTTP mode for the toolkit runtime.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignedHttpMode {
    /// Do not require signature headers.
    Disabled,
    /// Reject any request missing/invalid signature headers.
    #[default]
    Required,
}

/// Fully loaded toolkit runtime config (validated + ready to use).
///
/// This type is primarily used internally by the runtime; it is public so the
/// config format can be documented and unit-tested.
#[derive(Clone)]
pub struct ToolkitRuntimeConfig {
    invoke_max_body_bytes: u64,
    signed_http: Option<SignedHttpRuntimeConfig>,
    source_path: Option<PathBuf>,
}

#[derive(Clone)]
pub(crate) struct SignedHttpRuntimeConfig {
    pub(crate) allowed_leaders: Arc<AllowedLeadersV1>,
    pub(crate) max_clock_skew_ms: u64,
    pub(crate) max_validity_ms: u64,
    pub(crate) tools: BTreeMap<String, SignedHttpToolRuntimeConfig>,
}

#[derive(Clone)]
pub(crate) struct SignedHttpToolRuntimeConfig {
    pub(crate) tool_kid: u64,
    pub(crate) tool_signing_key: SigningKey,
}

impl ToolkitRuntimeConfig {
    /// Load config from [`ENV_TOOLKIT_CONFIG_PATH`] if set, otherwise return defaults.
    pub fn from_env() -> anyhow::Result<Self> {
        let Some(path) = std::env::var(ENV_TOOLKIT_CONFIG_PATH).ok() else {
            return Ok(Self::default_for_runtime());
        };
        Self::from_path(path)
    }

    /// Parse config from a JSON file.
    pub fn from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let mut cfg = Self::from_json_bytes(&bytes).with_context(|| {
            format!(
                "failed to parse {} (expected ToolkitRuntimeConfig v1 JSON)",
                path.display()
            )
        })?;
        cfg.source_path = Some(path.to_path_buf());
        Ok(cfg)
    }

    /// Parse config from JSON bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let file: ToolkitRuntimeConfigFileV1 =
            serde_json::from_slice(bytes).context("invalid JSON")?;
        Self::try_from(file)
    }

    /// Parse config from a JSON string.
    pub fn from_json_str(json: &str) -> anyhow::Result<Self> {
        Self::from_json_bytes(json.as_bytes())
    }

    /// Maximum allowed `/invoke` request body size in bytes.
    pub fn invoke_max_body_bytes(&self) -> u64 {
        self.invoke_max_body_bytes
    }

    /// True if the runtime requires signed HTTP requests.
    pub fn signed_http_is_required(&self) -> bool {
        self.signed_http.is_some()
    }

    /// True if `tool_id` has a signing key configured in the current config.
    pub fn has_tool(&self, tool_id: &str) -> bool {
        self.signed_http
            .as_ref()
            .is_some_and(|s| s.tools.contains_key(tool_id))
    }

    pub(crate) fn signed_http(&self) -> Option<&SignedHttpRuntimeConfig> {
        self.signed_http.as_ref()
    }

    fn default_for_runtime() -> Self {
        Self {
            invoke_max_body_bytes: DEFAULT_INVOKE_MAX_BODY_BYTES,
            signed_http: None,
            source_path: None,
        }
    }

    pub(crate) fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

#[derive(Clone, Debug, Deserialize)]
struct ToolkitRuntimeConfigFileV1 {
    pub version: u8,
    #[serde(default)]
    pub invoke_max_body_bytes: Option<u64>,
    #[serde(default)]
    pub signed_http: Option<SignedHttpConfigFileV1>,
}

#[derive(Clone, Debug, Deserialize)]
struct SignedHttpConfigFileV1 {
    /// Defaults to `required` when omitted.
    #[serde(default)]
    pub mode: SignedHttpMode,

    /// Path to an allowlist file (same schema as `AllowedLeadersFileV1`).
    #[serde(default)]
    pub allowed_leaders_path: Option<String>,

    /// Inline allowlist (useful for tests and simple deployments).
    #[serde(default)]
    pub allowed_leaders: Option<AllowedLeadersFileV1>,

    #[serde(default)]
    pub max_clock_skew_ms: Option<u64>,
    #[serde(default)]
    pub max_validity_ms: Option<u64>,

    /// Per-tool signing material, keyed by `tool_id` string.
    #[serde(default)]
    pub tools: BTreeMap<String, SignedHttpToolConfigFileV1>,
}

#[derive(Clone, Debug, Deserialize)]
struct SignedHttpToolConfigFileV1 {
    pub tool_kid: u64,
    /// Hex or base64 encoding of a 32-byte Ed25519 private key.
    ///
    /// This also accepts Sui keytool encoding: base64 of `0x00 || sk32`.
    pub tool_signing_key: String,
}

impl TryFrom<ToolkitRuntimeConfigFileV1> for ToolkitRuntimeConfig {
    type Error = anyhow::Error;

    fn try_from(file: ToolkitRuntimeConfigFileV1) -> Result<Self, Self::Error> {
        if file.version != 1 {
            anyhow::bail!("unsupported config version {}, expected 1", file.version);
        }

        let invoke_max_body_bytes = file
            .invoke_max_body_bytes
            .unwrap_or(DEFAULT_INVOKE_MAX_BODY_BYTES);

        let signed_http = match file.signed_http {
            None => None,
            Some(s) if s.mode == SignedHttpMode::Disabled => None,
            Some(s) => Some(load_signed_http_config(s)?),
        };

        Ok(Self {
            invoke_max_body_bytes,
            signed_http,
            source_path: None,
        })
    }
}

fn load_signed_http_config(
    file: SignedHttpConfigFileV1,
) -> anyhow::Result<SignedHttpRuntimeConfig> {
    let allowed_leaders = match (file.allowed_leaders, file.allowed_leaders_path) {
        (Some(inline), _) => AllowedLeadersV1::try_from(inline).map_err(anyhow::Error::new)?,
        (None, Some(path)) => AllowedLeadersV1::from_path(path).map_err(anyhow::Error::new)?,
        (None, None) => {
            anyhow::bail!("signed_http requires either allowed_leaders or allowed_leaders_path")
        }
    };
    let allowed_leaders = Arc::new(allowed_leaders);

    if file.tools.is_empty() {
        anyhow::bail!("signed_http.tools must contain at least one tool entry");
    }

    let mut tools: BTreeMap<String, SignedHttpToolRuntimeConfig> = BTreeMap::new();
    for (tool_id, tool) in file.tools {
        let signing_key = parse_ed25519_signing_key(&tool.tool_signing_key).map_err(|e| {
            anyhow::anyhow!("invalid signed_http.tools[\"{tool_id}\"].tool_signing_key: {e}")
        })?;
        tools.insert(
            tool_id,
            SignedHttpToolRuntimeConfig {
                tool_kid: tool.tool_kid,
                tool_signing_key: signing_key,
            },
        );
    }

    Ok(SignedHttpRuntimeConfig {
        allowed_leaders,
        max_clock_skew_ms: file.max_clock_skew_ms.unwrap_or(DEFAULT_MAX_CLOCK_SKEW_MS),
        max_validity_ms: file.max_validity_ms.unwrap_or(DEFAULT_MAX_VALIDITY_MS),
        tools,
    })
}

/// Internal config holder with automatic file watching.
///
/// This is used internally by the runtime to enable hot-reload of configuration
/// without requiring a process restart. When the config file changes, the new
/// configuration is automatically loaded.
#[doc(hidden)]
pub struct Config {
    config: Arc<RwLock<Arc<ToolkitRuntimeConfig>>>,
    #[allow(dead_code)]
    watcher: Option<RecommendedWatcher>,
}

impl Config {
    /// Create a new config watcher from the environment.
    ///
    /// If [`ENV_TOOLKIT_CONFIG_PATH`] is set, starts a file watcher that reloads
    /// config on changes automatically.
    #[doc(hidden)]
    #[cfg(test)]
    pub async fn from_env() -> anyhow::Result<Arc<Self>> {
        let path = std::env::var(ENV_TOOLKIT_CONFIG_PATH)
            .ok()
            .map(PathBuf::from);

        let initial_config = match &path {
            Some(p) => ToolkitRuntimeConfig::from_path(p)?,
            None => ToolkitRuntimeConfig::default_for_runtime(),
        };

        let config = Arc::new(RwLock::new(Arc::new(initial_config)));

        let watcher = if let Some(ref p) = path {
            Some(Self::start_watcher(p.clone(), Arc::clone(&config))?)
        } else {
            None
        };

        Ok(Arc::new(Self { config, watcher }))
    }

    /// Wrap an existing config, watching its source file if it has one.
    ///
    /// If the config was loaded from a file (has a `source_path`), a file watcher
    /// is set up for automatic hot-reload. Otherwise, the config is wrapped without
    /// file watching.
    #[doc(hidden)]
    pub fn from_config(config: Arc<ToolkitRuntimeConfig>) -> Arc<Self> {
        let path = config.source_path().map(|p| p.to_path_buf());
        let config_holder = Arc::new(RwLock::new(config));

        let watcher = path.and_then(|p| {
            Self::start_watcher(p, Arc::clone(&config_holder))
                .map_err(|e| {
                    tracing::warn!("Failed to start config file watcher: {e}");
                    e
                })
                .ok()
        });

        Arc::new(Self {
            config: config_holder,
            watcher,
        })
    }

    /// Get the current configuration.
    #[doc(hidden)]
    pub fn current(&self) -> Arc<ToolkitRuntimeConfig> {
        self.config.read().unwrap().clone()
    }

    fn start_watcher(
        path: PathBuf,
        config: Arc<RwLock<Arc<ToolkitRuntimeConfig>>>,
    ) -> anyhow::Result<RecommendedWatcher> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    // Non-blocking send - if channel is full, we'll catch the next event
                    let _ = tx.try_send(());
                }
            }
        })?;

        // Watch the parent directory for ConfigMap atomic updates
        let watch_path = path.parent().unwrap_or(&path);
        watcher.watch(watch_path, RecursiveMode::NonRecursive)?;

        tracing::info!("Started config file watcher for {}", path.display());

        // Spawn reload task
        let reload_path = path.clone();
        tokio::spawn(async move {
            // Debounce interval to let writes settle
            let debounce_duration = Duration::from_millis(500);

            while rx.recv().await.is_some() {
                // Debounce: wait for writes to settle
                tokio::time::sleep(debounce_duration).await;

                // Drain any additional events that arrived during debounce
                while rx.try_recv().is_ok() {}

                match ToolkitRuntimeConfig::from_path(&reload_path) {
                    Ok(new_config) => {
                        let mut guard = config.write().unwrap();
                        *guard = Arc::new(new_config);
                        tracing::info!("Reloaded toolkit config from {}", reload_path.display());
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to reload toolkit config from {}: {e}",
                            reload_path.display()
                        );
                        // Keep using old config
                    }
                }
            }
        });

        Ok(watcher)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde_json::{json, Map},
    };

    fn make_config_json(
        tool_id: &str,
        tool_sk_hex: &str,
        leader_id: &str,
        leader_pk_hex: &str,
    ) -> String {
        let mut tools = Map::new();
        tools.insert(
            tool_id.to_string(),
            json!({
                "tool_kid": 7,
                "tool_signing_key": tool_sk_hex,
            }),
        );

        serde_json::to_string(&json!({
            "version": 1,
            "invoke_max_body_bytes": 123,
            "signed_http": {
                "mode": "required",
                "allowed_leaders": {
                    "version": 1,
                    "leaders": [{
                        "leader_id": leader_id,
                        "keys": [{
                            "kid": 0,
                            "public_key": leader_pk_hex,
                        }],
                    }],
                },
                "max_clock_skew_ms": 10,
                "max_validity_ms": 20,
                "tools": tools,
            },
        }))
        .unwrap()
    }

    #[test]
    fn parse_signed_http_inline_config() {
        let leader_sk = SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());
        let tool_id = "xyz.demo.tool@1";
        let tool_sk_hex = hex::encode([9u8; 32]);

        let cfg_json = make_config_json(tool_id, &tool_sk_hex, "0x1111", &leader_pk_hex);
        let cfg = ToolkitRuntimeConfig::from_json_str(&cfg_json).unwrap();

        assert_eq!(cfg.invoke_max_body_bytes(), 123);
        assert!(cfg.signed_http_is_required());
        assert!(cfg.has_tool(tool_id));

        let signed = cfg.signed_http().unwrap();
        assert_eq!(signed.max_clock_skew_ms, 10);
        assert_eq!(signed.max_validity_ms, 20);
        assert!(signed.allowed_leaders.source_path().is_none());
    }

    #[test]
    fn parse_signed_http_from_path() {
        let leader_sk = SigningKey::from_bytes(&[5u8; 32]);
        let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());
        let tool_sk_hex = hex::encode([6u8; 32]);

        let allowlist_json = serde_json::to_string(&json!({
            "version": 1,
            "leaders": [{
                "leader_id": "0x9999",
                "keys": [{
                    "kid": 0,
                    "public_key": leader_pk_hex,
                }],
            }],
        }))
        .unwrap();

        let file_name = format!(
            "nexus-allowlist-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(file_name);
        fs::write(&path, allowlist_json).unwrap();

        let cfg_json = serde_json::to_string(&json!({
            "version": 1,
            "signed_http": {
                "mode": "required",
                "allowed_leaders_path": path.display().to_string(),
                "tools": {
                    "xyz.demo.tool@1": {
                        "tool_kid": 0,
                        "tool_signing_key": tool_sk_hex,
                    },
                },
            },
        }))
        .unwrap();

        let cfg = ToolkitRuntimeConfig::from_json_str(&cfg_json).unwrap();
        assert!(cfg.signed_http_is_required());
        assert!(cfg.has_tool("xyz.demo.tool@1"));
        assert_eq!(
            cfg.signed_http().unwrap().allowed_leaders.source_path(),
            Some(path.as_path())
        );
    }

    #[test]
    fn signed_http_disabled_is_ignored() {
        let cfg_json = serde_json::to_string(&json!({
            "version": 1,
            "signed_http": {
                "mode": "disabled",
            },
        }))
        .unwrap();
        let cfg = ToolkitRuntimeConfig::from_json_str(&cfg_json).unwrap();
        assert!(!cfg.signed_http_is_required());
    }

    #[test]
    fn signed_http_requires_allowlist() {
        let cfg_json = serde_json::to_string(&json!({
            "version": 1,
            "signed_http": {
                "mode": "required",
                "tools": {
                    "demo": {
                        "tool_kid": 0,
                        "tool_signing_key": "00",
                    },
                },
            },
        }))
        .unwrap();
        assert!(ToolkitRuntimeConfig::from_json_str(&cfg_json).is_err());
    }

    #[test]
    fn signed_http_requires_tool_entries() {
        let cfg_json = serde_json::to_string(&json!({
            "version": 1,
            "signed_http": {
                "mode": "required",
                "allowed_leaders": {
                    "version": 1,
                    "leaders": [],
                },
            },
        }))
        .unwrap();
        assert!(ToolkitRuntimeConfig::from_json_str(&cfg_json).is_err());
    }

    #[test]
    fn rejects_unknown_config_version() {
        let cfg_json = serde_json::to_string(&json!({
            "version": 2,
        }))
        .unwrap();
        assert!(ToolkitRuntimeConfig::from_json_str(&cfg_json).is_err());
    }

    #[tokio::test]
    async fn config_watcher_loads_default_without_env_var() {
        // Ensure env var is not set
        std::env::remove_var(ENV_TOOLKIT_CONFIG_PATH);

        let watcher = Config::from_env().await.unwrap();
        let config = watcher.current();

        // Default config has no signed HTTP
        assert!(!config.signed_http_is_required());
        assert_eq!(config.invoke_max_body_bytes(), 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn config_watcher_loads_from_file() {
        let leader_sk = SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());
        let tool_id = "xyz.demo.tool@1";
        let tool_sk_hex = hex::encode([9u8; 32]);

        // Create config file
        let file_name = format!(
            "nexus-watcher-test-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(&file_name);

        let cfg_json = make_config_json(tool_id, &tool_sk_hex, "0x1111", &leader_pk_hex);
        fs::write(&path, &cfg_json).unwrap();

        // Set env var and create watcher
        std::env::set_var(ENV_TOOLKIT_CONFIG_PATH, path.display().to_string());

        let watcher = Config::from_env().await.unwrap();
        let config = watcher.current();

        // Verify config loaded correctly
        assert_eq!(config.invoke_max_body_bytes(), 123);
        assert!(config.signed_http_is_required());
        assert!(config.has_tool(tool_id));

        // Cleanup
        std::env::remove_var(ENV_TOOLKIT_CONFIG_PATH);
        let _ = fs::remove_file(&path);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn config_watcher_reloads_on_file_change() {
        let leader_sk = SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());
        let tool_id = "xyz.demo.tool@1";
        let tool_sk_hex = hex::encode([9u8; 32]);

        // Create initial config file
        let file_name = format!(
            "nexus-watcher-reload-test-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(&file_name);

        let cfg_json = make_config_json(tool_id, &tool_sk_hex, "0x1111", &leader_pk_hex);
        fs::write(&path, &cfg_json).unwrap();

        // Set env var and create watcher
        std::env::set_var(ENV_TOOLKIT_CONFIG_PATH, path.display().to_string());

        let watcher = Config::from_env().await.unwrap();

        // Verify initial config
        let config = watcher.current();
        assert_eq!(config.invoke_max_body_bytes(), 123);

        // Update the config file with new values
        let new_cfg_json = serde_json::to_string(&json!({
            "version": 1,
            "invoke_max_body_bytes": 456,
            "signed_http": {
                "mode": "required",
                "allowed_leaders": {
                    "version": 1,
                    "leaders": [{
                        "leader_id": "0x1111",
                        "keys": [{
                            "kid": 0,
                            "public_key": leader_pk_hex,
                        }],
                    }],
                },
                "tools": {
                    "xyz.demo.tool@1": {
                        "tool_kid": 7,
                        "tool_signing_key": tool_sk_hex,
                    },
                },
            },
        }))
        .unwrap();
        fs::write(&path, &new_cfg_json).unwrap();

        // Wait for debounce (500ms) + some buffer
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;

        // Verify config was reloaded
        let config = watcher.current();
        assert_eq!(config.invoke_max_body_bytes(), 456);

        // Cleanup
        std::env::remove_var(ENV_TOOLKIT_CONFIG_PATH);
        let _ = fs::remove_file(&path);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn config_watcher_keeps_old_on_invalid_update() {
        let leader_sk = SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());
        let tool_id = "xyz.demo.tool@1";
        let tool_sk_hex = hex::encode([9u8; 32]);

        // Create initial config file
        let file_name = format!(
            "nexus-watcher-invalid-test-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(&file_name);

        let cfg_json = make_config_json(tool_id, &tool_sk_hex, "0x1111", &leader_pk_hex);
        fs::write(&path, &cfg_json).unwrap();

        // Set env var and create watcher
        std::env::set_var(ENV_TOOLKIT_CONFIG_PATH, path.display().to_string());

        let watcher = Config::from_env().await.unwrap();

        // Verify initial config
        let config = watcher.current();
        assert_eq!(config.invoke_max_body_bytes(), 123);

        // Write invalid JSON to config file
        fs::write(&path, "{ invalid json }").unwrap();

        // Wait for debounce + buffer
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;

        // Config should still be the old one (invalid reload is ignored)
        let config = watcher.current();
        assert_eq!(config.invoke_max_body_bytes(), 123);

        // Cleanup
        std::env::remove_var(ENV_TOOLKIT_CONFIG_PATH);
        let _ = fs::remove_file(&path);
    }
}
