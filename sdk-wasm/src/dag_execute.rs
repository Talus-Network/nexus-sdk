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

/// ✅ Main function: Prepare DAG execution transaction
#[wasm_bindgen]
pub fn prepare_dag_execution_transaction(
    dag_id: &str,
    entry_group: &str,
    input_json: &str,
    gas_budget: &str,
    gas_coin_id: Option<String>,
) -> ExecutionResult {
    // Parse input JSON
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

    // Parse gas budget
    let gas_budget_u64: u64 = match gas_budget.parse() {
        Ok(budget) => budget,
        Err(e) => {
            return ExecutionResult {
                is_success: false,
                error_message: Some(format!("Gas budget parsing error: {}", e)),
                transaction_data: None,
            }
        }
    };

    // Create execution parameters
    let execution_params = ExecutionParams {
        dag_id: dag_id.to_string(),
        entry_group: entry_group.to_string(),
        input_data,
        gas_budget: gas_budget_u64,
        gas_coin_id: gas_coin_id.clone(),
    };

    // For now, we'll create a placeholder encryption info
    // In a real implementation, this would be populated by fetching DAG info
    let encryption_info = EncryptionInfo {
        has_encrypted_ports: false,
        encrypted_ports: HashMap::new(),
        requires_session: false,
    };

    // Create operation sequence for DAG execution
    let mut steps = Vec::new();

    // Step 1: Fetch DAG object
    steps.push(DagExecutionOperation {
        operation: "fetch_dag".to_string(),
        description: format!("Fetch DAG object with ID: {}", dag_id),
        sdk_function: "fetch_object_by_id".to_string(),
        parameters: Some(serde_json::json!({
            "object_id": dag_id
        })),
    });

    // Step 2: Fetch encrypted entry ports (if needed)
    steps.push(DagExecutionOperation {
        operation: "fetch_encrypted_ports".to_string(),
        description: "Fetch information about encrypted entry ports".to_string(),
        sdk_function: "fetch_encrypted_entry_ports".to_string(),
        parameters: Some(serde_json::json!({
            "dag_id": dag_id,
            "entry_group": entry_group
        })),
    });

    // Step 3: Encrypt input data (if needed)
    if encryption_info.has_encrypted_ports {
        steps.push(DagExecutionOperation {
            operation: "encrypt_inputs".to_string(),
            description: "Encrypt sensitive input data using active session".to_string(),
            sdk_function: "encrypt_entry_ports_once".to_string(),
            parameters: Some(serde_json::json!({
                "targets": encryption_info.encrypted_ports
            })),
        });
    }

    // Step 4: Create execution transaction
    steps.push(DagExecutionOperation {
        operation: "create_execution_tx".to_string(),
        description: "Create programmable transaction for DAG execution".to_string(),
        sdk_function: "dag::execute".to_string(),
        parameters: Some(serde_json::json!({
            "dag_id": dag_id,
            "entry_group": entry_group,
            "input_data": execution_params.input_data
        })),
    });

    // Step 5: Set gas parameters
    steps.push(DagExecutionOperation {
        operation: "set_gas_params".to_string(),
        description: "Configure gas coin and budget for transaction".to_string(),
        sdk_function: "TransactionData::new_programmable".to_string(),
        parameters: Some(serde_json::json!({
            "gas_budget": gas_budget_u64,
            "gas_coin_id": gas_coin_id
        })),
    });

    // Step 6: Sign and execute
    steps.push(DagExecutionOperation {
        operation: "execute".to_string(),
        description: "Sign and execute the transaction on Sui network".to_string(),
        sdk_function: "sign_and_execute_transaction".to_string(),
        parameters: None,
    });

    // Create final execution sequence
    let execution_sequence = DagExecutionSequence {
        operation_type: "dag_execute".to_string(),
        steps,
        execution_params,
        encryption_info,
    };

    // Serialize and return
    match serde_json::to_string(&execution_sequence) {
        Ok(serialized) => ExecutionResult {
            is_success: true,
            error_message: None,
            transaction_data: Some(serialized),
        },
        Err(e) => ExecutionResult {
            is_success: false,
            error_message: Some(format!("Execution sequence serialization error: {}", e)),
            transaction_data: None,
        },
    }
}

/// ✅ Helper function: Validate execution parameters
#[wasm_bindgen]
pub fn validate_execution_params(
    dag_id: &str,
    entry_group: &str,
    input_json: &str,
    gas_budget: &str,
) -> ExecutionResult {
    // Validate DAG ID format
    if dag_id.is_empty() {
        return ExecutionResult {
            is_success: false,
            error_message: Some("DAG ID cannot be empty".to_string()),
            transaction_data: None,
        };
    }

    // Validate entry group
    if entry_group.is_empty() {
        return ExecutionResult {
            is_success: false,
            error_message: Some("Entry group cannot be empty".to_string()),
            transaction_data: None,
        };
    }

    // Validate input JSON
    if let Err(e) = serde_json::from_str::<serde_json::Value>(input_json) {
        return ExecutionResult {
            is_success: false,
            error_message: Some(format!("Invalid input JSON: {}", e)),
            transaction_data: None,
        };
    }

    // Validate gas budget
    let gas_budget_u64: u64 = match gas_budget.parse() {
        Ok(budget) => budget,
        Err(e) => {
            return ExecutionResult {
                is_success: false,
                error_message: Some(format!("Gas budget parsing error: {}", e)),
                transaction_data: None,
            }
        }
    };

    if gas_budget_u64 == 0 {
        return ExecutionResult {
            is_success: false,
            error_message: Some("Gas budget must be greater than 0".to_string()),
            transaction_data: None,
        };
    }

    ExecutionResult {
        is_success: true,
        error_message: None,
        transaction_data: Some("Execution parameters are valid".to_string()),
    }
}

/// ✅ Function to check if DAG requires authentication/encryption
#[wasm_bindgen]
pub fn check_execution_requirements(dag_id: &str, entry_group: &str) -> ExecutionResult {
    // This would normally make an async call to fetch DAG info
    // For now, return a placeholder that indicates what's needed

    let requirements = serde_json::json!({
        "dag_id": dag_id,
        "entry_group": entry_group,
        "requires_authentication": false, // Would be determined by checking encrypted ports
        "requires_gas_coin": true,
        "estimated_gas": 10000000, // Placeholder estimate
        "supported_entry_groups": [entry_group] // Would come from DAG metadata
    });

    match serde_json::to_string(&requirements) {
        Ok(serialized) => ExecutionResult {
            is_success: true,
            error_message: None,
            transaction_data: Some(serialized),
        },
        Err(e) => ExecutionResult {
            is_success: false,
            error_message: Some(format!("Requirements check error: {}", e)),
            transaction_data: None,
        },
    }
}

/// ✅ Build DAG execution transaction using SDK (CLI-compatible)
#[wasm_bindgen]
pub fn build_dag_execution_transaction(
    dag_id: &str,
    entry_group: &str,
    input_json: &str,
    encrypted_ports_json: &str,
    gas_budget: &str,
) -> ExecutionResult {
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        // Parse inputs
        let input_data: serde_json::Value = serde_json::from_str(input_json)?;
        let encrypted_ports: std::collections::HashMap<String, Vec<String>> =
            serde_json::from_str(encrypted_ports_json)?;
        let gas_budget_u64: u64 = gas_budget.parse()?;

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

                // Create NexusData like CLI (encrypted vs non-encrypted)
                let json_string = serde_json::to_string(port_value)?;
                let json_bytes = json_string.as_bytes().to_vec();

                // Use appropriate NexusData function based on encryption status (like CLI)
                let data_target = if is_encrypted {
                    "{{primitives_pkg_id}}::data::inline_one" // Use inline_one for encrypted data
                } else {
                    "{{primitives_pkg_id}}::data::inline_one" // Use inline_one for regular data too
                };

                commands.push(serde_json::json!({
                    "type": "moveCall",
                    "target": data_target,
                    "arguments": [
                        {"type": "pure", "pure_type": "vector_u8", "value": json_bytes}
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

        // Step 4: Final DAG execution call (exactly like CLI)
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
                {"type": "clock_object"}
            ],
            "result_index": command_index
        }));

        let transaction_data = serde_json::json!({
            "commands": commands,
            "gas_budget": gas_budget_u64,
            "encrypted_ports_count": encrypted_ports.len(),
            "vertices_count": input_data.as_object().map_or(0, |obj| obj.len())
        });

        Ok(serde_json::json!({
            "success": true,
            "transaction_data": transaction_data.to_string(),
            "message": "CLI-compatible transaction built successfully"
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

/// ✅ Simple validation for DAG execution readiness
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
