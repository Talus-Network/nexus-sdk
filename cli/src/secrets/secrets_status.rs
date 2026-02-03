use {
    crate::{
        command_title,
        display::json_output,
        item,
        prelude::*,
        secrets::store::{
            master_key::{SERVICE, USER},
            policy,
        },
    },
    keyring::Entry,
};

#[derive(Serialize)]
struct SecretsStatusJson {
    configured_mode: SecretsMode,
    effective_mode: SecretsMode,
    effective_mode_source: policy::ModeSource,
    keyring_available: bool,
    keyring_error: Option<String>,
    master_key_present: Option<bool>,
    writes: String,
}

pub(crate) async fn secrets_status(
    conf_path: PathBuf,
    _crypto_conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    let conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_default();

    let (effective_mode, mode_source) = policy::resolve_mode(&conf_path, conf.secrets.mode)
        .map_err(|e| NexusCliError::Any(e.into()))?;

    let (keyring_available, master_key_present, keyring_error) = match Entry::new(SERVICE, USER) {
        Ok(entry) => match entry.get_password() {
            Ok(_) => (true, Some(true), None),
            Err(keyring::Error::NoEntry) => (true, Some(false), None),
            Err(e) => (false, None, Some(e.to_string())),
        },
        Err(e) => (false, None, Some(e.to_string())),
    };

    let writes = match effective_mode {
        SecretsMode::Off => "plaintext (mode=off)".to_owned(),
        SecretsMode::Require => match (keyring_available, master_key_present) {
            (true, Some(true)) => "encrypted".to_owned(),
            _ => "blocked (mode=require, key unavailable)".to_owned(),
        },
        SecretsMode::Auto => {
            if keyring_available {
                match master_key_present {
                    Some(true) => "encrypted".to_owned(),
                    Some(false) => {
                        "encrypted (key will be generated on first secret write)".to_owned()
                    }
                    None => "encrypted".to_owned(),
                }
            } else {
                "plaintext (keyring unavailable)".to_owned()
            }
        }
    };

    json_output(&SecretsStatusJson {
        configured_mode: conf.secrets.mode,
        effective_mode,
        effective_mode_source: mode_source,
        keyring_available,
        keyring_error: keyring_error.clone(),
        master_key_present,
        writes: writes.clone(),
    })?;

    command_title!("Local secrets status");

    item!(
        "mode: {mode} ({source})",
        mode = effective_mode,
        source = mode_source
    );
    item!(
        "keyring: {status}",
        status = if keyring_available {
            "available".to_owned()
        } else {
            format!(
                "unavailable ({})",
                keyring_error.unwrap_or_else(|| "unknown error".to_owned())
            )
        }
    );
    if let Some(present) = master_key_present {
        item!(
            "master key: {}",
            if present { "present" } else { "missing" }
        );
    }
    item!("writes: {writes}");

    Ok(())
}
