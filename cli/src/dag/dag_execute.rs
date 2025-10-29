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
        nexus::error::NexusError,
        object_crawler::{fetch_one, Structure, VecMap, VecSet},
        types::{hint_remote_fields, PortsData, StorageKind, TypeName},
    },
    std::sync::Arc,
};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
pub(crate) async fn execute_dag(
    dag_id: sui::ObjectID,
    entry_group: String,
    input_json: serde_json::Value,
    remote: Vec<String>,
    inspect: bool,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    let (nexus_client, sui) = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;

    // Build the remote storage conf.
    let conf = CliConf::load().await.unwrap_or_default();
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage.clone();
    let storage_conf = conf.data_storage.clone().into();

    // Get the active session for potential encryption
    let session = CryptoConf::get_active_session(None).await.map_err(|e|
        NexusCliError::Any(
            anyhow!(
                "Failed to get active session: {}.\nPlease initiate a session first.\n\n{init_key}\n{crypto_auth}",
                e,
                init_key = "$ nexus crypto init-key --force",
                crypto_auth = "$ nexus crypto auth"
            )
        )
    )?;

    // Fetch information about entry ports that need to be encrypted.
    let encrypt = fetch_encrypted_entry_ports(&sui, entry_group.clone(), &dag_id).await?;

    // Encrypt ports that need to be encrypted and store ports remote if they
    // need to be stored remotely.
    let input_data =
        process_entry_ports(&input_json, preferred_remote_storage, &encrypt, &remote).await?;

    let tx_handle = loading!("Crafting and executing transaction...");

    let result = match nexus_client
        .workflow()
        .execute(
            dag_id,
            input_data,
            Some(&entry_group),
            &storage_conf,
            Arc::clone(&session),
        )
        .await
    {
        Ok(r) => r,
        Err(NexusError::Storage(e)) => {
            tx_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "{e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                e = e,
                command = "$ nexus conf set --data-storage.walrus-publisher-url <URL> --data-storage.walrus-save-for-epochs <EPOCHS>",
                testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
            )));
        }
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Nexus(e));
        }
    };

    tx_handle.success();

    notify_success!(
        "DAGExecution object ID: {id}",
        id = result
            .execution_object_id
            .to_string()
            .truecolor(100, 100, 100)
    );

    // Update the session in the configuration.
    CryptoConf::release_session(session, None)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!("Failed to release session: {}", e)))?;

    if inspect {
        inspect_dag_execution(result.execution_object_id, result.tx_digest).await?;
    } else {
        json_output(
            &json!({ "digest": result.tx_digest, "execution_id": result.execution_object_id }),
        )?;
    }

    Ok(())
}

/// Process entry ports: encrypt and/or store remotely as needed. `input` must
/// be at least a 2-level JSON object (vertex -> port -> value).
async fn process_entry_ports(
    input: &serde_json::Value,
    preferred_remote_storage: Option<StorageKind>,
    encrypt: &HashMap<String, Vec<String>>,
    remote: &Vec<String>,
) -> Result<HashMap<String, PortsData>, NexusCliError> {
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
        let nexus_data_map = types::json_to_nexus_data_map(
            data,
            encrypt_fields.unwrap_or(&vec![]),
            &remote_fields,
            preferred_remote_storage.clone(),
        )
        .map_err(NexusCliError::Any)?;

        result.insert(vertex.clone(), PortsData::from_map(nexus_data_map));
    }

    // Hint the user if they should use remote storage and for what fields.
    let flattened = vertices
        .iter()
        .flat_map(|(vertex, ports)| {
            ports
                .as_object()
                .expect("Must be an object")
                .iter()
                .map(|(port, data)| (format!("{}.{}", vertex.clone(), port.clone()), data))
        })
        .collect::<HashMap<String, &serde_json::Value>>();

    let remote_hints = hint_remote_fields(&json!(flattened)).unwrap_or_default();

    if !remote_hints.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "Some input fields must be stored remotely to fit within transaction size limits. Please add the following argument to your command:\n\n{command} {fields}",
            command = "--remote",
            fields = remote_hints.join(",")
        )));
    }

    Ok(result)
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
        assert_matches::assert_matches,
        mockito::{Server, ServerGuard},
        nexus_sdk::{
            test_utils::nexus_mocks,
            types::{DataStorage, Storable, StorageConf},
            walrus::{BlobObject, BlobStorage, NewlyCreated, StorageInfo},
        },
        serde_json::json,
        std::collections::HashMap,
    };

    /// Setup mock server for Walrus testing
    async fn setup_mock_server_and_conf() -> anyhow::Result<(ServerGuard, StorageConf)> {
        // Create mock server
        let server = Server::new_async().await;
        let server_url = server.url();

        // Create a Walrus client that points to our mock server
        let storage_conf = StorageConf {
            walrus_publisher_url: Some(server_url.clone()),
            walrus_aggregator_url: Some(server_url),
            walrus_save_for_epochs: Some(2),
        };

        Ok((server, storage_conf))
    }

    #[tokio::test]
    async fn test_process_entry_ports_success_no_encrypt_no_remote() {
        let input = json!({
            "vertex1": {
                "port1": "value1",
                "port2": "value2"
            }
        });
        let (_, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let (session, _) = nexus_mocks::mock_session();
        let encrypt = HashMap::new();
        let remote = vec![];

        let result = process_entry_ports(&input, None, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result
            .get("vertex1")
            .expect("vertex1 missing")
            .clone()
            .commit_all(&storage_conf, Arc::clone(&session))
            .await
            .expect("commit_all failed");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        // Both should be Inline and not encrypted
        assert_matches!(port1, DataStorage::Inline(_));
        assert_eq!(port1.as_json(), &json!("value1"));
        assert!(!port1.is_encrypted());

        assert_matches!(port2, DataStorage::Inline(_));
        assert_eq!(port2.as_json(), &json!("value2"));
        assert!(!port2.is_encrypted());
    }

    #[tokio::test]
    async fn test_process_entry_ports_encrypt_only() {
        let input = json!({
            "vertex1": {
                "port1": "secret_value",
                "port2": "plain_value"
            }
        });
        let (_, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let (session, _) = nexus_mocks::mock_session();
        let mut encrypt = HashMap::new();
        encrypt.insert("vertex1".to_string(), vec!["port1".to_string()]);
        let remote = vec![];

        let result = process_entry_ports(&input, None, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result
            .get("vertex1")
            .expect("vertex1 missing")
            .clone()
            .commit_all(&storage_conf, Arc::clone(&session))
            .await
            .expect("commit_all failed");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        // port1 should be encrypted
        assert!(port1.is_encrypted());
        // port2 should not be encrypted
        assert!(!port2.is_encrypted());
    }

    #[tokio::test]
    async fn test_process_entry_ports_remote_only() {
        let input = json!({
            "vertex1": {
                "port1": "remote_value",
                "port2": "local_value"
            }
        });
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let (session, _) = nexus_mocks::mock_session();
        let encrypt = HashMap::new();
        let remote = vec!["vertex1.port1".to_string()];

        // Setup mock Walrus response
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
            .with_body(serde_json::to_string(&mock_put_response).expect("Must serialize"))
            .create_async()
            .await;

        let result = process_entry_ports(&input, None, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result
            .get("vertex1")
            .expect("vertex1 missing")
            .clone()
            .commit_all(&storage_conf, Arc::clone(&session))
            .await
            .expect("commit_all failed");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        // port1 should be walrus
        assert_matches!(port1, DataStorage::Walrus(_));
        // port2 should be Inline
        assert_matches!(port2, DataStorage::Inline(_));

        // Verify the request was made
        mock_put.assert_async().await;
    }

    #[tokio::test]
    async fn test_process_entry_ports_encrypt_and_remote() {
        let input = json!({
            "vertex1": {
                "port1": "secret_remote_value",
                "port2": "plain_local_value"
            }
        });
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let (session, _) = nexus_mocks::mock_session();
        let mut encrypt = HashMap::new();
        encrypt.insert("vertex1".to_string(), vec!["port1".to_string()]);
        let remote = vec!["vertex1.port1".to_string()];

        // Setup mock Walrus response
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
            .with_body(serde_json::to_string(&mock_put_response).expect("Must serialize"))
            .create_async()
            .await;

        let result = process_entry_ports(&input, None, &encrypt, &remote)
            .await
            .expect("Should succeed");

        let vertex = result
            .get("vertex1")
            .expect("vertex1 missing")
            .clone()
            .commit_all(&storage_conf, Arc::clone(&session))
            .await
            .expect("commit_all failed");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        // port1 should be encrypted and walrus
        assert!(port1.is_encrypted());
        assert_matches!(port1, DataStorage::Walrus(_));
        // port2 should be Inline and not encrypted
        assert_matches!(port2, DataStorage::Inline(_));
        assert!(!port2.is_encrypted());

        // Verify the request was made
        mock_put.assert_async().await;
    }

    #[tokio::test]
    async fn test_process_entry_ports_invalid_input_not_object() {
        let input = json!("not_an_object");
        let encrypt = HashMap::new();
        let remote = vec![];

        let result = process_entry_ports(&input, None, &encrypt, &remote).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Input JSON must be an object"));
    }

    #[tokio::test]
    async fn test_process_entry_ports_invalid_vertex_not_object() {
        let input = json!({
            "vertex1": "not_an_object"
        });
        let encrypt = HashMap::new();
        let remote = vec![];

        let result = process_entry_ports(&input, None, &encrypt, &remote).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("must be an object with port names as keys"));
    }
}
