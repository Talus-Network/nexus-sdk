use {
    crate::{
        command_title,
        dag::dag_inspect_execution::inspect_dag_execution,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    anyhow::anyhow,
    bincode,
    nexus_sdk::{
        crypto::session::{Message, Session},
        idents::workflow,
        transactions::dag,
    },
    serde_json::Value,
};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
pub(crate) async fn execute_dag(
    dag_id: sui::ObjectID,
    entry_group: String,
    mut input_json: serde_json::Value,
    encrypt: Vec<String>,
    inspect: bool,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Always validate authentication before proceeding
    // This validation is still required even if we dont encrypt anything at input ports
    validate_authentication(&conf)?;

    if !encrypt.is_empty() {
        // Get the active session and modify it (this advances the ratchet state)
        let session = get_active_session(&mut conf)?;

        encrypt_entry_ports_once(session, &mut input_json, &encrypt)?;

        // Save the updated config
        conf.save().await.map_err(NexusCliError::Any)?;
    }

    // Nexus objects must be present in the configuration.
    let objects = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Fetch DAG object for its ObjectRef.
    let dag = fetch_object_by_id(&sui, dag_id).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Err(e) = dag::execute(&mut tx, objects, &dag, &entry_group, input_json, encrypt) {
        tx_handle.error();

        return Err(NexusCliError::Any(e));
    }

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and send the TX.
    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    // We need to parse the DAGExecution object ID from the response.
    let dag = response
        .object_changes
        .unwrap_or_default()
        .into_iter()
        .find_map(|change| match change {
            sui::ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.address == *objects.workflow_pkg_id
                && object_type.module == workflow::Dag::DAG_EXECUTION.module.into()
                && object_type.name == workflow::Dag::DAG_EXECUTION.name.into() =>
            {
                Some(object_id)
            }
            _ => None,
        });

    let Some(object_id) = dag else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the DAGExecution object ID in the transaction response."
        )));
    };

    notify_success!(
        "DAGExecution object ID: {id}",
        id = object_id.to_string().truecolor(100, 100, 100)
    );

    if inspect {
        inspect_dag_execution(object_id, response.digest).await?;
    } else {
        json_output(&json!({ "digest": response.digest, "execution_id": object_id }))?;
    }

    Ok(())
}

fn encrypt_entry_ports_once(
    session: &mut Session,
    input: &mut Value,
    targets: &[String],
) -> Result<(), NexusCliError> {
    if targets.is_empty() {
        return Ok(()); // nothing to do, avoid ratchet advance
    }

    for handle in targets {
        let (vertex, port) = handle
            .split_once('.')
            .ok_or_else(|| NexusCliError::Any(anyhow!("Bad --encrypt handle: {handle}")))?;

        // Take the plaintext for ownership
        let slot = input
            .get_mut(vertex)
            .and_then(|v| v.get_mut(port))
            .ok_or_else(|| NexusCliError::Any(anyhow!("Input JSON has no {vertex}.{port}")))?;

        let plaintext = slot.take();
        let bytes = serde_json::to_vec(&plaintext).map_err(|e| NexusCliError::Any(anyhow!(e)))?;

        // Encrypt
        let msg = session
            .encrypt(&bytes)
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

        // Session must always return a Standard packet here
        let Message::Standard(pkt) = msg else {
            return Err(NexusCliError::Any(anyhow!(
                "Session returned non-standard packet"
            )));
        };

        // Serialize the StandardMessage with bincode
        let serialized = bincode::serialize(&pkt).map_err(|e| NexusCliError::Any(anyhow!(e)))?;
        *slot = serde_json::to_value(&serialized).map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    }

    // commit the session state
    session.commit_sender(None);

    Ok(())
}

/// Validates that the user has an active authentication session
fn validate_authentication(conf: &CliConf) -> Result<(), NexusCliError> {
    if conf.crypto.sessions.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "Authentication required — run `nexus crypto auth` first"
        )));
    }
    Ok(())
}

/// Gets the active session for encryption/decryption
fn get_active_session(
    conf: &mut CliConf,
) -> Result<&mut nexus_sdk::crypto::session::Session, NexusCliError> {
    if conf.crypto.sessions.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "Authentication required — run `nexus crypto auth` first"
        )));
    }

    let session_id = *conf.crypto.sessions.values().next().unwrap().id();
    conf.crypto
        .sessions
        .get_mut(&session_id)
        .ok_or_else(|| NexusCliError::Any(anyhow!("Session not found in config")))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::crypto::{
            session::{Message, Session},
            x3dh::{IdentityKey, PreKeyBundle},
        },
        serde_json::json,
    };

    /// Helper to create a mock session for testing
    fn create_mock_session() -> Session {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = IdentityKey::generate().secret().clone();
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        let (message, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"test").expect("Failed to initiate session");

        let initial_msg = match message {
            Message::Initial(msg) => msg,
            _ => panic!("Expected Initial message type"),
        };

        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, &initial_msg, None)
                .expect("Failed to receive session");

        // Exchange messages to establish the ratchet properly
        let setup_msg = sender_sess
            .encrypt(b"setup")
            .expect("Failed to encrypt setup message");
        let _ = receiver_sess
            .decrypt(&setup_msg)
            .expect("Failed to decrypt setup message");

        sender_sess
    }

    #[test]
    fn test_encrypt_entry_ports_once_empty_targets() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "test_value"
            }
        });
        let targets: Vec<String> = vec![];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());
        // Input should remain unchanged when no targets are specified
        assert_eq!(input["vertex1"]["port1"], "test_value");
    }

    #[test]
    fn test_encrypt_entry_ports_once_single_target() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "test_value",
                "port2": "other_value"
            }
        });
        let targets = vec!["vertex1.port1".to_string()];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // port1 should be encrypted (no longer the original string)
        assert_ne!(input["vertex1"]["port1"], "test_value");
        // port2 should remain unchanged
        assert_eq!(input["vertex1"]["port2"], "other_value");

        // The encrypted value should be a JSON array of bytes
        assert!(input["vertex1"]["port1"].is_array());
    }

    #[test]
    fn test_encrypt_entry_ports_once_multiple_targets() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "value1",
                "port2": "value2"
            },
            "vertex2": {
                "port3": "value3"
            }
        });
        let targets = vec![
            "vertex1.port1".to_string(),
            "vertex1.port2".to_string(),
            "vertex2.port3".to_string(),
        ];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // All targeted ports should be encrypted
        assert!(input["vertex1"]["port1"].is_array());
        assert!(input["vertex1"]["port2"].is_array());
        assert!(input["vertex2"]["port3"].is_array());

        // Original values should no longer be present
        assert_ne!(input["vertex1"]["port1"], "value1");
        assert_ne!(input["vertex1"]["port2"], "value2");
        assert_ne!(input["vertex2"]["port3"], "value3");
    }

    #[test]
    fn test_encrypt_entry_ports_once_bad_handle_format() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "test_value"
            }
        });
        let targets = vec!["invalid_handle_format".to_string()];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Bad --encrypt handle"));
    }

    #[test]
    fn test_encrypt_entry_ports_once_nonexistent_vertex() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "test_value"
            }
        });
        let targets = vec!["nonexistent_vertex.port1".to_string()];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Input JSON has no nonexistent_vertex.port1"));
    }

    #[test]
    fn test_encrypt_entry_ports_once_nonexistent_port() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "test_value"
            }
        });
        let targets = vec!["vertex1.nonexistent_port".to_string()];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Input JSON has no vertex1.nonexistent_port"));
    }

    #[test]
    fn test_encrypt_entry_ports_once_complex_data_types() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": {
                    "nested": {
                        "data": [1, 2, 3],
                        "string": "test"
                    }
                }
            }
        });
        let targets = vec!["vertex1.port1".to_string()];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // The complex JSON should be encrypted
        assert!(input["vertex1"]["port1"].is_array());
        // It should no longer contain the original nested structure
        assert!(!input["vertex1"]["port1"].get("nested").is_some());
    }

    #[test]
    fn test_encrypt_entry_ports_once_ratchet_advancement() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "value1",
                "port2": "value2"
            }
        });
        let targets = vec!["vertex1.port1".to_string(), "vertex1.port2".to_string()];

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // Both values should be encrypted, but since we advance the ratchet only once,
        // subsequent encryptions should use static encryption
        assert!(input["vertex1"]["port1"].is_array());
        assert!(input["vertex1"]["port2"].is_array());

        // The encrypted values should be different even for the same session
        // (due to nonces), but we can't easily test the ratchet state directly
        // without access to session internals
    }
}
