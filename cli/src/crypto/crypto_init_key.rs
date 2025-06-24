use {
    crate::{
        command_title,
        loading,
        notify_success,
        prelude::*,
        utils::secrets::master_key::{MasterKeyError, KEY_LEN, SERVICE, USER},
    },
    keyring::Entry,
    rand::{rngs::OsRng, RngCore},
};

/// Generate and store a new 32-byte key in the OS key-ring.
pub async fn crypto_init_key(force: bool) -> AnyResult<(), NexusCliError> {
    command_title!("Generating and storing a new 32-byte master key");

    // 1. Abort if any persistent key already exists (unless --force)
    let check_handle = loading!("Checking for existing keys...");

    if (Entry::new(SERVICE, "passphrase")
        .map_err(|e| NexusCliError::Any(e.into()))?
        .get_password()
        .is_ok()
        || Entry::new(SERVICE, USER)
            .map_err(|e| NexusCliError::Any(e.into()))?
            .get_password()
            .is_ok())
        && !force
    {
        check_handle.error();
        return Err(NexusCliError::Any(MasterKeyError::KeyAlreadyExists.into()));
    }

    check_handle.success();

    // 2. Generate and store a new 32-byte key
    let generate_handle = loading!("Generating and storing master key...");

    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);

    match Entry::new(SERVICE, USER)
        .map_err(|e| NexusCliError::Any(e.into()))?
        .set_password(&hex::encode(key))
    {
        Ok(()) => {
            generate_handle.success();
            // Remove any stale pass-phrase entry so that key-status reports the new raw key.
            let _ = Entry::new(SERVICE, "passphrase").and_then(|e| e.delete_credential());
            notify_success!("32-byte master key saved to the OS key-ring");
            Ok(())
        }
        Err(e) => {
            generate_handle.error();
            Err(NexusCliError::Any(e.into()))
        }
    }
}
