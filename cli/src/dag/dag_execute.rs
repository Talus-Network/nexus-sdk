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
    nexus_sdk::{
        crypto::session::Session,
        idents::workflow,
        object_crawler::{fetch_one, Structure, VecMap, VecSet},
        transactions::dag,
        types::TypeName,
    },
    serde_json::Value,
};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
pub(crate) async fn execute_dag(
    dag_id: sui::ObjectID,
    entry_group: String,
    mut input_json: serde_json::Value,
    inspect: bool,
    gas_price: u64,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Nexus objects must be present in the configuration.
    let objects = &get_nexus_objects(&mut conf).await?;

    // Get the active session for potential encryption
    let session = get_active_session(&mut conf)?;

    // Fetch information about entry ports that need to be encrypted.
    let encrypt = fetch_encrypted_entry_ports(&sui, entry_group.clone(), &dag_id).await?;

    if !encrypt.is_empty() {
        encrypt_entry_ports_once(session, &mut input_json, &encrypt)?;
    }

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Fetch DAG object for its ObjectRef.
    let dag = fetch_object_by_id(&sui, dag_id).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // TODO: Add gas tool
    if let Err(e) = dag::execute(
        &mut tx,
        objects,
        &dag,
        gas_price,
        &entry_group,
        input_json,
        &encrypt,
    ) {
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

    // Always save the updated config
    conf.save().await.map_err(NexusCliError::Any)?;

    Ok(())
}

fn encrypt_entry_ports_once(
    session: &mut Session,
    input: &mut Value,
    targets: &HashMap<String, Vec<String>>,
) -> Result<(), NexusCliError> {
    if targets.is_empty() {
        return Ok(()); // nothing to do, avoid ratchet advance
    }

    for (vertex, ports) in targets {
        for port in ports {
            let data = input
                .get_mut(&vertex)
                .and_then(|v| v.get_mut(&port))
                .ok_or_else(|| NexusCliError::Any(anyhow!("Input JSON has no {vertex}.{port}")))?;

            session
                .encrypt_nexus_data_json(data)
                .map_err(NexusCliError::Any)?;
        }
    }

    // commit the session state
    session.commit_sender(None);

    Ok(())
}

/// Gets the active session for encryption/decryption
fn get_active_session(
    conf: &mut CliConf,
) -> Result<&mut nexus_sdk::crypto::session::Session, NexusCliError> {
    match &mut conf.crypto {
        Some(crypto_secret) => {
            if crypto_secret.sessions.is_empty() {
                return Err(NexusCliError::Any(anyhow!(
                    "Authentication required — run `nexus crypto auth` first"
                )));
            }

            let session_id = *crypto_secret.sessions.values().next().unwrap().id();
            crypto_secret
                .sessions
                .get_mut(&session_id)
                .ok_or_else(|| NexusCliError::Any(anyhow!("Session not found in config")))
        }
        None => Err(NexusCliError::Any(anyhow!(
            "Authentication required — run `nexus crypto auth` first"
        ))),
    }
}

/// Fetches the encrypted entry ports for a DAG.
async fn fetch_encrypted_entry_ports(
    sui: &sui::Client,
    entry_group: String,
    dag_id: &sui::ObjectID,
) -> AnyResult<HashMap<String, Vec<String>>, NexusCliError> {
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
    struct EntryPort {
        name: String,
        encrypted: bool,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct Dag {
        entry_groups:
            VecMap<Structure<TypeName>, VecMap<Structure<TypeName>, VecSet<Structure<EntryPort>>>>,
    }

    let result = fetch_one::<Structure<Dag>>(sui, *dag_id)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    // Get the relevant entry group.
    let group: TypeName = TypeName {
        name: entry_group.clone(),
    };

    let entry_group = result
        .data
        .into_inner()
        .entry_groups
        .into_inner()
        .remove(&group.into())
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!("Entry group '{entry_group}' not found in DAG"))
        })?;

    // Collapse into a more readable format.
    Ok(entry_group
        .into_inner()
        .into_iter()
        .filter_map(|(vertex, ports)| {
            let encrypted_ports: Vec<String> = ports
                .into_inner()
                .into_iter()
                .filter_map(|port| {
                    let port = port.into_inner();

                    if port.encrypted {
                        Some(port.name)
                    } else {
                        None
                    }
                })
                .collect();

            if encrypted_ports.is_empty() {
                return None; // Skip vertices with no encrypted ports
            }

            Some((vertex.into_inner().name, encrypted_ports))
        })
        .collect::<HashMap<_, _>>())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::crypto::{
            session::{Message, Session, StandardMessage},
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
        let targets = HashMap::new();

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
        let targets = HashMap::from([("vertex1".to_string(), vec!["port1".to_string()])]);

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // port1 should be encrypted (no longer the original string)
        assert_ne!(input["vertex1"]["port1"], "test_value");
        // port2 should remain unchanged
        assert_eq!(input["vertex1"]["port2"], "other_value");

        // The value should be encrypted and serialized as a `StandardMessage`
        let msg = serde_json::from_value::<StandardMessage>(input["vertex1"]["port1"].take());
        assert!(msg.is_ok(), "Failed to deserialize StandardMessage");
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

        let targets = HashMap::from([
            (
                "vertex1".to_string(),
                vec!["port1".to_string(), "port2".to_string()],
            ),
            ("vertex2".to_string(), vec!["port3".to_string()]),
        ]);

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // All targeted ports should be encrypted and serialized as `StandardMessage`
        let msg1 = serde_json::from_value::<StandardMessage>(input["vertex1"]["port1"].take());
        let msg2 = serde_json::from_value::<StandardMessage>(input["vertex1"]["port2"].take());
        let msg3 = serde_json::from_value::<StandardMessage>(input["vertex2"]["port3"].take());

        assert!(msg1.is_ok(), "Failed to deserialize StandardMessage");
        assert!(msg2.is_ok(), "Failed to deserialize StandardMessage");
        assert!(msg3.is_ok(), "Failed to deserialize StandardMessage");
    }

    #[test]
    fn test_encrypt_entry_ports_once_bad_handle_format() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "test_value"
            }
        });
        let targets = HashMap::from([(
            "vertex1".to_string(),
            vec!["invalid_handle_format".to_string()],
        )]);

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Input JSON has no vertex1.invalid_handle_format"));
    }

    #[test]
    fn test_encrypt_entry_ports_once_nonexistent_vertex() {
        let mut session = create_mock_session();
        let mut input = json!({
            "vertex1": {
                "port1": "test_value"
            }
        });
        let targets =
            HashMap::from([("nonexistent_vertex".to_string(), vec!["port1".to_string()])]);

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
        let targets =
            HashMap::from([("vertex1".to_string(), vec!["nonexistent_port".to_string()])]);

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
        let targets = HashMap::from([("vertex1".to_string(), vec!["port1".to_string()])]);

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // The complex JSON should be encrypted and serialized as a `StandardMessage`
        let msg = serde_json::from_value::<StandardMessage>(input["vertex1"]["port1"].take());
        assert!(msg.is_ok(), "Failed to deserialize StandardMessage");
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
        let targets = HashMap::from([(
            "vertex1".to_string(),
            vec!["port1".to_string(), "port2".to_string()],
        )]);

        let result = encrypt_entry_ports_once(&mut session, &mut input, &targets);

        assert!(result.is_ok());

        // Both values should be encrypted and serialized as a `StandardMessage`
        let msg1 = serde_json::from_value::<StandardMessage>(input["vertex1"]["port1"].take());
        let msg2 = serde_json::from_value::<StandardMessage>(input["vertex1"]["port2"].take());

        assert!(msg1.is_ok(), "Failed to deserialize StandardMessage");
        assert!(msg2.is_ok(), "Failed to deserialize StandardMessage");

        // The encrypted values should be different even for the same session
        // (due to nonces), but we can't easily test the ratchet state directly
        // without access to session internals
    }
}
