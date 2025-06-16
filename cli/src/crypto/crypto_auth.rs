use {
    crate::{command_title, display::json_output, loading, prelude::*, sui::*},
    nexus_sdk::{
        crypto::{
            session::Session,
            x3dh::{IdentityKey, PreKeyBundle},
        },
        object_crawler::fetch_one,
        sui,
        transactions::crypto::*,
    },
};

// Temporary struct for fetching raw prekey data
#[derive(serde::Deserialize)]
struct RawPreKey {
    bytes: Vec<u8>,
}

pub(crate) async fn crypto_auth(gas: GasArgs) -> AnyResult<(), NexusCliError> {
    command_title!("Establishing a secure session with the network");

    // 1. Load config & objects
    let mut conf = CliConf::load().await.unwrap_or_default();
    let objects = &get_nexus_objects(&mut conf).await?;

    // 2. Wallet / client / address
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // 3. Gas coin selection
    let gas_coin = fetch_gas_coin(&sui, address, gas.sui_gas_coin).await?;
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // 4. Craft claim transaction
    let tx_handle = loading!("Crafting transaction...");
    let mut tx_builder = sui::ProgrammableTransactionBuilder::new();
    // Ignore the return value, it's probably empty
    if let Err(e) = claim_pre_key_for_self(&mut tx_builder, objects) {
        tx_handle.error();
        return Err(NexusCliError::Any(e));
    }
    let ptb = tx_builder.finish();
    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        ptb,
        gas.sui_gas_budget,
        reference_gas_price,
    );

    let tx_resp = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    // 5. Locate the newly‑created Prekey object in effects
    let effects = tx_resp
        .effects
        .ok_or_else(|| NexusCliError::Any(anyhow!("No effects found in response")))?;

    let prekey_obj_id = effects
        .unwrapped()
        .iter()
        .find_map(|object| {
            if object.owner.get_owner_address() == Ok(address) {
                return Some(object.object_id());
            }
            None
        })
        .ok_or_else(|| NexusCliError::Any(anyhow!("PreKey object ID not found")))?;

    // Fetch full object
    let fetch_handle = loading!("Fetching prekey object...");
    let prekey_resp =
        match fetch_one::<nexus_sdk::object_crawler::Structure<RawPreKey>>(&sui, prekey_obj_id)
            .await
        {
            Ok(response) => {
                fetch_handle.success();
                response
            }
            Err(e) => {
                fetch_handle.error();
                return Err(NexusCliError::Any(e));
            }
        };
    // Extract object reference before moving data
    let prekey_object_ref = prekey_resp.object_ref();
    let peer_bundle = bincode::deserialize::<PreKeyBundle>(&prekey_resp.data.into_inner().bytes)
        .map_err(|e| NexusCliError::Any(e.into()))?;

    // 6. Ensure IdentityKey
    // Ensure crypto config exists, initialize if needed
    let crypto_secret = conf
        .crypto
        .get_or_insert_with(|| Secret::new(CryptoConf::default()));

    crypto_secret
        .identity_key
        .get_or_insert_with(IdentityKey::generate);

    // 7. Run X3DH & store session
    let first_message = b"nexus auth";
    let (initial_msg, session) = {
        let identity_key = crypto_secret.identity_key.as_ref().unwrap();
        Session::initiate(identity_key, &peer_bundle, first_message)
            .map_err(|e| NexusCliError::Any(e.into()))?
    };

    // Extract InitialMessage from Message enum
    let initial_message = match initial_msg {
        nexus_sdk::crypto::session::Message::Initial(msg) => msg,
        _ => {
            return Err(NexusCliError::Any(anyhow!(
                "Expected Initial message from session initiation"
            )))
        }
    };

    // Store session and save config
    let session_id = *session.id();
    crypto_secret.sessions.insert(session_id, session);

    let save_handle = loading!("Saving session to configuration...");
    match conf.save().await {
        Ok(()) => {
            save_handle.success();
        }
        Err(e) => {
            save_handle.error();
            return Err(NexusCliError::Any(e));
        }
    }

    // Make borrow checker happy
    let objects = &get_nexus_objects(&mut conf).await?;

    // 8. Craft associate transaction
    let tx_handle = loading!("Crafting transaction...");
    let mut tx_builder = sui::ProgrammableTransactionBuilder::new();
    if let Err(e) = associate_pre_key_with_sender(
        &mut tx_builder,
        objects,
        &prekey_object_ref,
        initial_message.clone(),
    ) {
        tx_handle.error();
        return Err(NexusCliError::Any(e));
    }
    let ptb = tx_builder.finish();
    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        ptb,
        gas.sui_gas_budget,
        reference_gas_price,
    );

    let associate_tx_resp = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    // Output both transaction digests
    json_output(&json!({
        "claim_digest": tx_resp.digest,
        "associate_digest": associate_tx_resp.digest,
        "initial_message": initial_message,
    }))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        rand::rngs::OsRng,
        std::{collections::HashMap, env},
        tempfile::TempDir,
        x25519_dalek::StaticSecret,
    };

    #[tokio::test]
    #[serial_test::serial(crypto_auth_offline)]
    async fn test_crypto_auth_offline_session_persistence() {
        // Arrange
        // Isolate the filesystem & environment so the test is self-contained.
        let tmp = TempDir::new().expect("temp dir");
        let conf_path = tmp.path().join("conf.toml");

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

        // Build a CryptoConf with the freshly created session and wrap it in a Secret.
        let mut sessions = HashMap::new();
        sessions.insert(session_id, session);
        let crypto_conf = CryptoConf {
            identity_key: Some(sender_identity),
            sessions,
        };
        let secret_crypto = Secret::new(crypto_conf);

        // Compose full CLI configuration and persist to disk.
        let cli_conf = CliConf {
            sui: SuiConf::default(),
            nexus: None,
            tools: HashMap::new(),
            crypto: Some(secret_crypto),
        };

        let toml_str = toml::to_string(&cli_conf).expect("serialize conf to TOML");
        tokio::fs::write(&conf_path, toml_str)
            .await
            .expect("write conf.toml");

        // Reload and check that the session is still present & decrypts cleanly.
        let loaded_conf = CliConf::load_from_path(&conf_path)
            .await
            .expect("load conf");
        let loaded_crypto = loaded_conf.crypto.expect("crypto section");
        let saved_session = loaded_crypto
            .sessions
            .get(&session_id)
            .expect("session stored");

        // Basic sanity: session IDs match.
        assert_eq!(saved_session.id(), &session_id);

        // Clean-up env so other tests are unaffected.
        env::remove_var("XDG_CONFIG_HOME");
    }
}
