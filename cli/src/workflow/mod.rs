use {
    crate::prelude::*,
    anyhow::anyhow,
    nexus_sdk::types::{hint_remote_fields, json_to_nexus_data_map, PortsData, StorageKind},
    serde_json::Value,
    std::collections::{HashMap, HashSet},
};

/// Process entry ports: store remotely as needed. `input` must be at least
/// a 2-level JSON object (vertex -> port -> value).
pub(crate) async fn process_entry_ports(
    input: &Value,
    preferred_remote_storage: Option<StorageKind>,
    remote: &[String],
) -> Result<HashMap<String, PortsData>, NexusCliError> {
    let Some(vertices) = input.as_object() else {
        return Err(NexusCliError::Any(anyhow!(
            "Input JSON must be an object with vertex names as keys."
        )));
    };

    let remote_handles: HashSet<String> = remote.iter().cloned().collect();
    let mut result = HashMap::new();

    for (vertex, data) in vertices {
        let Some(ports) = data.as_object() else {
            return Err(NexusCliError::Any(anyhow!(
                "Input JSON for vertex '{vertex}' must be an object with port names as keys."
            )));
        };

        // Figure out which ports need to be stored remotely for this vertex.
        let remote_fields = ports
            .iter()
            .filter_map(|(port, _)| {
                let handle = format!("{vertex}.{port}");
                remote_handles.contains(&handle).then_some(port.clone())
            })
            .collect::<Vec<_>>();

        // Convert this json into a map of port -> NexusData.
        let nexus_data_map = json_to_nexus_data_map(data, &remote_fields, preferred_remote_storage)
            .map_err(NexusCliError::Any)?;

        result.insert(vertex.clone(), PortsData::from_map(nexus_data_map));
    }

    // Hint the user if they should use remote storage and for what fields.
    let mut flattened: serde_json::Map<String, Value> = serde_json::Map::new();
    for (vertex, ports) in vertices {
        let ports = ports
            .as_object()
            .expect("Input JSON for vertex should already be validated as an object");
        for (port, data) in ports {
            let handle = format!("{vertex}.{port}");
            if remote_handles.contains(&handle) {
                continue;
            }
            flattened.insert(handle, data.clone());
        }
    }

    let flattened_json = Value::Object(flattened);
    let remote_hints = hint_remote_fields(&flattened_json).unwrap_or_default();

    if !remote_hints.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "Some input fields must be stored remotely to fit within transaction size limits. Please add the following argument to your command:\n\n{command} {fields}",
            command = "--remote",
            fields = remote_hints.join(",")
        )));
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
            types::{DataStorage, Storable, StorageConf},
            walrus::{BlobObject, BlobStorage, NewlyCreated, StorageInfo},
        },
        serde_json::json,
    };

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
    async fn process_entry_ports_success_no_remote() {
        let input = json!({
            "vertex1": {
                "port1": "value1",
                "port2": "value2"
            }
        });
        let (_, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let remote = vec![];

        let result = process_entry_ports(&input, None, &remote)
            .await
            .expect("Should succeed");

        let vertex = result
            .get("vertex1")
            .expect("vertex1 missing")
            .clone()
            .commit_all(&storage_conf)
            .await
            .expect("commit_all failed");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert_matches!(port1, DataStorage::Inline(_));
        assert_eq!(port1.as_json(), &json!("value1"));

        assert_matches!(port2, DataStorage::Inline(_));
        assert_eq!(port2.as_json(), &json!("value2"));
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

        let result = process_entry_ports(&input, None, &remote)
            .await
            .expect("Should succeed");

        let vertex = result
            .get("vertex1")
            .expect("vertex1 missing")
            .clone()
            .commit_all(&storage_conf)
            .await
            .expect("commit_all failed");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert_matches!(port1, DataStorage::Walrus(_));
        assert_matches!(port2, DataStorage::Inline(_));

        mock_put.assert_async().await;
    }

    #[tokio::test]
    async fn process_entry_ports_missing_remote_hint() {
        let input = json!({
            "vertex1": {
                "port1": "a".repeat(80_000),
            }
        });
        let remote = vec![];

        let result = process_entry_ports(&input, None, &remote).await;

        assert_matches!(result, Err(NexusCliError::Any(_)));
    }

    #[tokio::test]
    async fn process_entry_ports_invalid_input_not_object() {
        let input = json!("not an object");
        let remote = vec![];

        let result = process_entry_ports(&input, None, &remote).await;

        assert_matches!(result, Err(NexusCliError::Any(_)));
    }

    #[tokio::test]
    async fn process_entry_ports_invalid_vertex_not_object() {
        let input = json!({
            "vertex1": "not an object"
        });
        let remote = vec![];

        let result = process_entry_ports(&input, None, &remote).await;

        assert_matches!(result, Err(NexusCliError::Any(_)));
    }
}
