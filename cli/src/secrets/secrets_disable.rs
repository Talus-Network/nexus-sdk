use crate::{
    command_title,
    confirm,
    loading,
    notify_success,
    prelude::*,
    secrets::{
        helpers,
        store::{master_key, policy},
    },
};

pub(crate) async fn secrets_disable(
    skip_confirmation: bool,
    conf_path: PathBuf,
    crypto_conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Disabling at-rest encryption for local secrets");

    if !skip_confirmation {
        confirm!(
            "This will rewrite local secret state as plaintext and delete the master key from your keyring. Continue?"
        );
    }

    // Load configs first (may require the current key if encrypted), then rewrite with mode=off.
    policy::set_mode_for_process(SecretsMode::Require);
    let crypto_conf = helpers::load_crypto_conf_if_exists(&crypto_conf_path).await?;

    let mut conf = helpers::load_cli_conf(&conf_path).await;
    conf.secrets.mode = SecretsMode::Off;
    policy::set_mode_for_process(SecretsMode::Off);

    // Persist mode=off and rewrite any secrets as plaintext.
    conf.save_to_path(&conf_path)
        .await
        .map_err(|e| NexusCliError::Any(e.into()))?;

    if let Some(crypto_conf) = crypto_conf {
        let rewrite_handle = loading!("Rewriting crypto config as plaintext...");
        crypto_conf
            .save_to_path(&crypto_conf_path)
            .await
            .map_err(|e| anyhow!("Failed to write {}: {e}", crypto_conf_path.display()))
            .map_err(NexusCliError::Any)?;
        rewrite_handle.success();
    }

    // Delete master key if present.
    let delete_handle = loading!("Deleting master key from keyring...");
    match master_key::delete_master_key() {
        Ok(master_key::DeleteMasterKey::Deleted) => {
            delete_handle.success();
            notify_success!("Master key deleted from keyring");
        }
        Ok(master_key::DeleteMasterKey::NotFound) => {
            delete_handle.success();
        }
        Err(e) => {
            delete_handle.error();
            return Err(NexusCliError::Any(e.into()));
        }
    }

    notify_success!("At-rest encryption disabled (mode=off)");
    Ok(())
}
