use crate::{
    command_title,
    loading,
    notify_success,
    prelude::*,
    secrets::{
        helpers,
        store::{master_key, policy},
    },
};

pub(crate) async fn secrets_enable(
    conf_path: PathBuf,
    crypto_conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Enabling at-rest encryption for local secrets");

    policy::set_mode_for_process(SecretsMode::Auto);

    // Ensure a persistent master key exists.
    let key_handle = loading!("Ensuring a master key exists...");
    match master_key::ensure_master_key_exists() {
        Ok(master_key::EnsureMasterKey::Created) => {
            key_handle.success();
            notify_success!("At-rest encryption enabled (master key created in keyring)");
        }
        Ok(master_key::EnsureMasterKey::AlreadyExists) => {
            key_handle.success();
            notify_success!("At-rest encryption already enabled (master key present in keyring)");
        }
        Err(e) => {
            key_handle.error();
            return Err(NexusCliError::Any(e.into()));
        }
    }

    // Persist the policy choice (and rewrite any stored secrets under the key).
    let mut conf = helpers::load_cli_conf(&conf_path).await;
    conf.secrets.mode = SecretsMode::Auto;
    conf.save_to_path(&conf_path)
        .await
        .map_err(|e| NexusCliError::Any(e.into()))?;

    // If a crypto config already exists, re-save it so any plaintext secrets are re-written as
    // encrypted values under the master key.
    if let Some(crypto_conf) = helpers::load_crypto_conf_if_exists(&crypto_conf_path).await? {
        let rewrite_handle = loading!("Rewriting crypto config with encryption...");

        crypto_conf
            .save_to_path(&crypto_conf_path)
            .await
            .map_err(|e| anyhow!("Failed to write {}: {e}", crypto_conf_path.display()))
            .map_err(NexusCliError::Any)?;

        rewrite_handle.success();
    }

    Ok(())
}
