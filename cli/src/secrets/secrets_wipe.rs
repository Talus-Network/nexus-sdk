use crate::{
    command_title,
    confirm,
    loading,
    notify_success,
    prelude::*,
    secrets::{helpers, store::master_key},
};

pub(crate) async fn secrets_wipe(
    skip_confirmation: bool,
    conf_path: PathBuf,
    crypto_conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Wiping local secret state");

    if !skip_confirmation {
        confirm!(
            "This will delete the master key and wipe local secret state (sessions, identity key). Continue?"
        );
    }

    // Persist mode=off so future runs don't attempt to use the deleted key.
    let mut conf = helpers::load_cli_conf(&conf_path).await;
    conf.secrets.mode = SecretsMode::Off;

    conf.save_to_path(&conf_path)
        .await
        .map_err(|e| NexusCliError::Any(e.into()))?;

    let wipe_handle = loading!("Removing local crypto config...");
    match tokio::fs::remove_file(&crypto_conf_path).await {
        Ok(()) => wipe_handle.success(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => wipe_handle.success(),
        Err(e) => {
            wipe_handle.error();
            return Err(NexusCliError::Any(anyhow!(
                "Failed to remove {}: {e}",
                crypto_conf_path.display()
            )));
        }
    }

    let delete_handle = loading!("Deleting master key from keyring...");
    match master_key::delete_master_key() {
        Ok(master_key::DeleteMasterKey::Deleted) => {
            delete_handle.success();
        }
        Ok(master_key::DeleteMasterKey::NotFound) => {
            delete_handle.success();
        }
        Err(e) => {
            delete_handle.error();
            return Err(NexusCliError::Any(e.into()));
        }
    }

    notify_success!("Local secret state wiped");
    Ok(())
}
