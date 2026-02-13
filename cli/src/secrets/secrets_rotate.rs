use {
    crate::{
        command_title,
        confirm,
        loading,
        notify_success,
        prelude::*,
        secrets::{
            helpers,
            store::{
                master_key::{KEY_LEN, SERVICE, USER},
                policy,
            },
        },
    },
    keyring::Entry,
    rand::{rngs::OsRng, RngCore},
};

pub(crate) async fn secrets_rotate(
    skip_confirmation: bool,
    conf_path: PathBuf,
    crypto_conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Rotating master key for local secrets");

    if !skip_confirmation {
        confirm!(
            "This will rotate the master key and re-encrypt locally stored secret state. If interrupted, you may need to run `nexus secrets wipe`. Continue?"
        );
    }

    let conf = helpers::load_cli_conf(&conf_path).await;
    if conf.secrets.mode == SecretsMode::Off {
        return Err(NexusCliError::Any(anyhow!(
            "At-rest encryption is disabled (mode=off). Run `nexus secrets enable` first."
        )));
    }

    // Read the current key so we can restore it on failure.
    let entry = Entry::new(SERVICE, USER).map_err(|e| NexusCliError::Any(e.into()))?;
    let old_hex = entry.get_password().map_err(|e| {
        NexusCliError::Any(anyhow!(
            "No master key found or keyring unavailable: {e}.\nRun `nexus secrets enable` to create one."
        ))
    })?;

    // Load the existing crypto config using the current key (if it exists).
    policy::set_mode_for_process(SecretsMode::Require);
    let crypto_conf = helpers::load_crypto_conf_if_exists(&crypto_conf_path).await?;

    // Generate a fresh 32-byte key.
    let mut new_key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut new_key);
    let new_hex = hex::encode(new_key);

    // Swap the key in the keyring, then re-encrypt configs.
    let rotate_handle = loading!("Updating keyring and re-encrypting configs...");

    if let Err(e) = entry.set_password(&new_hex) {
        rotate_handle.error();
        return Err(NexusCliError::Any(e.into()));
    }

    if let Some(crypto_conf) = crypto_conf {
        if let Err(e) = crypto_conf.save_to_path(&crypto_conf_path).await {
            // Best-effort restore of the previous key so the existing data remains decryptable.
            let _ = entry.set_password(&old_hex);
            let _ = crypto_conf.save_to_path(&crypto_conf_path).await;

            rotate_handle.error();
            return Err(NexusCliError::Any(anyhow!(
                "Failed to write {}: {e}",
                crypto_conf_path.display()
            )));
        }
    }

    rotate_handle.success();
    notify_success!("Master key rotated and local secrets re-encrypted");

    Ok(())
}
