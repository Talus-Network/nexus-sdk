//! Preflight and materialization of task inputs with explicit remote selections.

use {
    super::input_json::{hint_remote_fields, is_canonical_nexus_data, nexus_data_from_json_value},
    crate::{
        execution_limits::MAX_RESOLVED_INPUT_BYTES,
        scheduler::TaskInputs,
        types::NexusData,
        walrus::WalrusUploadData,
    },
    anyhow::{anyhow, ensure, Result as AnyResult},
    serde_json::Value,
    std::collections::{BTreeMap, HashSet},
};

/// Local preflight commitments and the exact bytes awaiting explicit upload.
#[derive(Debug)]
pub struct TaskInputPlan {
    prepared: TaskInputs,
    uploads: BTreeMap<String, WalrusUploadData>,
}

impl TaskInputPlan {
    /// Parses all local inputs and remote selectors without making a request.
    /// Arrays denote Many ports. Canonical references can be supplied directly.
    pub fn new(input: &Value, remote: &[String]) -> AnyResult<Self> {
        let vertices = input
            .as_object()
            .ok_or_else(|| anyhow!("Input JSON must be an object with vertex names as keys"))?;
        let mut selected = HashSet::new();
        for handle in remote {
            ensure!(
                selected.insert(handle.clone()),
                "Remote input selector '{handle}' is duplicated"
            );
        }
        let mut prepared = TaskInputs::new();
        let mut uploads = BTreeMap::new();
        let mut flattened = serde_json::Map::new();
        let mut total_bytes = 0usize;
        for (vertex, data) in vertices {
            let ports = data.as_object().ok_or_else(|| {
                anyhow!(
                    "Input JSON for vertex '{vertex}' must be an object with port names as keys"
                )
            })?;
            let mut result = BTreeMap::new();
            for (port, value) in ports {
                let handle = format!("{vertex}.{port}");
                ensure!(
                    !flattened.contains_key(&handle),
                    "ambiguous input selector '{handle}'"
                );
                flattened.insert(handle.clone(), value.clone());
                let value = if selected.contains(&handle) {
                    ensure!(!is_canonical_nexus_data(value), "remote selections accept JSON data; reuse existing canonical references directly");
                    let upload = WalrusUploadData::from_json_document(
                        serde_json::to_vec(value)?,
                        value.is_array(),
                    )?;
                    total_bytes = total_bytes
                        .checked_add(upload.byte_len())
                        .ok_or_else(|| anyhow!("input size overflow"))?;
                    let placeholder = upload.preflight_reference()?;
                    uploads.insert(handle, upload);
                    placeholder
                } else {
                    nexus_data_from_json_value(value.clone()).map_err(|error| anyhow!("input '{handle}': {error}; select this port for remote storage when its data is large"))?
                };
                result.insert(port.clone(), value);
            }
            prepared.insert(vertex.clone(), result);
        }
        for handle in &selected {
            ensure!(
                flattened.contains_key(handle),
                "Remote input selector '{handle}' does not identify an input field"
            );
        }
        ensure!(
            total_bytes <= MAX_RESOLVED_INPUT_BYTES,
            "remote inputs exceed the execution byte limit"
        );
        let hints = hint_remote_fields(&Value::Object(flattened), &selected)?;
        ensure!(
            hints.is_empty(),
            "Inputs exceed transaction size limits; select these remote ports: {}",
            hints.join(",")
        );
        Ok(Self { prepared, uploads })
    }

    /// Returns commitments for authoritative DAG preflight. Pending uploads use placeholders.
    pub fn preflight_inputs(&self) -> TaskInputs {
        self.prepared.clone()
    }

    /// Whether the plan contains payloads awaiting upload.
    pub fn has_uploads(&self) -> bool {
        !self.uploads.is_empty()
    }

    /// Whether the plan includes remote commitments that already exist.
    pub fn has_references(&self) -> bool {
        self.prepared.iter().any(|(vertex, ports)| {
            ports.iter().any(|(port, data)| {
                data.has_walrus() && !self.uploads.contains_key(&format!("{vertex}.{port}"))
            })
        })
    }

    /// Resolves existing commitments before a new paid upload is allowed to start.
    pub async fn verify_references(&self, reader: &crate::walrus::WalrusReader) -> AnyResult<()> {
        for (vertex, ports) in &self.prepared {
            let existing = ports
                .iter()
                .filter(|(port, _)| !self.uploads.contains_key(&format!("{vertex}.{port}")))
                .map(|(port, value)| (port.clone(), value.clone()))
                .collect();
            let resolved = reader.resolve_ports(existing).await?;
            let mut remaining = MAX_RESOLVED_INPUT_BYTES;
            for value in resolved.values().flatten() {
                let size = match value {
                    crate::types::NexusValue::InlineData { bytes } => bytes.len(),
                    _ => 32,
                };
                remaining = remaining
                    .checked_sub(size)
                    .ok_or_else(|| anyhow!("resolved inputs exceed the execution byte limit"))?;
            }
            for port in ports.keys() {
                if let Some(upload) = self.uploads.get(&format!("{vertex}.{port}")) {
                    remaining = remaining.checked_sub(upload.byte_len()).ok_or_else(|| {
                        anyhow!(
                            "resolved inputs and pending uploads exceed the execution byte limit"
                        )
                    })?;
                }
            }
        }
        Ok(())
    }

    /// Uploads after preflight through the caller's storage policy and checks every commitment.
    pub async fn materialize_with<F, Fut>(mut self, mut upload: F) -> AnyResult<TaskInputs>
    where
        F: FnMut(String, WalrusUploadData) -> Fut,
        Fut: std::future::Future<Output = AnyResult<NexusData>>,
    {
        for (handle, data) in self.uploads {
            let result = upload(handle.clone(), data.clone()).await?;
            data.verify_reference(&result)?;
            for (vertex, ports) in &mut self.prepared {
                for (port, value) in ports {
                    if format!("{vertex}.{port}") == handle {
                        ensure!(
                            result.is_well_formed()
                                && result.has_walrus()
                                && result.is_many() == value.is_many()
                                && result.values()?.len() == value.values()?.len(),
                            "upload changed the preflight port shape"
                        );
                        *value = result.clone();
                    }
                }
            }
        }
        Ok(self.prepared)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[tokio::test]
    async fn prepares_large_remote_values_and_materializes_only_selected_ports() {
        let value = "x".repeat(100_000);
        let plan = TaskInputPlan::new(
            &json!({"v":{"large":value, "inline":"small"}}),
            &["v.large".into()],
        )
        .unwrap();
        assert!(plan.preflight_inputs()["v"]["large"].has_walrus());
        let result = plan
            .materialize_with(|handle, bytes| async move {
                assert_eq!(handle, "v.large");
                assert_eq!(
                    bytes.values(),
                    &[serde_json::to_vec(&"x".repeat(100_000)).unwrap()]
                );
                {
                    use sha2::{Digest as _, Sha256};
                    NexusData::walrus_data(
                        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                        Sha256::digest(&bytes.values()[0]).to_vec(),
                    )
                }
            })
            .await
            .unwrap();
        assert!(result["v"]["large"].has_walrus());
        assert!(!result["v"]["inline"].has_walrus());
    }

    #[test]
    fn validates_all_local_decisions_before_uploading() {
        assert!(TaskInputPlan::new(&json!("invalid"), &[]).is_err());
        assert!(
            TaskInputPlan::new(&json!({"a":{"port":"x"},"z":false}), &["a.port".into()]).is_err()
        );
        assert!(TaskInputPlan::new(
            &json!({"a":{"port":"x"}}),
            &["a.port".into(), "a.port".into()]
        )
        .is_err());
        assert!(TaskInputPlan::new(&json!({"a":{"port":"x"}}), &["a.missing".into()]).is_err());
        assert!(TaskInputPlan::new(&json!({"a":{"port":"x".repeat(100_000)}}), &[]).is_err());
        let ports = (0..6)
            .map(|i| (format!("p{i}"), json!(vec!["x"; 128])))
            .collect::<serde_json::Map<_, _>>();
        assert!(TaskInputPlan::new(
            &json!({"a":ports}),
            &(0..6).map(|i| format!("a.p{i}")).collect::<Vec<_>>()
        )
        .is_err());
    }
}
