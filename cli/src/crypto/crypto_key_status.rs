use {
    crate::{
        command_title,
        loading,
        notify_success,
        prelude::*,
        utils::secrets::master_key::{SERVICE, USER},
    },
    keyring::Entry,
};

/// Show where the key was loaded from.
pub fn crypto_key_status() -> AnyResult<(), NexusCliError> {
    command_title!("Checking master key status");

    let check_handle = loading!("Checking key sources...");

    let status = if std::env::var("NEXUS_CLI_STORE_PASSPHRASE").is_ok() {
        "source: ENV var"
    } else if let Ok(_) = Entry::new(SERVICE, "passphrase")
        .map_err(|e| NexusCliError::Any(e.into()))?
        .get_password()
    {
        "source: key-ring pass-phrase"
    } else if let Ok(hex) = Entry::new(SERVICE, USER)
        .map_err(|e| NexusCliError::Any(e.into()))?
        .get_password()
    {
        check_handle.success();
        notify_success!("source: key-ring raw key ({:.8}…)", &hex[..8]);
        return Ok(());
    } else {
        check_handle.success();
        notify_success!("no persistent master key found");
        return Ok(());
    };

    check_handle.success();
    notify_success!("{}", status);
    Ok(())
}
