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
//! };
//!
//! let tool_id = "xyz.dummy.tool@1";
//! let tool_sk_hex = hex::encode([7u8; 32]);
//! let leader_sk = SigningKey::from_bytes(&[9u8; 32]);
//! let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());
//!
//! let cfg_json = format!(r#"{{
//!   "version": 1,
//!   "invoke_max_body_bytes": 123,
//!   "signed_http": {{
//!     "mode": "required",
//!     "allowed_leaders": {{
//!       "version": 1,
//!       "leaders": [{{"leader_id":"0x1111","keys":[{{"kid":0,"public_key":"{leader_pk_hex}"}}]}}]
//!     }},
//!     "tools": {{
//!       "{tool_id}": {{
//!         "tool_kid": 0,
//!         "tool_signing_key": "{tool_sk_hex}"
//!       }}
//!     }}
//!   }}
//! }}"#);
//!
//! let cfg = ToolkitRuntimeConfig::from_json_str(&cfg_json).unwrap();
//! assert_eq!(cfg.invoke_max_body_bytes(), 123);
//! assert!(cfg.signed_http_is_required());
//! assert!(cfg.has_tool(tool_id));
//! ```

use {
    anyhow::Context as _,
    base64::Engine as _,
    ed25519_dalek::SigningKey,
    nexus_sdk::signed_http::v1::{AllowedLeadersFileV1, AllowedLeadersV1},
    serde::Deserialize,
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    },
};

/// Env var read by the toolkit runtime to locate its JSON config file.
pub const ENV_TOOLKIT_CONFIG_PATH: &str = "NEXUS_TOOLKIT_CONFIG_PATH";

const DEFAULT_INVOKE_MAX_BODY_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB
const DEFAULT_MAX_CLOCK_SKEW_MS: u64 = 30_000;
const DEFAULT_MAX_VALIDITY_MS: u64 = 60_000;

/// Signed HTTP mode for the toolkit runtime.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignedHttpMode {
    /// Do not require signature headers.
    Disabled,
    /// Reject any request missing/invalid signature headers.
    Required,
}

impl Default for SignedHttpMode {
    fn default() -> Self {
        Self::Required
    }
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
    pub(crate) allowed_leaders: AllowedLeadersV1,
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

fn parse_ed25519_signing_key(raw: &str) -> Result<SigningKey, String> {
    // Try hex first (64 hex chars => 32 bytes).
    if let Ok(bytes) = hex::decode(raw) {
        if let Ok(arr) = <[u8; 32]>::try_from(bytes.as_slice()) {
            return Ok(SigningKey::from_bytes(&arr));
        }
    }

    // Try base64 (standard, with/without padding).
    let b64 = raw.trim();
    let try_b64 = |engine: &base64::engine::general_purpose::GeneralPurpose| -> Option<[u8; 32]> {
        engine
            .decode(b64.as_bytes())
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
    };

    let bytes = try_b64(&base64::engine::general_purpose::STANDARD)
        .or_else(|| try_b64(&base64::engine::general_purpose::STANDARD_NO_PAD))
        .or_else(|| try_b64(&base64::engine::general_purpose::URL_SAFE))
        .or_else(|| try_b64(&base64::engine::general_purpose::URL_SAFE_NO_PAD))
        .ok_or_else(|| "expected 32-byte ed25519 private key as hex or base64".to_string())?;

    Ok(SigningKey::from_bytes(&bytes))
}
