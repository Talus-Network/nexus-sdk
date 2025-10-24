use {
    crate::{error::NexusCliError, prelude::AnyResult},
    anyhow::anyhow,
    nexus_sdk::{
        crypto::session::Session,
        object_crawler::{fetch_one, Structure, VecMap, VecSet},
        sui,
        types::{
            hint_remote_fields,
            json_to_nexus_data_map,
            DataStorage,
            PortsData,
            Storable,
            StorageConf,
            StorageKind,
            TypeName,
        },
    },
    serde::Deserialize,
    serde_json::{json, Value},
    std::{collections::HashMap, sync::Arc},
    tokio::sync::Mutex,
};

pub(crate) async fn fetch_encrypted_entry_ports(
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

    let key = TypeName {
        name: entry_group.clone(),
    };

    let entry_group = result
        .data
        .into_inner()
        .entry_groups
        .into_inner()
        .remove(&key.into())
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!("Entry group '{entry_group}' not found in DAG"))
        })?;

    Ok(entry_group
        .into_inner()
        .into_iter()
        .filter_map(|(vertex, ports)| {
            let encrypted_ports: Vec<String> = ports
                .into_inner()
                .into_iter()
                .filter_map(|port| {
                    let port = port.into_inner();
                    port.encrypted.then_some(port.name)
                })
                .collect();

            (!encrypted_ports.is_empty()).then_some((vertex.into_inner().name, encrypted_ports))
        })
        .collect())
}

/// Process entry ports: encrypt and/or store remotely as needed. `input` must
/// be at least a 2-level JSON object (vertex -> port -> value).
pub(crate) async fn process_entry_ports(
    input: &Value,
    storage_conf: &StorageConf,
    preferred_remote_storage: Option<StorageKind>,
    session: Arc<Mutex<Session>>,
    encrypt: &HashMap<String, Vec<String>>,
    remote: &Vec<String>,
) -> Result<HashMap<String, HashMap<String, DataStorage>>, NexusCliError> {
    let Some(vertices) = input.as_object() else {
        return Err(NexusCliError::Any(anyhow!(
            "Input JSON must be an object with vertex names as keys."
        )));
    };

    let mut result = HashMap::new();

    for (vertex, data) in vertices {
        let Some(ports) = data.as_object() else {
            return Err(NexusCliError::Any(anyhow!(
                "Input JSON for vertex '{vertex}' must be an object with port names as keys."
            )));
        };

        // Figure out which ports need to be encrypted and stored remotely for
        // this vertex.
        let encrypt_fields = encrypt.get(vertex);
        let remote_fields = ports
            .iter()
            .filter_map(|(port, _)| {
                let handle = format!("{vertex}.{port}");
                if remote.contains(&handle) {
                    Some(port.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Convert this json into a map of port -> NexusData.
        let nexus_data_map = json_to_nexus_data_map(
            data,
            encrypt_fields.unwrap_or(&vec![]),
            &remote_fields,
            preferred_remote_storage.clone(),
        )
        .map_err(NexusCliError::Any)?;

        // Commit each field - meaning it will get encrypted if necessary, and
        // uploaded to remote storage if necessary.
        let ports_data = PortsData::from_map(nexus_data_map)
            .commit_all(storage_conf, Arc::clone(&session))
            .await
            .map_err(|e| {
                NexusCliError::Any(anyhow!(
                    "Failed to store data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                    e = e,
                    command = "$ nexus conf set --data-storage.walrus-publisher-url <URL> --data-storage.walrus-save-for-epochs <EPOCHS>",
                    testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                ))
            })?;

        result.insert(vertex.clone(), ports_data);
    }

    // Hint the user if they should use remote storage and for what fields.
    let flattened = result
        .iter()
        .flat_map(|(vertex, ports)| {
            ports.iter().map(|(port, data)| {
                (
                    format!("{}.{}", vertex.clone(), port.clone()),
                    data.as_json(),
                )
            })
        })
        .collect::<HashMap<String, &Value>>();

    let remote_hints = hint_remote_fields(&json!(flattened)).unwrap_or_default();

    if !remote_hints.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "Some input fields must be stored remotely to fit within transaction size limits. Please add the following argument to your command:\n\n{command} {fields}",
            command = "--remote",
            fields = remote_hints.join(",")
        )));
    }

    // Advance the ratchet if we need to.
    if !encrypt.is_empty() {
        session.lock().await.commit_sender(None);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        assert_matches::assert_matches,
        mockito::{Server, ServerGuard},
        nexus_sdk::{
            crypto::{
                session::{Message, Session},
                x3dh::{IdentityKey, PreKeyBundle},
            },
            types::Storable,
            walrus::{BlobObject, BlobStorage, NewlyCreated, StorageInfo},
        },
        serde_json::json,
        std::collections::HashMap,
    };

    fn create_mock_session() -> Arc<Mutex<Session>> {
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

        let setup_msg = sender_sess
            .encrypt(b"setup")
            .expect("Failed to encrypt setup message");
        let _ = receiver_sess
            .decrypt(&setup_msg)
            .expect("Failed to decrypt setup message");

        Arc::new(Mutex::new(sender_sess))
    }

    async fn setup_mock_server_and_conf() -> anyhow::Result<(ServerGuard, StorageConf)> {
        let server = Server::new_async().await;
        let server_url = server.url();

        let storage_conf = StorageConf {
            walrus_publisher_url: Some(server_url.clone()),
            walrus_aggregator_url: Some(server_url),
            walrus_save_for_epochs: Some(2),
        };

        Ok((server, storage_conf))
    }

    #[tokio::test]
    async fn process_entry_ports_success_no_encrypt_no_remote() {
        let input = json!({
            "vertex1": {
                "port1": "value1",
                "port2": "value2"
            }
        });
        let (_, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let session = create_mock_session();
        let encrypt = HashMap::new();
        let remote = vec![];

        let result = process_entry_ports(&input, &storage_conf, None, session, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result.get("vertex1").expect("vertex1 missing");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert_matches!(port1, DataStorage::Inline(_));
        assert_eq!(port1.as_json(), &json!("value1"));
        assert!(!port1.is_encrypted());

        assert_matches!(port2, DataStorage::Inline(_));
        assert_eq!(port2.as_json(), &json!("value2"));
        assert!(!port2.is_encrypted());
    }

    #[tokio::test]
    async fn process_entry_ports_encrypt_only() {
        let input = json!({
            "vertex1": {
                "port1": "secret_value",
                "port2": "plain_value"
            }
        });
        let (_, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let session = create_mock_session();
        let mut encrypt = HashMap::new();
        encrypt.insert("vertex1".to_string(), vec!["port1".to_string()]);
        let remote = vec![];

        let result = process_entry_ports(&input, &storage_conf, None, session, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result.get("vertex1").expect("vertex1 missing");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert!(port1.is_encrypted());
        assert!(!port2.is_encrypted());
    }

    #[tokio::test]
    async fn process_entry_ports_remote_only() {
        let input = json!({
            "vertex1": {
                "port1": "value1",
                "port2": "value2"
            }
        });
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let session = create_mock_session();
        let encrypt = HashMap::new();
        let remote = vec!["vertex1.port1".to_string()];

        let mock_put_response = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: "json_blob_id".to_string(),
                    id: "json_object_id".to_string(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };

        let mock_put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_put_response).expect("serialize"))
            .create_async()
            .await;

        let result = process_entry_ports(&input, &storage_conf, None, session, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result.get("vertex1").expect("vertex1 missing");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert_matches!(port1, DataStorage::Walrus(_));
        assert_matches!(port2, DataStorage::Inline(_));

        mock_put.assert_async().await;
    }

    #[tokio::test]
    async fn process_entry_ports_encrypt_and_remote() {
        let input = json!({
            "vertex1": {
                "port1": "secret_remote_value",
                "port2": "plain_local_value"
            }
        });
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let session = create_mock_session();
        let mut encrypt = HashMap::new();
        encrypt.insert("vertex1".to_string(), vec!["port1".to_string()]);
        let remote = vec!["vertex1.port1".to_string()];

        let mock_put_response = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: "json_blob_id".to_string(),
                    id: "json_object_id".to_string(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };

        let mock_put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_put_response).expect("serialize"))
            .create_async()
            .await;

        let result = process_entry_ports(&input, &storage_conf, None, session, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result.get("vertex1").expect("vertex1 missing");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert!(port1.is_encrypted());
        assert_matches!(port1, DataStorage::Walrus(_));
        assert_matches!(port2, DataStorage::Inline(_));
        assert!(!port2.is_encrypted());

        mock_put.assert_async().await;
    }

    #[tokio::test]
    async fn process_entry_ports_invalid_input_not_object() {
        let input = json!("not_an_object");
        let (_, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let session = create_mock_session();
        let encrypt = HashMap::new();
        let remote = vec![];

        let result =
            process_entry_ports(&input, &storage_conf, None, session, &encrypt, &remote).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Input JSON must be an object"));
    }

    #[tokio::test]
    async fn process_entry_ports_invalid_vertex_not_object() {
        let input = json!({
            "vertex1": "not_an_object"
        });
        let (_, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let session = create_mock_session();
        let encrypt = HashMap::new();
        let remote = vec![];

        let result =
            process_entry_ports(&input, &storage_conf, None, session, &encrypt, &remote).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("must be an object with port names"));
    }
}
