use {
    nexus_sdk::{dag::validator, types::Dag},
    serde::{Deserialize, Serialize},
    wasm_bindgen::prelude::*,
};

/// Lightweight DAG operation sequence for JS-side transaction building
#[derive(Serialize, Deserialize)]
pub struct DagOperationSequence {
    pub operation_type: String,
    pub steps: Vec<DagOperation>,
    pub validated_dag_json: String,
    pub dag_summary: DagSummary,
}

/// Individual DAG operation that maps to SDK functions
#[derive(Serialize, Deserialize)]
pub struct DagOperation {
    pub operation: String, // "empty", "create_vertex", "create_edge", "publish", etc.
    pub description: String,
    pub sdk_function: String, // e.g., "dag::empty", "dag::create_vertex"
}

/// Summary statistics of the DAG
#[derive(Serialize, Deserialize)]
pub struct DagSummary {
    pub vertices_count: usize,
    pub edges_count: usize,
    pub has_entry_groups: bool,
    pub has_default_values: bool,
    pub has_outputs: bool,
}

/// WASM-exported result structure
#[wasm_bindgen]
pub struct PublishResult {
    is_success: bool,
    error_message: Option<String>,
    transaction_data: Option<String>,
}

#[wasm_bindgen]
impl PublishResult {
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

/// ✅ Main function: Validate DAG and return operation sequence for JS transaction building
#[wasm_bindgen]
pub fn prepare_dag_publish_transaction(dag_json: &str) -> PublishResult {
    // Parse DAG JSON
    let dag: Dag = match serde_json::from_str(dag_json) {
        Ok(dag) => dag,
        Err(e) => {
            return PublishResult {
                is_success: false,
                error_message: Some(format!("DAG JSON parsing error: {}", e)),
                transaction_data: None,
            }
        }
    };

    // Validate DAG
    if let Err(e) = validator::validate(dag.clone()) {
        return PublishResult {
            is_success: false,
            error_message: Some(format!("DAG validation error: {}", e)),
            transaction_data: None,
        };
    }

    // Analyze DAG and create operation sequence
    let dag_summary = DagSummary {
        vertices_count: dag.vertices.len(),
        edges_count: dag.edges.len(),
        has_entry_groups: dag
            .entry_groups
            .as_ref()
            .map_or(false, |groups| !groups.is_empty()),
        has_default_values: dag
            .default_values
            .as_ref()
            .map_or(false, |values| !values.is_empty()),
        has_outputs: dag
            .outputs
            .as_ref()
            .map_or(false, |outputs| !outputs.is_empty()),
    };

    // Create operation sequence that mirrors dag::create + dag::publish
    let mut steps = Vec::new();

    // Step 1: Create empty DAG
    steps.push(DagOperation {
        operation: "empty".to_string(),
        description: "Create an empty DAG using dag::empty()".to_string(),
        sdk_function: "dag::empty".to_string(),
    });

    // Step 2: Add vertices
    for vertex in &dag.vertices {
        steps.push(DagOperation {
            operation: "create_vertex".to_string(),
            description: format!("Create vertex '{}' using dag::create_vertex()", vertex.name),
            sdk_function: "dag::create_vertex".to_string(),
        });
    }

    // Step 3: Add default values (if any)
    if let Some(default_values) = &dag.default_values {
        for default_value in default_values {
            steps.push(DagOperation {
                operation: "create_default_value".to_string(),
                description: format!("Create default value for vertex '{}' port '{}' using dag::create_default_value()", default_value.vertex, default_value.input_port),
                sdk_function: "dag::create_default_value".to_string(),
            });
        }
    }

    // Step 4: Add edges
    for edge in &dag.edges {
        steps.push(DagOperation {
            operation: "create_edge".to_string(),
            description: format!(
                "Create edge from '{}' to '{}' using dag::create_edge()",
                edge.from.vertex, edge.to.vertex
            ),
            sdk_function: "dag::create_edge".to_string(),
        });
    }

    // Step 5: Add outputs (if any)
    if let Some(outputs) = &dag.outputs {
        for output in outputs {
            steps.push(DagOperation {
                operation: "create_output".to_string(),
                description: format!(
                    "Create output from vertex '{}' using dag::create_output()",
                    output.vertex
                ),
                sdk_function: "dag::create_output".to_string(),
            });
        }
    }

    // Step 6: Mark entry ports/vertices
    if let Some(entry_groups) = &dag.entry_groups {
        for entry_group in entry_groups {
            for vertex_name in &entry_group.vertices {
                if let Some(vertex) = dag.vertices.iter().find(|v| &v.name == vertex_name) {
                    if let Some(entry_ports) = &vertex.entry_ports {
                        for entry_port in entry_ports {
                            steps.push(DagOperation {
                                operation: "mark_entry_input_port".to_string(),
                                description: format!("Mark entry port '{}' on vertex '{}' using dag::mark_entry_input_port()", entry_port.name, vertex_name),
                                sdk_function: "dag::mark_entry_input_port".to_string(),
                            });
                        }
                    } else {
                        steps.push(DagOperation {
                            operation: "mark_entry_vertex".to_string(),
                            description: format!(
                                "Mark entry vertex '{}' using dag::mark_entry_vertex()",
                                vertex_name
                            ),
                            sdk_function: "dag::mark_entry_vertex".to_string(),
                        });
                    }
                }
            }
        }
    } else {
        // Handle default entry group
        for vertex in &dag.vertices {
            if let Some(entry_ports) = &vertex.entry_ports {
                for entry_port in entry_ports {
                    steps.push(DagOperation {
                        operation: "mark_entry_input_port".to_string(),
                        description: format!("Mark entry port '{}' on vertex '{}' (default group) using dag::mark_entry_input_port()", entry_port.name, vertex.name),
                        sdk_function: "dag::mark_entry_input_port".to_string(),
                    });
                }
            }
        }
    }

    // Step 7: Publish DAG
    steps.push(DagOperation {
        operation: "publish".to_string(),
        description: "Publish DAG using dag::publish() - makes it publicly accessible".to_string(),
        sdk_function: "dag::publish".to_string(),
    });

    // Create final operation sequence
    let operation_sequence = DagOperationSequence {
        operation_type: "dag_publish".to_string(),
        steps,
        validated_dag_json: dag_json.to_string(),
        dag_summary,
    };

    // Serialize and return
    match serde_json::to_string(&operation_sequence) {
        Ok(serialized) => PublishResult {
            is_success: true,
            error_message: None,
            transaction_data: Some(serialized),
        },
        Err(e) => PublishResult {
            is_success: false,
            error_message: Some(format!("Operation sequence serialization error: {}", e)),
            transaction_data: None,
        },
    }
}

/// ✅ Simple validation function (backward compatibility)
#[wasm_bindgen]
pub fn validate_dag_for_publish(dag_json: &str) -> PublishResult {
    let dag: Dag = match serde_json::from_str(dag_json) {
        Ok(dag) => dag,
        Err(e) => {
            return PublishResult {
                is_success: false,
                error_message: Some(format!("DAG JSON parsing error: {}", e)),
                transaction_data: None,
            }
        }
    };

    match validator::validate(dag) {
        Ok(_) => PublishResult {
            is_success: true,
            error_message: None,
            transaction_data: Some("DAG validation successful".to_string()),
        },
        Err(e) => PublishResult {
            is_success: false,
            error_message: Some(format!("DAG validation error: {}", e)),
            transaction_data: None,
        },
    }
}
