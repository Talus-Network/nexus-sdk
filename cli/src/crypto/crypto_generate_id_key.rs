use crate::{command_title, display::json_output, loading, notify_success, prelude::*};

/// Generate and store a fresh identity key in the Nexus CLI configuration.
/// WARNING: This will invalidate all existing sessions!
pub(crate) async fn crypto_generate_identity_key(
    conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    let mut conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_default();

    command_title!("Generating a fresh identity key");

    let conf_handle = loading!("Generating identity key...");

    // Ensure crypto config exists, initialize if needed
    if conf.crypto.is_none() {
        conf.crypto = Some(Secret::new(CryptoConf::default()));
    }

    // Generate a fresh identity key.
    if let Some(ref mut crypto_secret) = conf.crypto {
        crypto_secret.identity_key = Some(IdentityKey::generate());
        // wipe all sessions
        // TODO: think of something better than this
        crypto_secret.sessions.clear();
    }

    json_output(&serde_json::to_value(&conf).unwrap())?;

    match conf.save_to_path(&conf_path).await {
        Ok(()) => {
            conf_handle.success();
            notify_success!("Identity key generated successfully");
            notify_success!("All existing sessions have been invalidated");
            Ok(())
        }
        Err(e) => {
            conf_handle.error();
            Err(NexusCliError::Any(e))
        }
    }
}
