use {
    nexus_sdk::types::DEFAULT_ENTRY_GROUP,
    serde::{Deserialize, Serialize},
    std::collections::{HashMap, HashSet},
    wasm_bindgen::prelude::*,
};

#[derive(Serialize, Deserialize)]
pub struct DagExecutionSequence {
    pub operation_type: String,
    pub steps: Vec<DagExecutionOperation>,
    pub execution_params: ExecutionParams,
}

#[derive(Serialize, Deserialize)]
pub struct DagExecutionOperation {
    pub operation: String,
    pub description: String,
    pub sdk_function: String,
    pub parameters: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ExecutionParams {
    pub dag_id: String,
    pub entry_group: String,
    pub input_data: serde_json::Value,
    pub gas_budget: u64,
    pub gas_coin_id: Option<String>,
}

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

/// Build a DAG execution transaction matching the SDK's `prepare_dag_execution` flow.
///
/// Input data is plain JSON. The Sui transaction is signed JS-side with the
/// Sui SDK using the stored private key.
///
/// # Walrus (remote storage) parameters
///
/// When using remote storage, call `upload_json_to_walrus` for each remote
/// port first, then pass the blob IDs here.
///
/// * `remote_ports_json` – JSON array of `"vertex.port"` strings,
///   e.g. `["add-vertex.a"]`
/// * `remote_ports_blob_ids_json` – JSON object mapping `"vertex.port"` to
///   Walrus blob IDs
#[wasm_bindgen]
pub fn build_dag_execution_transaction(
    dag_id: &str,
    entry_group: &str,
    input_json: &str,
    gas_budget: &str,
    priority_fee_per_gas_unit: Option<String>,
    remote_ports_json: Option<String>,
    remote_ports_blob_ids_json: Option<String>,
) -> ExecutionResult {
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        let input_data: serde_json::Value = serde_json::from_str(input_json)?;
        // Match `nexus_sdk` / CLI: `execute(..., entry_group.unwrap_or(DEFAULT_ENTRY_GROUP), ...)`
        let entry_group = if entry_group.trim().is_empty() {
            DEFAULT_ENTRY_GROUP
        } else {
            entry_group
        };
        let gas_budget_u64: u64 = gas_budget.parse()?;

        let priority_fee: u64 = priority_fee_per_gas_unit
            .as_ref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let remote_ports: HashSet<String> = remote_ports_json
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let remote_blob_ids: HashMap<String, String> = remote_ports_blob_ids_json
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        if !remote_ports.is_empty() {
            for rp in &remote_ports {
                if !remote_blob_ids.contains_key(rp) {
                    return Err(format!(
                        "Missing blob ID for remote port '{}'. Call upload_json_to_walrus first.",
                        rp
                    )
                    .into());
                }
            }
        }

        let mut commands = Vec::new();

        // Step 1: outer VecMap<Vertex, VecMap<InputPort, NexusData>>
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

        // Step 2: build per-vertex input maps
        for (vertex_name, vertex_data) in
            input_data.as_object().unwrap_or(&serde_json::Map::new())
        {
            if !vertex_data.is_object() {
                continue;
            }

            commands.push(serde_json::json!({
                "type": "moveCall",
                "target": "{{workflow_pkg_id}}::dag::vertex_from_string",
                "arguments": [{"type": "pure", "pure_type": "string", "value": vertex_name}],
                "result_index": command_index
            }));
            let vertex_result_index = command_index;
            command_index += 1;

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

            for (port_name, port_value) in
                vertex_data.as_object().unwrap_or(&serde_json::Map::new())
            {
                commands.push(serde_json::json!({
                    "type": "moveCall",
                    "target": "{{workflow_pkg_id}}::dag::input_port_from_string",
                    "arguments": [{"type": "pure", "pure_type": "string", "value": port_name}],
                    "result_index": command_index
                }));
                let port_result_index = command_index;
                command_index += 1;

                let remote_handle = format!("{}.{}", vertex_name, port_name);
                let is_remote = remote_ports.contains(&remote_handle);

                let (data_target, data_bytes) = if is_remote {
                    let blob_id = remote_blob_ids.get(&remote_handle).ok_or_else(|| {
                        format!("Missing blob ID for remote port '{}'", remote_handle)
                    })?;
                    let blob_id_bytes =
                        serde_json::to_vec(&serde_json::Value::String(blob_id.clone()))?;
                    ("{{primitives_pkg_id}}::data::walrus_one", blob_id_bytes)
                } else {
                    // Same as `primitives::Data::nexus_data_inline_from_json` for non-array values:
                    // `pure_arg(&serde_json::to_vec(json)?)`.
                    let bytes = serde_json::to_vec(port_value)?;
                    ("{{primitives_pkg_id}}::data::inline_one", bytes)
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

        // Step 3: entry group
        commands.push(serde_json::json!({
            "type": "moveCall",
            "target": "{{workflow_pkg_id}}::dag::entry_group_from_string",
            "arguments": [{"type": "pure", "pure_type": "string", "value": entry_group}],
            "result_index": command_index
        }));
        let entry_group_result_index = command_index;
        command_index += 1;

        // Step 4: prepare_dag_execution
        // Returns (RequestWalkExecution, DAGExecution, ExecutionGas).
        // JS side must access nested results [0], [1], [2] from this call.
        commands.push(serde_json::json!({
            "type": "moveCall",
            "target": "{{workflow_pkg_id}}::default_tap::prepare_dag_execution",
            "arguments": [
                {"type": "shared_object_by_id", "id": "{{default_tap_object_id}}", "mutable": true},
                {"type": "shared_object_by_id", "id": dag_id, "mutable": false},
                {"type": "shared_object_by_id", "id": "{{gas_service_object_id}}", "mutable": true},
                {"type": "shared_object_by_id", "id": "{{tool_registry_id}}", "mutable": false},
                {"type": "pure", "pure_type": "id", "value": "{{network_id}}"},
                {"type": "result", "index": entry_group_result_index},
                {"type": "result", "index": 0},
                {"type": "pure", "pure_type": "u64", "value": priority_fee},
                {"type": "clock_object"}
            ],
            "result_index": command_index,
            "returns": {
                "ticket": {"nested_index": 0},
                "execution": {"nested_index": 1},
                "execution_gas": {"nested_index": 2}
            }
        }));
        let prepare_result_index = command_index;
        let _ = command_index + 1;

        // Step 5: lock_gas_state_for_tool (one per tool)
        // JS side must iterate over tool gas objects and add one command per tool.
        // Template for a single tool:
        let gas_lock_template = serde_json::json!({
            "type": "moveCall",
            "target": "{{workflow_pkg_id}}::gas::lock_gas_state_for_tool",
            "arguments_template": [
                {"type": "nested_result", "index": prepare_result_index, "nested_index": 2, "description": "execution_gas"},
                {"type": "shared_object_by_id", "id": "{{tool_gas_object_id}}", "mutable": true, "description": "tool_gas (JS must provide per tool)"},
                {"type": "shared_object_by_id", "id": "{{invoker_gas_object_id}}", "mutable": true, "description": "invoker_gas (JS must provide)"},
                {"type": "shared_object_by_id", "id": dag_id, "mutable": false, "description": "dag"},
                {"type": "nested_result", "index": prepare_result_index, "nested_index": 1, "description": "execution"},
                {"type": "nested_result", "index": prepare_result_index, "nested_index": 0, "description": "ticket"}
            ],
            "note": "Repeat this command for each tool in the DAG. JS must supply actual tool_gas and invoker_gas object IDs."
        });

        // Step 6: request_network_to_execute_walks
        let walk_request_template = serde_json::json!({
            "type": "moveCall",
            "target": "{{workflow_pkg_id}}::dag::request_network_to_execute_walks",
            "arguments_template": [
                {"type": "shared_object_by_id", "id": dag_id, "mutable": false, "description": "dag"},
                {"type": "nested_result", "index": prepare_result_index, "nested_index": 1, "description": "execution"},
                {"type": "nested_result", "index": prepare_result_index, "nested_index": 0, "description": "ticket"},
                {"type": "shared_object_by_id", "id": "{{leader_registry_id}}", "mutable": false, "description": "leader_registry"},
                {"type": "clock_object"}
            ]
        });

        // Step 7: share DAGExecution and ExecutionGas
        let share_execution_template = serde_json::json!({
            "type": "moveCall",
            "target": "0x2::transfer::public_share_object",
            "typeArguments": ["{{workflow_pkg_id}}::dag::DAGExecution"],
            "arguments_template": [
                {"type": "nested_result", "index": prepare_result_index, "nested_index": 1, "description": "execution"}
            ]
        });

        let share_gas_template = serde_json::json!({
            "type": "moveCall",
            "target": "0x2::transfer::public_share_object",
            "typeArguments": ["{{workflow_pkg_id}}::gas::ExecutionGas"],
            "arguments_template": [
                {"type": "nested_result", "index": prepare_result_index, "nested_index": 2, "description": "execution_gas"}
            ]
        });

        let transaction_data = serde_json::json!({
            "commands": commands,
            "gas_budget": gas_budget_u64,
            "priority_fee_per_gas_unit": priority_fee,
            "vertices_count": input_data.as_object().map_or(0, |obj| obj.len()),
            "remote_ports_count": remote_ports.len(),
            "prepare_result_index": prepare_result_index,
            "post_prepare_templates": {
                "lock_gas_state_for_tool": gas_lock_template,
                "request_network_to_execute_walks": walk_request_template,
                "share_execution": share_execution_template,
                "share_execution_gas": share_gas_template
            }
        });

        Ok(serde_json::json!({
            "success": true,
            "transaction_data": transaction_data.to_string(),
            "message": "Transaction built (prepare_dag_execution + post-prepare templates)",
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

/// Validate that all required parameters are present before execution.
#[wasm_bindgen]
pub fn validate_dag_execution_readiness(
    dag_id: &str,
    entry_group: &str,
    input_json: &str,
) -> ExecutionResult {
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

    if dag_id.is_empty() {
        return ExecutionResult {
            is_success: false,
            error_message: Some("DAG ID is required".to_string()),
            transaction_data: None,
        };
    }

    let resolved_entry_group = if entry_group.trim().is_empty() {
        DEFAULT_ENTRY_GROUP
    } else {
        entry_group
    };

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
        "entry_group": resolved_entry_group,
        "entry_group_was_defaulted": entry_group.trim().is_empty(),
        "input_vertices": input_data.as_object().unwrap().keys().collect::<Vec<_>>(),
        "ready_for_execution": true,
        "validation_timestamp": js_sys::Date::now() as u64 / 1000
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
