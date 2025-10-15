use {
    crate::{
        command_title,
        display::json_output,
        item,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{
        self,
        events::{NexusEvent, NexusEventKind},
        idents::primitives,
        types::{NexusData, Storable, StorageConf, TypeName},
    },
};

/// Inspect a Nexus DAG execution process based on the provided object ID and
/// execution digest.
pub(crate) async fn inspect_dag_execution(
    dag_execution_id: sui::ObjectID,
    execution_digest: sui::TransactionDigest,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Nexus DAG Execution '{dag_execution_id}'");

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Nexus objects must be present in the configuration.
    let primitives_pkg_id = {
        let NexusObjects {
            primitives_pkg_id, ..
        } = &get_nexus_objects(&mut conf).await?;
        *primitives_pkg_id // ObjectID is Copy
    };

    // Build Sui client.
    let sui_conf = conf.sui.clone();
    let sui = build_sui_client(&sui_conf).await?;

    // Remote storage conf.
    let storage_conf = conf.data_storage.clone().into();

    // Check if we have authentication for potential decryption and get the session
    let session = get_active_session(&mut conf)?;

    let limit = None;
    let descending_order = false;

    // Starting cursor is the provided event digest and `event_seq` always 0.
    let mut cursor = Some(sui::EventID {
        tx_digest: execution_digest,
        event_seq: 0,
    });

    let mut json_trace = Vec::new();

    // Loop until we find an `ExecutionFinished` event.
    'query: loop {
        let query = sui::EventFilter::MoveEventModule {
            package: primitives_pkg_id,
            module: primitives::Event::EVENT_WRAPPER.module.into(),
        };

        let events = match sui
            .event_api()
            .query_events(query, cursor, limit, descending_order)
            .await
        {
            Ok(page) => {
                cursor = page.next_cursor;

                page.data
            }
            Err(_) => {
                // If RPC call fails, wait and retry.
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                continue;
            }
        };

        // Parse `SuiEvent` into `NexusEvent`.
        let events = events.into_iter().filter_map(|e| match e.try_into() {
            Ok(event) => Some::<NexusEvent>(event),
            Err(e) => {
                eprintln!("Failed to parse event: {:?}", e);
                None
            }
        });

        for event in events {
            match event.data {
                NexusEventKind::WalkAdvanced(e) if e.execution == dag_execution_id => {
                    notify_success!(
                        "Vertex '{vertex}' evaluated with output variant '{variant}'.",
                        vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                        variant = e.variant.name.truecolor(100, 100, 100),
                    );

                    let mut json_data = Vec::new();

                    for (port, data) in e.variant_ports_to_data.into_map() {
                        let (display_data, json_data_value) =
                            process_port_data(&port, data, &storage_conf, session).await?;

                        item!(
                            "Port '{port}' produced data: {data}",
                            port = port.name.truecolor(100, 100, 100),
                            data = display_data.truecolor(100, 100, 100),
                        );

                        json_data.push(json_data_value);
                    }

                    json_trace.push(json!({
                        "end_state": false,
                        "vertex": e.vertex,
                        "variant": e.variant.name,
                        "data": json_data,
                    }));
                }

                NexusEventKind::EndStateReached(e) if e.execution == dag_execution_id => {
                    notify_success!(
                        "{end_state} Vertex '{vertex}' evaluated with output variant '{variant}'.",
                        vertex = format!("{}", e.vertex).truecolor(100, 100, 100),
                        variant = e.variant.name.truecolor(100, 100, 100),
                        end_state = "END STATE".truecolor(100, 100, 100)
                    );

                    let mut json_data = Vec::new();

                    for (port, data) in e.variant_ports_to_data.into_map() {
                        let (display_data, json_data_value) =
                            process_port_data(&port, data, &storage_conf, session).await?;

                        item!(
                            "Port '{port}' produced data: {data}",
                            port = port.name.truecolor(100, 100, 100),
                            data = display_data.truecolor(100, 100, 100),
                        );

                        json_data.push(json_data_value);
                    }

                    json_trace.push(json!({
                        "end_state": true,
                        "vertex": e.vertex,
                        "variant": e.variant.name,
                        "data": json_data,
                    }));
                }

                NexusEventKind::ExecutionFinished(e) if e.execution == dag_execution_id => {
                    if e.has_any_walk_failed {
                        notify_error!("DAG execution finished unsuccessfully");

                        break 'query;
                    }

                    notify_success!("DAG execution finished successfully");

                    break 'query;
                }

                _ => {}
            }
        }
    }

    // Always save the updated config
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json_trace)?;

    Ok(())
}

/// Process port data, handling decryption if needed
async fn process_port_data(
    port: &TypeName,
    data: NexusData,
    storage_conf: &StorageConf,
    session: &mut nexus_sdk::crypto::session::Session,
) -> Result<(String, serde_json::Value), NexusCliError> {
    let data = data.fetch(&storage_conf, session).await.map_err(|e| {
        NexusCliError::Any(anyhow!(
            "Failed to fetch data for port '{port}': {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
            port = port.name,
            e = e,
            command = "$ nexus conf set --data-storage.walrus-aggregator-url <URL>",
            testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
        ))
    })?;

    Ok((
        format!("{data:?}"),
        json!({ "port": port.name, "data": data.as_json(), "was_encrypted": data.is_encrypted(), "storage": data.storage_kind() }),
    ))
}

/// Gets the active session for decryption
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

    /// Helper to create sender and receiver sessions for testing
    /// Returns (nexus_session, user_session) where:
    /// - nexus_session: represents the Nexus system that encrypts output data
    /// - user_session: represents the user inspecting execution and decrypting data
    fn create_test_sessions() -> (Session, Session) {
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

        (sender_sess, receiver_sess)
    }

    #[tokio::test]
    async fn test_process_port_data_plain_data() {
        let (mut _sender, mut receiver) = create_test_sessions();
        let storage_conf = StorageConf::default();

        let port = TypeName {
            name: "test_port".to_string(),
        };

        let test_data = json!({
            "message": "Hello, world!",
            "number": 42,
            "array": [1, 2, 3]
        });

        let nexus_data = NexusData::new_inline(test_data.clone());

        let result = process_port_data(&port, nexus_data, &storage_conf, &mut receiver).await;

        assert!(result.is_ok());
        let (display_data, json_result) = result.unwrap();

        // Check display format
        assert!(display_data.contains("Hello, world!"));
        assert!(display_data.contains("42"));

        // Check JSON result structure
        assert_eq!(json_result["port"], "test_port");
        assert_eq!(json_result["data"], test_data);
    }

    #[tokio::test]
    async fn test_process_port_data_encrypted_data() {
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let storage_conf = StorageConf::default();

        let port = TypeName {
            name: "encrypted_port".to_string(),
        };

        let original = json!({
            "secret": "top secret message",
            "value": 12345,
            "nested": {
                "field": "encrypted nested data"
            }
        });

        let mut encrypted = original.clone();

        // Encrypt the data using Nexus session (simulating system encrypting output)
        nexus_session
            .encrypt_nexus_data_json(&mut encrypted)
            .expect("Failed to encrypt test data");

        let nexus_data = NexusData::new_inline_encrypted(encrypted);

        let result = process_port_data(&port, nexus_data, &storage_conf, &mut user_session).await;

        assert!(result.is_ok());
        let (display_data, json_result) = result.unwrap();

        // Check that the decrypted data matches original
        assert!(display_data.contains("top secret message"));
        assert!(display_data.contains("12345"));

        // Check JSON result structure
        assert_eq!(json_result["port"], "encrypted_port");
        assert_eq!(json_result["data"], original);
    }

    #[tokio::test]
    async fn test_process_port_data_encrypted_simple_string() {
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let storage_conf = StorageConf::default();

        let port = TypeName {
            name: "string_port".to_string(),
        };

        let original = json!("simple encrypted string");
        let mut encrypted = original.clone();

        // Encrypt the data using Nexus session (simulating system encrypting output)
        nexus_session
            .encrypt_nexus_data_json(&mut encrypted)
            .expect("Failed to encrypt test data");

        let nexus_data = NexusData::new_inline_encrypted(encrypted);

        let result = process_port_data(&port, nexus_data, &storage_conf, &mut user_session).await;

        assert!(result.is_ok());
        let (display_data, json_result) = result.unwrap();

        // Check that the decrypted data matches original
        assert!(display_data.contains("simple encrypted string"));

        // Check JSON result structure
        assert_eq!(json_result["port"], "string_port");
        assert_eq!(json_result["data"], original);
    }

    #[tokio::test]
    async fn test_process_port_data_encrypted_complex_object() {
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let storage_conf = StorageConf::default();

        let port = TypeName {
            name: "complex_port".to_string(),
        };

        let original = json!({
            "user": {
                "id": 123,
                "name": "Alice",
                "email": "alice@example.com",
                "preferences": {
                    "theme": "dark",
                    "notifications": true,
                    "languages": ["en", "es", "fr"]
                }
            },
            "metadata": {
                "created": "2023-01-01T00:00:00Z",
                "tags": ["important", "encrypted"],
                "version": 1.2
            }
        });

        let mut encrypted = original.clone();

        // Encrypt the data using Nexus session (simulating system encrypting output)
        nexus_session
            .encrypt_nexus_data_json(&mut encrypted)
            .expect("Failed to encrypt test data");

        let nexus_data = NexusData::new_inline_encrypted(encrypted);

        let result = process_port_data(&port, nexus_data, &storage_conf, &mut user_session).await;

        assert!(result.is_ok());
        let (display_data, json_result) = result.unwrap();

        // Check that the decrypted data contains expected elements
        assert!(display_data.contains("Alice"));
        assert!(display_data.contains("alice@example.com"));
        assert!(display_data.contains("dark"));

        // Check JSON result structure
        assert_eq!(json_result["port"], "complex_port");
        assert_eq!(json_result["data"], original);
    }

    #[tokio::test]
    async fn test_process_port_data_malformed_encrypted_data() {
        let (_nexus_session, mut user_session) = create_test_sessions();
        let storage_conf = StorageConf::default();

        let port = TypeName {
            name: "bad_port".to_string(),
        };

        // Create malformed encrypted data (not properly serialized)
        let bad_encrypted_data = json!("this is not encrypted binary data");

        let nexus_data = NexusData::new_inline_encrypted(bad_encrypted_data);

        let result = process_port_data(&port, nexus_data, &storage_conf, &mut user_session).await;

        // Should fail because the data is not properly encrypted
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_encrypt_decrypt_roundtrips() {
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let storage_conf = StorageConf::default();

        let test_cases = vec![
            ("port1", json!("first message")),
            ("port2", json!({"num": 42})),
            ("port3", json!([1, 2, 3, 4, 5])),
            ("port4", json!(null)),
            ("port5", json!(true)),
        ];

        for (port_name, original) in test_cases {
            let port = TypeName {
                name: port_name.to_string(),
            };

            let mut encrypted = original.clone();

            nexus_session
                .encrypt_nexus_data_json(&mut encrypted)
                .expect("Failed to encrypt test data");

            let nexus_data = NexusData::new_inline_encrypted(encrypted);

            // Decrypt and verify using user session (simulating user inspecting execution)
            let result =
                process_port_data(&port, nexus_data, &storage_conf, &mut user_session).await;
            assert!(result.is_ok(), "Failed to process port {}", port_name);

            let (_display_data, json_result) = result.unwrap();
            assert_eq!(json_result["port"], port_name);
            assert_eq!(json_result["data"], original);
        }
    }
}
