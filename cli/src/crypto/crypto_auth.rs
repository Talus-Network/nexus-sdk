use {
    crate::{command_title, display::json_output, loading, prelude::*, sui::*},
    nexus_sdk::crypto::x3dh::IdentityKey,
};

pub(crate) async fn crypto_auth(gas: GasArgs) -> AnyResult<(), NexusCliError> {
    command_title!("Establishing a secure session with the network");

    let (nexus_client, _) = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    // Fetch or create an identity key.
    if CryptoConf::get_identity_key(None).await.is_err() {
        CryptoConf::set_identity_key(IdentityKey::generate(), None)
            .await
            .map_err(NexusCliError::Any)?;
    }

    let ik = CryptoConf::get_identity_key(None)
        .await
        .map_err(NexusCliError::Any)?;

    // Perform the handshake.
    let handshake_handle = loading!("Establishing secure session...");

    let response = match nexus_client.crypto().handshake(&ik).await {
        Ok(hs) => hs,
        Err(e) => {
            handshake_handle.error();

            return Err(NexusCliError::Nexus(e));
        }
    };

    handshake_handle.success();

    // Store session and save config
    let save_handle = loading!("Saving session to configuration...");

    if let Err(e) = CryptoConf::insert_session(response.session, None).await {
        save_handle.error();

        return Err(NexusCliError::Any(e));
    }

    save_handle.success();

    // Output both transaction digests
    json_output(&json!({
        "claim_digest": response.claim_tx_digest,
        "associate_digest": response.associate_tx_digest,
    }))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::crypto::x3dh::PreKeyBundle,
        rand::rngs::OsRng,
        std::env,
        tempfile::TempDir,
        x25519_dalek::StaticSecret,
    };

    #[tokio::test]
    #[serial_test::serial(master_key_env)]
    async fn test_crypto_auth_offline_session_persistence() {
        // Arrange
        // Isolate the filesystem & environment so the test is self-contained.
        let tmp = TempDir::new().expect("temp dir");

        env::set_var("XDG_CONFIG_HOME", tmp.path());

        // Supply the master-key via environment variable.
        env::set_var("NEXUS_CLI_STORE_PASSPHRASE", "offline-test-passphrase");

        // Sanity-check that the master key can now be derived.
        crate::utils::secrets::master_key::get_master_key()
            .expect("master key should be available");

        // Generate Receiver (network side) pre-key material.
        let receiver_identity = IdentityKey::generate();
        let spk_secret = StaticSecret::random_from_rng(OsRng);
        let bundle = PreKeyBundle::new(&receiver_identity, 1, &spk_secret, None, None);

        // Generate Sender (CLI side) identity key.
        let sender_identity = IdentityKey::generate();

        // Run the X3DH Sender flow directly.
        let first_message = b"nexus auth";
        let (_initial_msg, session) =
            Session::initiate(&sender_identity, &bundle, first_message).expect("X3DH initiate");
        let session_id = *session.id();

        // insert session into CryptoConf and save to disk.
        CryptoConf::insert_session(session, None)
            .await
            .expect("Session should be saved");

        // Reload and check that the session is still present & decrypts cleanly.
        let saved_session = CryptoConf::get_active_session(None)
            .await
            .expect("Session should load");

        // Basic sanity: session IDs match.
        assert_eq!(saved_session.lock().await.id(), &session_id);

        // Clean-up env so other tests are unaffected.
        env::remove_var("XDG_CONFIG_HOME");
    }
}
