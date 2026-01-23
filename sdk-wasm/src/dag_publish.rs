use {
    nexus_sdk::{
        dag::validator,
        types::{Dag, EdgeKind, StorageKind, VertexKind},
    },
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    wasm_bindgen::prelude::*,
};

/// Transaction command for JS execution
#[derive(Serialize, Deserialize)]
pub struct TransactionCommand {
    #[serde(rename = "type")]
    pub command_type: String,
    pub target: String,
    pub arguments: Vec<CommandArgument>,
    #[serde(rename = "typeArguments")]
    pub type_arguments: Vec<String>,
    pub result_index: usize,
}

/// Command argument types
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CommandArgument {
    #[serde(rename = "pure")]
    Pure {
        pure_type: String,
        value: serde_json::Value,
    },
    #[serde(rename = "object")]
    Object { value: String },
    #[serde(rename = "result")]
    Result { index: usize },
}

/// DAG publish transaction response
#[derive(Serialize, Deserialize)]
pub struct DagPublishTransactionData {
    pub operation_type: String,
    pub commands: Vec<TransactionCommand>,
    pub dag_summary: DagSummary,
}

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
    pub operation: String,
    pub description: String,
    pub sdk_function: String,
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

#[wasm_bindgen]
pub fn build_dag_publish_transaction(dag_json: &str, nexus_objects_json: &str) -> PublishResult {
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

    // Parse Nexus objects
    let nexus_objects: HashMap<String, String> = match serde_json::from_str(nexus_objects_json) {
        Ok(objects) => objects,
        Err(e) => {
            return PublishResult {
                is_success: false,
                error_message: Some(format!("Nexus objects JSON parsing error: {}", e)),
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

    // Build transaction commands
    let mut commands = Vec::new();
    let mut result_index = 0;

    // Command 1: Create empty DAG
    commands.push(TransactionCommand {
        command_type: "moveCall".to_string(),
        target: format!(
            "{}::dag::new",
            nexus_objects
                .get("workflow_pkg_id")
                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
        ),
        arguments: vec![],
        type_arguments: vec![],
        result_index,
    });
    let dag_result_index = result_index;
    result_index += 1;

    // Track the current DAG reference
    let mut current_dag_index = dag_result_index;

    // Command 2-N: Add vertices
    for vertex in &dag.vertices {
        // Create vertex name
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::vertex_from_string",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            ),
            arguments: vec![CommandArgument::Pure {
                pure_type: "string".to_string(),
                value: serde_json::Value::String(vertex.name.clone()),
            }],
            type_arguments: vec![],
            result_index,
        });
        let vertex_name_index = result_index;
        result_index += 1;

        // Create vertex kind - supports both OffChain and OnChain (v0.5.0)
        let vertex_kind_index = match &vertex.kind {
            VertexKind::OffChain { tool_fqn } => {
                commands.push(TransactionCommand {
                    command_type: "moveCall".to_string(),
                    target: format!(
                        "{}::dag::vertex_off_chain",
                        nexus_objects
                            .get("workflow_pkg_id")
                            .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                    ),
                    arguments: vec![CommandArgument::Pure {
                        pure_type: "string".to_string(),
                        value: serde_json::Value::String(tool_fqn.to_string()),
                    }],
                    type_arguments: vec![],
                    result_index,
                });
                let idx = result_index;
                result_index += 1;
                idx
            }
            VertexKind::OnChain { tool_fqn } => {
                // OnChain vertex requires tool_registry lookup
                commands.push(TransactionCommand {
                    command_type: "moveCall".to_string(),
                    target: format!(
                        "{}::dag::vertex_on_chain",
                        nexus_objects
                            .get("workflow_pkg_id")
                            .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                    ),
                    arguments: vec![
                        CommandArgument::Object {
                            value: nexus_objects
                                .get("tool_registry_id")
                                .cloned()
                                .unwrap_or_else(|| "{{tool_registry_id}}".to_string()),
                        },
                        CommandArgument::Pure {
                            pure_type: "string".to_string(),
                            value: serde_json::Value::String(tool_fqn.to_string()),
                        },
                    ],
                    type_arguments: vec![],
                    result_index,
                });
                let idx = result_index;
                result_index += 1;
                idx
            }
        };

        // Add vertex to DAG
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::with_vertex",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            ),
            arguments: vec![
                CommandArgument::Result {
                    index: current_dag_index,
                },
                CommandArgument::Result {
                    index: vertex_name_index,
                },
                CommandArgument::Result {
                    index: vertex_kind_index,
                },
            ],
            type_arguments: vec![],
            result_index,
        });
        // Update DAG reference for the next iteration
        current_dag_index = result_index;
        result_index += 1;
    }

    // Add default values
    if let Some(default_values) = &dag.default_values {
        for default_value in default_values {
            // Create vertex argument
            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: format!(
                    "{}::dag::vertex_from_string",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                ),
                arguments: vec![CommandArgument::Pure {
                    pure_type: "string".to_string(),
                    value: serde_json::Value::String(default_value.vertex.clone()),
                }],
                type_arguments: vec![],
                result_index,
            });
            let vertex_index = result_index;
            result_index += 1;

            // Create input port argument
            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: format!(
                    "{}::dag::input_port_from_string",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                ),
                arguments: vec![CommandArgument::Pure {
                    pure_type: "string".to_string(),
                    value: serde_json::Value::String(default_value.input_port.clone()),
                }],
                type_arguments: vec![],
                result_index,
            });
            let input_port_index = result_index;
            result_index += 1;

            // Create nexus data - supports both Inline and Walrus storage (v0.5.0)
            let json_string = serde_json::to_string(&default_value.value.data).unwrap_or_default();
            let json_bytes: Vec<u8> = json_string.into_bytes();

            // Determine storage type from the Data struct
            let is_walrus = matches!(default_value.value.storage, StorageKind::Walrus);
            let data_target = if is_walrus {
                format!(
                    "{}::data::walrus_one",
                    nexus_objects
                        .get("primitives_pkg_id")
                        .unwrap_or(&"{{primitives_pkg_id}}".to_string())
                )
            } else {
                format!(
                    "{}::data::inline_one",
                    nexus_objects
                        .get("primitives_pkg_id")
                        .unwrap_or(&"{{primitives_pkg_id}}".to_string())
                )
            };

            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: data_target,
                arguments: vec![CommandArgument::Pure {
                    pure_type: "vector_u8".to_string(),
                    value: serde_json::Value::Array(
                        json_bytes
                            .into_iter()
                            .map(|b| serde_json::Value::Number(serde_json::Number::from(b)))
                            .collect(),
                    ),
                }],
                type_arguments: vec![],
                result_index,
            });
            let nexus_data_index = result_index;
            result_index += 1;

            // Add default value to DAG
            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: format!(
                    "{}::dag::with_default_value",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                ),
                arguments: vec![
                    CommandArgument::Result {
                        index: current_dag_index,
                    },
                    CommandArgument::Result {
                        index: vertex_index,
                    },
                    CommandArgument::Result {
                        index: input_port_index,
                    },
                    CommandArgument::Result {
                        index: nexus_data_index,
                    },
                ],
                type_arguments: vec![],
                result_index,
            });
            current_dag_index = result_index;
            result_index += 1;
        }
    }

    // Add edges
    for edge in &dag.edges {
        // Create from vertex argument
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::vertex_from_string",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            ),
            arguments: vec![CommandArgument::Pure {
                pure_type: "string".to_string(),
                value: serde_json::Value::String(edge.from.vertex.clone()),
            }],
            type_arguments: vec![],
            result_index,
        });
        let from_vertex_index = result_index;
        result_index += 1;

        // Create from output variant argument
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::output_variant_from_string",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            ),
            arguments: vec![CommandArgument::Pure {
                pure_type: "string".to_string(),
                value: serde_json::Value::String(edge.from.output_variant.clone()),
            }],
            type_arguments: vec![],
            result_index,
        });
        let from_output_variant_index = result_index;
        result_index += 1;

        // Create from output port argument
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::output_port_from_string",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            ),
            arguments: vec![CommandArgument::Pure {
                pure_type: "string".to_string(),
                value: serde_json::Value::String(edge.from.output_port.clone()),
            }],
            type_arguments: vec![],
            result_index,
        });
        let from_output_port_index = result_index;
        result_index += 1;

        // Create to vertex argument
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::vertex_from_string",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            ),
            arguments: vec![CommandArgument::Pure {
                pure_type: "string".to_string(),
                value: serde_json::Value::String(edge.to.vertex.clone()),
            }],
            type_arguments: vec![],
            result_index,
        });
        let to_vertex_index = result_index;
        result_index += 1;

        // Create to input port argument
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::input_port_from_string",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            ),
            arguments: vec![CommandArgument::Pure {
                pure_type: "string".to_string(),
                value: serde_json::Value::String(edge.to.input_port.clone()),
            }],
            type_arguments: vec![],
            result_index,
        });
        let to_input_port_index = result_index;
        result_index += 1;

        // Create edge kind argument
        let edge_kind_str = match edge.kind {
            EdgeKind::Normal => "normal",
            EdgeKind::ForEach => "for_each",
            EdgeKind::Collect => "collect",
            EdgeKind::DoWhile => "do_while",
            EdgeKind::Break => "break",
        };
        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: format!(
                "{}::dag::edge_kind_{}",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string()),
                edge_kind_str
            ),
            arguments: vec![],
            type_arguments: vec![],
            result_index,
        });
        let edge_kind_index = result_index;
        result_index += 1;

        // Add edge to DAG (with encrypted edge support)
        let edge_target = if edge.from.encrypted {
            format!(
                "{}::dag::with_encrypted_edge",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            )
        } else {
            format!(
                "{}::dag::with_edge",
                nexus_objects
                    .get("workflow_pkg_id")
                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
            )
        };

        commands.push(TransactionCommand {
            command_type: "moveCall".to_string(),
            target: edge_target,
            arguments: vec![
                CommandArgument::Result {
                    index: current_dag_index,
                },
                CommandArgument::Result {
                    index: from_vertex_index,
                },
                CommandArgument::Result {
                    index: from_output_variant_index,
                },
                CommandArgument::Result {
                    index: from_output_port_index,
                },
                CommandArgument::Result {
                    index: to_vertex_index,
                },
                CommandArgument::Result {
                    index: to_input_port_index,
                },
                CommandArgument::Result {
                    index: edge_kind_index,
                },
            ],
            type_arguments: vec![],
            result_index,
        });
        current_dag_index = result_index;
        result_index += 1;
    }

    // Add outputs (SDK order: outputs before entry groups)
    if let Some(outputs) = &dag.outputs {
        for output in outputs {
            // Create vertex argument
            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: format!(
                    "{}::dag::vertex_from_string",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                ),
                arguments: vec![CommandArgument::Pure {
                    pure_type: "string".to_string(),
                    value: serde_json::Value::String(output.vertex.clone()),
                }],
                type_arguments: vec![],
                result_index,
            });
            let vertex_index = result_index;
            result_index += 1;

            // Create output variant argument
            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: format!(
                    "{}::dag::output_variant_from_string",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                ),
                arguments: vec![CommandArgument::Pure {
                    pure_type: "string".to_string(),
                    value: serde_json::Value::String(output.output_variant.clone()),
                }],
                type_arguments: vec![],
                result_index,
            });
            let output_variant_index = result_index;
            result_index += 1;

            // Create output port argument
            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: format!(
                    "{}::dag::output_port_from_string",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                ),
                arguments: vec![CommandArgument::Pure {
                    pure_type: "string".to_string(),
                    value: serde_json::Value::String(output.output_port.clone()),
                }],
                type_arguments: vec![],
                result_index,
            });
            let output_port_index = result_index;
            result_index += 1;

            // Add output to DAG (with encrypted support like SDK)
            let output_target = if output.encrypted {
                format!(
                    "{}::dag::with_encrypted_output",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                )
            } else {
                format!(
                    "{}::dag::with_output",
                    nexus_objects
                        .get("workflow_pkg_id")
                        .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                )
            };

            commands.push(TransactionCommand {
                command_type: "moveCall".to_string(),
                target: output_target,
                arguments: vec![
                    CommandArgument::Result {
                        index: current_dag_index,
                    },
                    CommandArgument::Result {
                        index: vertex_index,
                    },
                    CommandArgument::Result {
                        index: output_variant_index,
                    },
                    CommandArgument::Result {
                        index: output_port_index,
                    },
                ],
                type_arguments: vec![],
                result_index,
            });
            current_dag_index = result_index;
            result_index += 1;
        }
    }

    // Add entry ports and vertices (SDK order: after outputs)
    if let Some(entry_groups) = &dag.entry_groups {
        for entry_group in entry_groups {
            for vertex_name in &entry_group.vertices {
                let entry_ports = dag
                    .vertices
                    .iter()
                    .find(|v| &v.name == vertex_name)
                    .and_then(|v| v.entry_ports.as_ref());

                if let Some(entry_ports) = entry_ports {
                    for entry_port in entry_ports {
                        // Create vertex argument
                        commands.push(TransactionCommand {
                            command_type: "moveCall".to_string(),
                            target: format!(
                                "{}::dag::vertex_from_string",
                                nexus_objects
                                    .get("workflow_pkg_id")
                                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                            ),
                            arguments: vec![CommandArgument::Pure {
                                pure_type: "string".to_string(),
                                value: serde_json::Value::String(vertex_name.clone()),
                            }],
                            type_arguments: vec![],
                            result_index,
                        });
                        let vertex_index = result_index;
                        result_index += 1;

                        // Create entry port argument
                        let entry_port_target = if entry_port.encrypted {
                            format!(
                                "{}::dag::encrypted_input_port_from_string",
                                nexus_objects
                                    .get("workflow_pkg_id")
                                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                            )
                        } else {
                            format!(
                                "{}::dag::input_port_from_string",
                                nexus_objects
                                    .get("workflow_pkg_id")
                                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                            )
                        };

                        commands.push(TransactionCommand {
                            command_type: "moveCall".to_string(),
                            target: entry_port_target,
                            arguments: vec![CommandArgument::Pure {
                                pure_type: "string".to_string(),
                                value: serde_json::Value::String(entry_port.name.clone()),
                            }],
                            type_arguments: vec![],
                            result_index,
                        });
                        let entry_port_index = result_index;
                        result_index += 1;

                        // Create entry group argument
                        commands.push(TransactionCommand {
                            command_type: "moveCall".to_string(),
                            target: format!(
                                "{}::dag::entry_group_from_string",
                                nexus_objects
                                    .get("workflow_pkg_id")
                                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                            ),
                            arguments: vec![CommandArgument::Pure {
                                pure_type: "string".to_string(),
                                value: serde_json::Value::String(entry_group.name.clone()),
                            }],
                            type_arguments: vec![],
                            result_index,
                        });
                        let entry_group_index = result_index;
                        result_index += 1;

                        // Add entry port to DAG
                        commands.push(TransactionCommand {
                            command_type: "moveCall".to_string(),
                            target: format!(
                                "{}::dag::with_entry_port_in_group",
                                nexus_objects
                                    .get("workflow_pkg_id")
                                    .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                            ),
                            arguments: vec![
                                CommandArgument::Result {
                                    index: current_dag_index,
                                },
                                CommandArgument::Result {
                                    index: vertex_index,
                                },
                                CommandArgument::Result {
                                    index: entry_port_index,
                                },
                                CommandArgument::Result {
                                    index: entry_group_index,
                                },
                            ],
                            type_arguments: vec![],
                            result_index,
                        });
                        current_dag_index = result_index;
                        result_index += 1;
                    }
                } else {
                    // Mark entry vertex (no entry ports)
                    // Create vertex argument
                    commands.push(TransactionCommand {
                        command_type: "moveCall".to_string(),
                        target: format!(
                            "{}::dag::vertex_from_string",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        ),
                        arguments: vec![CommandArgument::Pure {
                            pure_type: "string".to_string(),
                            value: serde_json::Value::String(vertex_name.clone()),
                        }],
                        type_arguments: vec![],
                        result_index,
                    });
                    let vertex_index = result_index;
                    result_index += 1;

                    // Create entry group argument
                    commands.push(TransactionCommand {
                        command_type: "moveCall".to_string(),
                        target: format!(
                            "{}::dag::entry_group_from_string",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        ),
                        arguments: vec![CommandArgument::Pure {
                            pure_type: "string".to_string(),
                            value: serde_json::Value::String(entry_group.name.clone()),
                        }],
                        type_arguments: vec![],
                        result_index,
                    });
                    let entry_group_index = result_index;
                    result_index += 1;

                    // Add entry vertex to DAG
                    commands.push(TransactionCommand {
                        command_type: "moveCall".to_string(),
                        target: format!(
                            "{}::dag::with_entry_in_group",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        ),
                        arguments: vec![
                            CommandArgument::Result {
                                index: current_dag_index,
                            },
                            CommandArgument::Result {
                                index: vertex_index,
                            },
                            CommandArgument::Result {
                                index: entry_group_index,
                            },
                        ],
                        type_arguments: vec![],
                        result_index,
                    });
                    current_dag_index = result_index;
                    result_index += 1;
                }
            }
        }
    } else {
        // Handle default entry group (like CLI)
        for vertex in &dag.vertices {
            if let Some(entry_ports) = &vertex.entry_ports {
                for entry_port in entry_ports {
                    // Create vertex argument
                    commands.push(TransactionCommand {
                        command_type: "moveCall".to_string(),
                        target: format!(
                            "{}::dag::vertex_from_string",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        ),
                        arguments: vec![CommandArgument::Pure {
                            pure_type: "string".to_string(),
                            value: serde_json::Value::String(vertex.name.clone()),
                        }],
                        type_arguments: vec![],
                        result_index,
                    });
                    let vertex_index = result_index;
                    result_index += 1;

                    // Create entry port argument
                    let entry_port_target = if entry_port.encrypted {
                        format!(
                            "{}::dag::encrypted_input_port_from_string",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        )
                    } else {
                        format!(
                            "{}::dag::input_port_from_string",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        )
                    };

                    commands.push(TransactionCommand {
                        command_type: "moveCall".to_string(),
                        target: entry_port_target,
                        arguments: vec![CommandArgument::Pure {
                            pure_type: "string".to_string(),
                            value: serde_json::Value::String(entry_port.name.clone()),
                        }],
                        type_arguments: vec![],
                        result_index,
                    });
                    let entry_port_index = result_index;
                    result_index += 1;

                    // Create entry group argument (DEFAULT_ENTRY_GROUP)
                    commands.push(TransactionCommand {
                        command_type: "moveCall".to_string(),
                        target: format!(
                            "{}::dag::entry_group_from_string",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        ),
                        arguments: vec![CommandArgument::Pure {
                            pure_type: "string".to_string(),
                            value: serde_json::Value::String("_default_group".to_string()),
                        }],
                        type_arguments: vec![],
                        result_index,
                    });
                    let entry_group_index = result_index;
                    result_index += 1;

                    // Add entry port to DAG
                    commands.push(TransactionCommand {
                        command_type: "moveCall".to_string(),
                        target: format!(
                            "{}::dag::with_entry_port_in_group",
                            nexus_objects
                                .get("workflow_pkg_id")
                                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
                        ),
                        arguments: vec![
                            CommandArgument::Result {
                                index: current_dag_index,
                            },
                            CommandArgument::Result {
                                index: vertex_index,
                            },
                            CommandArgument::Result {
                                index: entry_port_index,
                            },
                            CommandArgument::Result {
                                index: entry_group_index,
                            },
                        ],
                        type_arguments: vec![],
                        result_index,
                    });
                    current_dag_index = result_index;
                    result_index += 1;
                }
            }
        }
    }

    // Final command: Publish DAG
    commands.push(TransactionCommand {
        command_type: "moveCall".to_string(),
        target: "0x2::transfer::public_share_object".to_string(),
        arguments: vec![CommandArgument::Result {
            index: current_dag_index,
        }],
        type_arguments: vec![format!(
            "{}::dag::DAG",
            nexus_objects
                .get("workflow_pkg_id")
                .unwrap_or(&"{{workflow_pkg_id}}".to_string())
        )],
        result_index,
    });

    // Create DAG summary
    let dag_summary = DagSummary {
        vertices_count: dag.vertices.len(),
        edges_count: dag.edges.len(),
        has_entry_groups: dag.entry_groups.is_some(),
        has_default_values: dag.default_values.is_some(),
        has_outputs: dag.outputs.is_some(),
    };

    // Create final transaction
    let transaction = DagPublishTransactionData {
        operation_type: "dag_publish_transaction".to_string(),
        commands,
        dag_summary,
    };

    // Serialize and return
    match serde_json::to_string(&transaction) {
        Ok(serialized) => PublishResult {
            is_success: true,
            error_message: None,
            transaction_data: Some(serialized),
        },
        Err(e) => PublishResult {
            is_success: false,
            error_message: Some(format!("Transaction serialization error: {}", e)),
            transaction_data: None,
        },
    }
}
