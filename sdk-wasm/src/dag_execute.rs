use {
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    wasm_bindgen::prelude::*,
};

/// DAG execution operation sequence for JS-side transaction building
#[derive(Serialize, Deserialize)]
pub struct DagExecutionSequence {
    pub operation_type: String,
    pub steps: Vec<DagExecutionOperation>,
    pub execution_params: ExecutionParams,
    pub encryption_info: EncryptionInfo,
}

/// Individual DAG execution operation
#[derive(Serialize, Deserialize)]
pub struct DagExecutionOperation {
    pub operation: String,
    pub description: String,
    pub sdk_function: String,
    pub parameters: Option<serde_json::Value>,
}

/// Execution parameters
#[derive(Serialize, Deserialize)]
pub struct ExecutionParams {
    pub dag_id: String,
    pub entry_group: String,
    pub input_data: serde_json::Value,
    pub gas_budget: u64,
    pub gas_coin_id: Option<String>,
}

/// Encryption information for entry ports
#[derive(Serialize, Deserialize)]
pub struct EncryptionInfo {
    pub has_encrypted_ports: bool,
    pub encrypted_ports: HashMap<String, Vec<String>>, // vertex -> [port_names]
    pub requires_session: bool,
}

/// WASM-exported execution result
#[wasm_bindgen]
pub struct ExecutionResult {
    is_success: bool,
    error_message: Option<String>,
    transaction_data: Option<String>,
}

#[wasm_bindgen]
impl ExecutionResult {
    #[wasm_bindgen(getter)]
    pub fn is_success(&self) -> bool {
        self.is_success
    }

    #[wasm_bindgen(getter)]
    pub fn error_message(&self) -> Option<String> {
        self.error_message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn transaction_data(&self) -> Option<String> {
        self.transaction_data.clone()
    }
}

/// ✅ Build DAG execution transaction using SDK (CLI-compatible with auto-encryption)
///
/// # Walrus (remote storage) parameters
/// When using remote storage, call `upload_json_to_walrus` for each remote port first,
/// then pass the blob IDs here.
///
/// * `remote_ports_json` - JSON array of "vertex.port" strings, e.g. `["add-vertex.a"]`
/// * `remote_ports_blob_ids_json` - JSON object mapping "vertex.port" to blob IDs
/// * `walrus_save_for_epochs` - Number of epochs (1-53) when using Walrus. Required when remote_ports are provided.
#[wasm_bindgen]
pub fn build_dag_execution_transaction(
    dag_id: &str,
    entry_group: &str,
    input_json: &str,
    encrypted_ports_json: &str,
    gas_budget: &str,
    priority_fee_per_gas_unit: Option<String>,
    remote_ports_json: Option<String>,
    remote_ports_blob_ids_json: Option<String>,
    walrus_save_for_epochs: Option<String>,
) -> ExecutionResult {
    use web_sys::console;

    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        // Parse inputs
        let mut input_data: serde_json::Value = serde_json::from_str(input_json)?;
        let encrypted_ports: std::collections::HashMap<String, Vec<String>> =
            serde_json::from_str(encrypted_ports_json)?;
        let gas_budget_u64: u64 = gas_budget.parse()?;

        // 🔐 CLI-parity: If there are encrypted ports, encrypt input data first
        // BUT: Check if data is already encrypted (array = already encrypted)
        if !encrypted_ports.is_empty() {
            // Check if any encrypted port is already in encrypted form (byte array)
            let mut needs_encryption = false;
            for (vertex_name, port_names) in &encrypted_ports {
                if let Some(vertex_obj) = input_data.get(vertex_name) {
                    for port_name in port_names {
                        if let Some(port_value) = vertex_obj.get(port_name) {
                            // If port_value is NOT an array, it needs encryption
                            // If it IS an array, it's already encrypted
                            if !port_value.is_array() {
                                needs_encryption = true;
                                break;
                            }
                        }
                    }
                }
                if needs_encryption {
                    break;
                }
            }

            if needs_encryption {
                console::log_1(
                    &"🔐 Encrypted ports detected, encrypting input data (CLI-parity)...".into(),
                );

                // Call the encryption function from crypto.rs (master key loaded internally)
                let encrypt_result = crate::encrypt_entry_ports(input_json, encrypted_ports_json);

                // Parse the encryption result
                let encrypt_response: serde_json::Value = serde_json::from_str(&encrypt_result)?;

                if !encrypt_response["success"].as_bool().unwrap_or(false) {
                    let error_msg = encrypt_response["error"]
                        .as_str()
                        .unwrap_or("Encryption failed");
                    return Err(format!("Input encryption failed: {}", error_msg).into());
                }

                // Use the encrypted input data
                input_data = encrypt_response["input_data"].clone();
                console::log_1(
                    &format!(
                        "✅ Successfully encrypted {} ports (CLI-parity)",
                        encrypt_response["encrypted_count"].as_u64().unwrap_or(0)
                    )
                    .into(),
                );
            } else {
                console::log_1(&"Input data already encrypted, skipping encryption (prevent double encryption)".into());
            }
        } else {
            console::log_1(&"No encrypted ports, using plaintext input (CLI-parity)".into());
        }

        // Parse Walrus remote ports (CLI --remote parity)
        let remote_ports: std::collections::HashSet<String> = remote_ports_json
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let remote_ports_blob_ids: std::collections::HashMap<String, String> =
            remote_ports_blob_ids_json
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
        let walrus_epochs: u8 = walrus_save_for_epochs
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if !remote_ports.is_empty() {
            if walrus_epochs == 0 {
                return Err("walrus_save_for_epochs is required when using remote ports".into());
            }
            for rp in &remote_ports {
                if !remote_ports_blob_ids.contains_key(rp) {
                    return Err(format!(
                        "Missing blob ID for remote port '{}'. Call upload_json_to_walrus first.",
                        rp
                    )
                    .into());
                }
            }
        }

        // Build transaction commands that mirror CLI's dag::execute function
        let mut commands = Vec::new();

        // Step 1: Create empty VecMap for vertex inputs (like CLI)
        commands.push(serde_json::json!({
            "type": "moveCall",
            "target": "0x2::vec_map::empty",
            "typeArguments": [
                "{{workflow_pkg_id}}::dag::Vertex",
                "0x2::vec_map::VecMap<{{workflow_pkg_id}}::dag::InputPort, {{primitives_pkg_id}}::data::NexusData>"
            ],
            "arguments": [],
            "result_index": 0
        }));

        let mut command_index = 1;

        // Step 2: Process each vertex like CLI
        for (vertex_name, vertex_data) in input_data.as_object().unwrap_or(&serde_json::Map::new())
        {
            if !vertex_data.is_object() {
                continue;
            }

            // Create vertex
            commands.push(serde_json::json!({
                "type": "moveCall",
                "target": "{{workflow_pkg_id}}::dag::vertex_from_string",
                "arguments": [{"type": "pure", "pure_type": "string", "value": vertex_name}],
                "result_index": command_index
            }));
            let vertex_result_index = command_index;
            command_index += 1;

            // Create empty inner VecMap for ports
            commands.push(serde_json::json!({
                "type": "moveCall",
                "target": "0x2::vec_map::empty",
                "typeArguments": [
                    "{{workflow_pkg_id}}::dag::InputPort",
                    "{{primitives_pkg_id}}::data::NexusData"
                ],
                "arguments": [],
                "result_index": command_index
            }));
            let inner_vecmap_result_index = command_index;
            command_index += 1;

            // Process each port
            for (port_name, port_value) in
                vertex_data.as_object().unwrap_or(&serde_json::Map::new())
            {
                let is_encrypted = encrypted_ports
                    .get(vertex_name)
                    .map_or(false, |ports| ports.contains(port_name));

                // Create input port (encrypted or normal like CLI)
                let port_target = if is_encrypted {
                    "{{workflow_pkg_id}}::dag::encrypted_input_port_from_string"
                } else {
                    "{{workflow_pkg_id}}::dag::input_port_from_string"
                };

                commands.push(serde_json::json!({
                    "type": "moveCall",
                    "target": port_target,
                    "arguments": [{"type": "pure", "pure_type": "string", "value": port_name}],
                    "result_index": command_index
                }));
                let port_result_index = command_index;
                command_index += 1;

                let remote_handle = format!("{}.{}", vertex_name, port_name);
                let is_remote = remote_ports.contains(&remote_handle);

                let (data_target, data_bytes) = if is_remote {
                    // Walrus: use blob_id bytes (JSON serialization of the string)
                    let blob_id = remote_ports_blob_ids.get(&remote_handle).ok_or_else(|| {
                        format!("Missing blob ID for remote port '{}'", remote_handle)
                    })?;
                    let blob_id_bytes =
                        serde_json::to_vec(&serde_json::Value::String(blob_id.clone()))?;
                    let target = if is_encrypted {
                        "{{primitives_pkg_id}}::data::walrus_one_encrypted"
                    } else {
                        "{{primitives_pkg_id}}::data::walrus_one"
                    };
                    (target, blob_id_bytes)
                } else {
                    // Inline: use port value bytes
                    let json_string = serde_json::to_string(port_value)?;
                    let bytes = json_string.as_bytes().to_vec();
                    let target = if is_encrypted {
                        "{{primitives_pkg_id}}::data::inline_one_encrypted"
                    } else {
                        "{{primitives_pkg_id}}::data::inline_one"
                    };
                    (target, bytes)
                };

                commands.push(serde_json::json!({
                    "type": "moveCall",
                    "target": data_target,
                    "arguments": [
                        {"type": "pure", "pure_type": "vector_u8", "value": data_bytes}
                    ],
                    "result_index": command_index
                }));
                let nexus_data_result_index = command_index;
                command_index += 1;

                // Insert port and data into inner VecMap
                commands.push(serde_json::json!({
                    "type": "moveCall",
                    "target": "0x2::vec_map::insert",
                    "typeArguments": [
                        "{{workflow_pkg_id}}::dag::InputPort",
                        "{{primitives_pkg_id}}::data::NexusData"
                    ],
                    "arguments": [
                        {"type": "result", "index": inner_vecmap_result_index},
                        {"type": "result", "index": port_result_index},
                        {"type": "result", "index": nexus_data_result_index}
                    ],
                    "result_index": command_index
                }));
                command_index += 1;
            }

            // Insert vertex and inner VecMap into outer VecMap
            commands.push(serde_json::json!({
                "type": "moveCall",
                "target": "0x2::vec_map::insert",
                "typeArguments": [
                    "{{workflow_pkg_id}}::dag::Vertex",
                    "0x2::vec_map::VecMap<{{workflow_pkg_id}}::dag::InputPort, {{primitives_pkg_id}}::data::NexusData>"
                ],
                "arguments": [
                    {"type": "result", "index": 0},
                    {"type": "result", "index": vertex_result_index},
                    {"type": "result", "index": inner_vecmap_result_index}
                ],
                "result_index": command_index
            }));
            command_index += 1;
        }

        // Step 3: Create entry group
        commands.push(serde_json::json!({
            "type": "moveCall",
            "target": "{{workflow_pkg_id}}::dag::entry_group_from_string",
            "arguments": [{"type": "pure", "pure_type": "string", "value": entry_group}],
            "result_index": command_index
        }));
        let entry_group_result_index = command_index;
        command_index += 1;

        // Parse priority fee (v0.5.0 feature)
        let priority_fee: u64 = priority_fee_per_gas_unit
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Step 4: Final DAG execution call (exactly like CLI/SDK)
        // Arguments order matches SDK: default_tap, dag, gas_service, network, entry_group, inputs, priority_fee, clock
        commands.push(serde_json::json!({
            "type": "moveCall",
            "target": "{{workflow_pkg_id}}::default_tap::begin_dag_execution",
            "arguments": [
                {"type": "shared_object_by_id", "id": "{{default_tap_object_id}}", "mutable": true},
                {"type": "shared_object_by_id", "id": dag_id, "mutable": false},
                {"type": "shared_object_by_id", "id": "{{gas_service_object_id}}", "mutable": true},
                {"type": "pure", "pure_type": "id", "value": "{{network_id}}"},
                {"type": "result", "index": entry_group_result_index},
                {"type": "result", "index": 0},
                {"type": "pure", "pure_type": "u64", "value": priority_fee},
                {"type": "clock_object"}
            ],
            "result_index": command_index
        }));

        let transaction_data = serde_json::json!({
            "commands": commands,
            "gas_budget": gas_budget_u64,
            "priority_fee_per_gas_unit": priority_fee,
            "encrypted_ports_count": encrypted_ports.len(),
            "vertices_count": input_data.as_object().map_or(0, |obj| obj.len()),
            "auto_encrypted": !encrypted_ports.is_empty(),
            "remote_ports_count": remote_ports.len(),
            "walrus_save_for_epochs": if remote_ports.is_empty() { None::<u8> } else { Some(walrus_epochs) }
        });

        Ok(serde_json::json!({
            "success": true,
            "transaction_data": transaction_data.to_string(),
            "message": "CLI-compatible transaction built successfully with auto-encryption",
            "encrypted": !encrypted_ports.is_empty()
        })
        .to_string())
    })();

    match result {
        Ok(response) => ExecutionResult {
            is_success: true,
            error_message: None,
            transaction_data: Some(response),
        },
        Err(e) => ExecutionResult {
            is_success: false,
            error_message: Some(format!("Transaction building error: {}", e)),
            transaction_data: None,
        },
    }
}

#[wasm_bindgen]
pub fn validate_dag_execution_readiness(
    dag_id: &str,
    entry_group: &str,
    input_json: &str,
) -> ExecutionResult {
    // Parse input JSON to validate structure
    let input_data: serde_json::Value = match serde_json::from_str(input_json) {
        Ok(data) => data,
        Err(e) => {
            return ExecutionResult {
                is_success: false,
                error_message: Some(format!("Input JSON parsing error: {}", e)),
                transaction_data: None,
            }
        }
    };

    // Basic validation checks
    if dag_id.is_empty() {
        return ExecutionResult {
            is_success: false,
            error_message: Some("DAG ID is required".to_string()),
            transaction_data: None,
        };
    }

    if entry_group.is_empty() {
        return ExecutionResult {
            is_success: false,
            error_message: Some("Entry group is required".to_string()),
            transaction_data: None,
        };
    }

    // Check if input data is an object (required for vertex-port mapping)
    if !input_data.is_object() {
        return ExecutionResult {
            is_success: false,
            error_message: Some(
                "Input data must be a JSON object with vertex-port structure".to_string(),
            ),
            transaction_data: None,
        };
    }

    let readiness_info = serde_json::json!({
        "dag_id": dag_id,
        "entry_group": entry_group,
        "input_vertices": input_data.as_object().unwrap().keys().collect::<Vec<_>>(),
        "ready_for_execution": true,
        "validation_timestamp": js_sys::Date::now() as u64 / 1000 // Unix timestamp
    });

    match serde_json::to_string(&readiness_info) {
        Ok(serialized) => ExecutionResult {
            is_success: true,
            error_message: None,
            transaction_data: Some(serialized),
        },
        Err(e) => ExecutionResult {
            is_success: false,
            error_message: Some(format!("Readiness validation error: {}", e)),
            transaction_data: None,
        },
    }
}
