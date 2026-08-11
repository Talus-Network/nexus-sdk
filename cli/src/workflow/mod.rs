use {
    crate::{
        cli_conf::StorageKind,
        nexus_data_json::{hint_remote_fields, json_to_nexus_data_map},
        prelude::*,
    },
    anyhow::anyhow,
    nexus_sdk::{scheduler::TaskInputs, types::NexusData, walrus::StorageConf},
    serde_json::Value,
    std::collections::{BTreeMap, HashMap, HashSet},
};

/// Validated inline Task inputs plus remote selections awaiting materialization.
///
/// [`TaskInputs`] cannot retain which values the caller selected for remote storage, so this plan
/// keeps that local decision without performing a network request before authoritative preflight.
#[derive(Debug)]
pub(crate) struct EntryPortPlan {
    prepared: HashMap<String, HashMap<String, NexusData>>,
    remote_handles: HashSet<String>,
}

impl EntryPortPlan {
    /// Parses and validates every local entry-port decision without uploading data.
    pub(crate) fn new(
        input: &Value,
        preferred_remote_storage: Option<StorageKind>,
        remote: &[String],
        storage_conf: &StorageConf,
    ) -> Result<Self, NexusCliError> {
        let Some(vertices) = input.as_object() else {
            return Err(NexusCliError::Any(anyhow!(
                "Input JSON must be an object with vertex names as keys."
            )));
        };

        let mut prepared = HashMap::new();
        let mut valid_handles = HashSet::new();
        let mut flattened = serde_json::Map::new();

        for (vertex, data) in vertices {
            let Some(ports) = data.as_object() else {
                return Err(NexusCliError::Any(anyhow!(
                    "Input JSON for vertex '{vertex}' must be an object with port names as keys."
                )));
            };

            for (port, value) in ports {
                let handle = format!("{vertex}.{port}");
                valid_handles.insert(handle.clone());
                flattened.insert(handle, value.clone());
            }
            prepared.insert(
                vertex.clone(),
                json_to_nexus_data_map(data).map_err(NexusCliError::Any)?,
            );
        }

        let mut remote_handles = HashSet::new();
        for handle in remote {
            if !remote_handles.insert(handle.clone()) {
                return Err(NexusCliError::Any(anyhow!(
                    "Remote input selector '{handle}' is duplicated"
                )));
            }
            if !valid_handles.contains(handle) {
                return Err(NexusCliError::Any(anyhow!(
                    "Remote input selector '{handle}' does not identify an input field"
                )));
            }
        }

        let preferred_remote_storage = preferred_remote_storage.unwrap_or(StorageKind::Walrus);
        if !remote_handles.is_empty() {
            match preferred_remote_storage {
                StorageKind::Walrus => storage_conf
                    .validate_walrus_upload()
                    .map_err(NexusCliError::Any)?,
                StorageKind::Inline => {
                    return Err(NexusCliError::Any(anyhow!(
                        "Cannot store data remotely using inline storage"
                    )));
                }
            }
        }

        let remote_hints = hint_remote_fields(&Value::Object(flattened), &remote_handles)
            .map_err(NexusCliError::Any)?;

        if !remote_hints.is_empty() {
            return Err(NexusCliError::Any(anyhow!(
                "Some input fields must be stored remotely to fit within transaction size limits. Please add the following argument to your command:\n\n{command} {fields}",
                command = "--remote",
                fields = remote_hints.join(",")
            )));
        }

        Ok(Self {
            prepared,
            remote_handles,
        })
    }

    /// Returns inline typed inputs for authoritative shape preflight.
    pub(crate) fn task_inputs(&self) -> TaskInputs {
        self.prepared
            .iter()
            .map(|(vertex, ports)| {
                (
                    vertex.clone(),
                    ports
                        .iter()
                        .map(|(port, value)| (port.clone(), value.clone()))
                        .collect(),
                )
            })
            .collect()
    }

    /// Uploads selected values after authoritative preflight and returns final typed inputs.
    pub(crate) async fn materialize(
        self,
        storage_conf: &StorageConf,
    ) -> Result<TaskInputs, NexusCliError> {
        let mut result = BTreeMap::new();
        for (vertex, ports) in self.prepared {
            let mut uploaded = BTreeMap::new();
            for (port, mut value) in ports {
                if self.remote_handles.contains(&format!("{vertex}.{port}")) {
                    value = value
                        .upload_data_fields(storage_conf)
                        .await
                        .map_err(NexusCliError::Any)?;
                }
                uploaded.insert(port, value);
            }
            result.insert(vertex, uploaded);
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::nexus_data_json::nexus_data_to_json_value,
        assert_matches::assert_matches,
        mockito::{Server, ServerGuard},
        nexus_sdk::walrus::{BlobObject, BlobStorage, NewlyCreated, StorageConf, StorageInfo},
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
    async fn entry_port_plan_preserves_inline_values_without_remote_work() {
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

        let result = EntryPortPlan::new(&input, None, &remote, &storage_conf)
            .map(|plan| plan.task_inputs())
            .expect("Should succeed");

        let vertex = result.get("vertex1").expect("vertex1 missing");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert!(!port1.has_walrus());
        assert_eq!(nexus_data_to_json_value(port1), json!("value1"));

        assert!(!port2.has_walrus());
        assert_eq!(nexus_data_to_json_value(port2), json!("value2"));
    }

    #[tokio::test]
    async fn entry_port_plan_materializes_only_selected_remote_values() {
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
        let mock_get = server
            .mock("GET", "/v1/blobs/json_blob_id")
            .with_status(200)
            .with_body(br#""value1""#)
            .create_async()
            .await;

        let result = EntryPortPlan::new(&input, None, &remote, &storage_conf)
            .expect("local preparation succeeds")
            .materialize(&storage_conf)
            .await
            .expect("Should succeed");

        let vertex = result.get("vertex1").expect("vertex1 missing");
        let port1 = vertex.get("port1").expect("port1 missing");
        let port2 = vertex.get("port2").expect("port2 missing");

        assert!(port1.has_walrus());
        assert!(!port2.has_walrus());

        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[test]
    fn entry_port_plan_rejects_a_missing_remote_hint() {
        let input = json!({
            "vertex1": {
                "port1": "a".repeat(80_000),
            }
        });
        let remote = vec![];

        let result = EntryPortPlan::new(&input, None, &remote, &StorageConf::default());

        assert_matches!(result, Err(NexusCliError::Any(_)));
    }

    #[test]
    fn entry_port_plan_rejects_a_non_object_input() {
        let input = json!("not an object");
        let remote = vec![];

        let result = EntryPortPlan::new(&input, None, &remote, &StorageConf::default());

        assert_matches!(result, Err(NexusCliError::Any(_)));
    }

    #[test]
    fn entry_port_plan_rejects_a_non_object_vertex() {
        let input = json!({
            "vertex1": "not an object"
        });
        let remote = vec![];

        let result = EntryPortPlan::new(&input, None, &remote, &StorageConf::default());

        assert_matches!(result, Err(NexusCliError::Any(_)));
    }

    #[tokio::test]
    async fn entry_port_plan_rejects_a_malformed_later_vertex_without_upload() {
        let input = json!({
            "a_valid": { "port": "value" },
            "z_invalid": "not an object",
        });
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .expect(0)
            .create_async()
            .await;

        let result = EntryPortPlan::new(&input, None, &["a_valid.port".to_owned()], &storage_conf);

        assert_matches!(result, Err(NexusCliError::Any(_)));
        put.assert_async().await;
    }

    #[tokio::test]
    async fn entry_port_plan_rejects_remote_reference_overflow_without_upload() {
        let ports = (0..6)
            .map(|index| {
                (
                    format!("port-{index}"),
                    Value::Array((0..128).map(|_| Value::String("x".repeat(100))).collect()),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let input = Value::Object(serde_json::Map::from_iter([(
            "vertex".to_owned(),
            Value::Object(ports),
        )]));
        let remote = (0..6)
            .map(|index| format!("vertex.port-{index}"))
            .collect::<Vec<_>>();
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Server must start");
        let put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .expect(0)
            .create_async()
            .await;

        let result = EntryPortPlan::new(&input, None, &remote, &storage_conf);

        assert_matches!(result, Err(NexusCliError::Any(_)));
        put.assert_async().await;
    }

    #[test]
    fn entry_port_plan_rejects_duplicate_remote_selector() {
        let input = json!({ "vertex": { "port": "value" } });
        let remote = vec!["vertex.port".to_owned(), "vertex.port".to_owned()];

        let result = EntryPortPlan::new(&input, None, &remote, &StorageConf::default());

        assert_matches!(result, Err(NexusCliError::Any(error)) if error.to_string().contains("duplicated"));
    }

    #[test]
    fn entry_port_plan_rejects_unknown_remote_selector() {
        let input = json!({ "vertex": { "port": "value" } });

        let result = EntryPortPlan::new(
            &input,
            None,
            &["vertex.missing".to_owned()],
            &StorageConf::default(),
        );

        assert_matches!(result, Err(NexusCliError::Any(error)) if error.to_string().contains("does not identify"));
    }
}
