//! Runtime policy for the CLI’s local at-rest secrets.
//!
//! This module answers two questions:
//! 1. Should this process encrypt secrets at rest? (`auto` / `require` / `off`)
//! 2. Is the keyring usable right now? (headless/CI environments often disable it)
//!
//! The policy is consumed in two places:
//! - [`crate::secrets::store::CliKeyProvider`] consults [`mode`] to decide whether to return a key
//!   to the SDK secret wrapper.
//! - [`prepare_for_secret_write`] runs before persisting secret state (e.g. saving
//!   `~/.nexus/crypto.toml`) to ensure the key exists in `auto` mode and to fail early in
//!   `require` mode.
//!
//! # Modes
//! - `auto` (default): try to encrypt when possible. If the keyring is unavailable for this run,
//!   warn once and write secrets in plaintext so the CLI still works.
//! - `require`: encryption is mandatory. Missing/unavailable keys cause an error.
//! - `off`: never encrypt; always store secrets as plaintext.
//!
//! # Sources / precedence
//! - `NEXUS_SECRETS_MODE` environment variable (highest priority)
//! - `secrets.mode` in `~/.nexus/conf.toml`
//! - default (`auto`)
//!
//! # Process-level caching
//! The resolved mode is cached in statics so serialization can be cheap (it happens inside serde
//! calls). Commands that rewrite state (enable/disable/rotate/wipe) can override the cached value
//! using [`set_mode_for_process`].

use {
    super::master_key,
    crate::{
        cli_conf::{CliConf, SecretsMode},
        notify_success,
        notify_warning,
    },
    anyhow::{bail, Error as AnyError, Result as AnyResult},
    serde::{Deserialize, Serialize},
    std::{
        path::PathBuf,
        sync::atomic::{AtomicBool, AtomicU8, Ordering},
    },
    thiserror::Error,
};

pub(crate) const SECRETS_MODE_ENV: &str = "NEXUS_SECRETS_MODE";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ModeSource {
    /// Effective mode was taken from `NEXUS_SECRETS_MODE`.
    Env,
    /// Effective mode came from `~/.nexus/conf.toml`.
    Config,
    /// Effective mode fell back to the default.
    Default,
}

impl std::fmt::Display for ModeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeSource::Env => write!(f, "env"),
            ModeSource::Config => write!(f, "config"),
            ModeSource::Default => write!(f, "default"),
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum SecretsPolicyError {
    #[error("invalid {SECRETS_MODE_ENV}={value:?} (expected: auto | require | off)")]
    InvalidEnv { value: String },
}

const MODE_AUTO: u8 = 0;
const MODE_REQUIRE: u8 = 1;
const MODE_OFF: u8 = 2;

static MODE: AtomicU8 = AtomicU8::new(MODE_AUTO);
static MODE_INITIALIZED: AtomicBool = AtomicBool::new(false);
static KEYRING_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

static NOTIFIED_AUTO_ENABLE: AtomicBool = AtomicBool::new(false);
static NOTIFIED_KEYRING_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

fn encode_mode(mode: SecretsMode) -> u8 {
    match mode {
        SecretsMode::Auto => MODE_AUTO,
        SecretsMode::Require => MODE_REQUIRE,
        SecretsMode::Off => MODE_OFF,
    }
}

fn decode_mode(v: u8) -> SecretsMode {
    match v {
        MODE_REQUIRE => SecretsMode::Require,
        MODE_OFF => SecretsMode::Off,
        _ => SecretsMode::Auto,
    }
}

/// Override the resolved secrets mode for the current process.
///
/// This is primarily used by `nexus secrets *` commands while rewriting local state so that reads
/// and writes behave consistently even if the on-disk config is being modified mid-command.
pub(crate) fn set_mode_for_process(mode: SecretsMode) {
    MODE.store(encode_mode(mode), Ordering::Relaxed);
    MODE_INITIALIZED.store(true, Ordering::Relaxed);
    if mode != SecretsMode::Auto {
        KEYRING_UNAVAILABLE.store(false, Ordering::Relaxed);
    }
}

/// True if the keyring has been detected as unavailable for this process in `auto` mode.
///
/// When this is set, the CLI will avoid repeatedly attempting keyring operations and will
/// serialize secrets as plaintext for the remainder of the process.
pub(crate) fn keyring_unavailable_for_auto() -> bool {
    KEYRING_UNAVAILABLE.load(Ordering::Relaxed)
}

fn parse_mode(value: &str) -> Result<SecretsMode, SecretsPolicyError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(SecretsMode::Auto),
        "require" => Ok(SecretsMode::Require),
        "off" => Ok(SecretsMode::Off),
        other => Err(SecretsPolicyError::InvalidEnv {
            value: other.to_owned(),
        }),
    }
}

/// Resolve an effective secrets mode from a configured value.
///
/// This is used for reporting (`nexus secrets status`). It follows the same precedence order as
/// the runtime:
/// - `NEXUS_SECRETS_MODE` environment variable (if set)
/// - the provided `configured` mode (typically read from `~/.nexus/conf.toml`)
pub(crate) fn resolve_mode(
    _conf_path: &PathBuf,
    configured: SecretsMode,
) -> Result<(SecretsMode, ModeSource), SecretsPolicyError> {
    if let Ok(v) = std::env::var(SECRETS_MODE_ENV) {
        return Ok((parse_mode(&v)?, ModeSource::Env));
    }

    Ok((configured, ModeSource::Config))
}

fn default_cli_conf_path() -> Option<PathBuf> {
    home::home_dir().map(|home| home.join(".nexus").join("conf.toml"))
}

fn init_mode_from_env_and_default() -> Result<(SecretsMode, ModeSource), SecretsPolicyError> {
    if let Ok(v) = std::env::var(SECRETS_MODE_ENV) {
        return Ok((parse_mode(&v)?, ModeSource::Env));
    }

    // Tests should never consult the user's real config.
    if cfg!(test) {
        return Ok((SecretsMode::Auto, ModeSource::Default));
    }

    let Some(conf_path) = default_cli_conf_path() else {
        return Ok((SecretsMode::Auto, ModeSource::Default));
    };

    let configured = std::fs::read_to_string(&conf_path)
        .ok()
        .and_then(|s| toml::from_str::<CliConf>(&s).ok())
        .map(|c| c.secrets.mode)
        .unwrap_or_default();

    Ok((configured, ModeSource::Config))
}

/// Return the effective secrets mode for this process.
///
/// On first use, this resolves the mode from `NEXUS_SECRETS_MODE` or from the user’s config (unless
/// running under tests), then caches it in statics. Subsequent calls are cheap.
pub(crate) fn mode() -> Result<SecretsMode, SecretsPolicyError> {
    if MODE_INITIALIZED.load(Ordering::Relaxed) {
        return Ok(decode_mode(MODE.load(Ordering::Relaxed)));
    }

    let (mode, _source) = init_mode_from_env_and_default()?;
    set_mode_for_process(mode);
    Ok(mode)
}

/// Ensure policy + key state is ready for writing secrets.
///
/// - `auto`: creates a key if missing and the keyring is available; otherwise warns once and
///   continues in plaintext mode.
/// - `require`: errors if a key can't be loaded.
/// - `off`: no-op.
pub(crate) fn prepare_for_secret_write() -> AnyResult<()> {
    let mode = self::mode().map_err(AnyError::from)?;

    match mode {
        SecretsMode::Off => Ok(()),
        SecretsMode::Require => {
            if master_key::load_master_key()?.is_some() {
                return Ok(());
            }

            bail!("No master key found. Run `nexus secrets enable` to enable at-rest encryption.");
        }
        SecretsMode::Auto => match master_key::ensure_master_key_exists() {
            Ok(master_key::EnsureMasterKey::AlreadyExists) => Ok(()),
            Ok(master_key::EnsureMasterKey::Created) => {
                if !NOTIFIED_AUTO_ENABLE.swap(true, Ordering::Relaxed) {
                    notify_success!(
                        "At-rest encryption auto-enabled (created a master key in the keyring)"
                    );
                }
                Ok(())
            }
            Err(master_key::MasterKeyError::Keyring(e)) => {
                KEYRING_UNAVAILABLE.store(true, Ordering::Relaxed);

                if !NOTIFIED_KEYRING_UNAVAILABLE.swap(true, Ordering::Relaxed) {
                    notify_warning!(
                        "Keyring unavailable ({e}); storing secrets in plaintext for this run"
                    );
                }

                Ok(())
            }
            Err(e) => Err(e.into()),
        },
    }
}

#[cfg(test)]
/// Reset all policy state so tests can run deterministically.
pub(crate) fn reset_for_tests() {
    MODE.store(MODE_AUTO, Ordering::Relaxed);
    MODE_INITIALIZED.store(false, Ordering::Relaxed);
    KEYRING_UNAVAILABLE.store(false, Ordering::Relaxed);
    NOTIFIED_AUTO_ENABLE.store(false, Ordering::Relaxed);
    NOTIFIED_KEYRING_UNAVAILABLE.store(false, Ordering::Relaxed);
    std::env::remove_var(SECRETS_MODE_ENV);
}
