use {
    crate::{
        idents::{primitives, pure_arg, sui_framework, workflow},
        sui,
        types::{
            Dag,
            DataStorage,
            DefaultValue,
            Edge,
            EntryPort,
            FromPort,
            NexusObjects,
            Storable,
            StorageKind,
            Vertex,
            VertexKind,
            DEFAULT_ENTRY_GROUP,
        },
    },
    std::collections::HashMap,
};

/// PTB template for creating a new empty DAG.
pub fn empty(tx: &mut sui::tx::TransactionBuilder, objects: &NexusObjects) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::NEW.module,
            workflow::Dag::NEW.name,
            vec![],
        ),
        vec![],
    )
}

/// PTB template to publish a DAG.
pub fn publish(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
) -> sui::types::Argument {
    let dag_type = workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::DAG);

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
            vec![dag_type],
        ),
        vec![dag],
    )
}

/// PTB template to publish a full [`crate::types::Dag`].
pub fn create(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    mut dag_arg: sui::types::Argument,
    dag: Dag,
) -> anyhow::Result<sui::types::Argument> {
    // Create all vertices.
    for vertex in &dag.vertices {
        dag_arg = create_vertex(tx, objects, dag_arg, vertex)?;
    }

    // Create all default values if present.
    if let Some(default_values) = &dag.default_values {
        for default_value in default_values {
            dag_arg = create_default_value(tx, objects, dag_arg, default_value)?;
        }
    }

    // Create all edges.
    for edge in &dag.edges {
        dag_arg = create_edge(tx, objects, dag_arg, edge)?;
    }

    // Create all outputs.
    if let Some(outputs) = &dag.outputs {
        for output in outputs {
            dag_arg = create_output(tx, objects, dag_arg, output)?;
        }
    }

    // Create all entry ports and vertices. Or create a default entry group
    // with all specified entry ports if none is present.
    if let Some(entry_groups) = &dag.entry_groups {
        for entry_group in entry_groups {
            for vertex in &entry_group.vertices {
                let entry_ports = dag
                    .vertices
                    .iter()
                    .find(|v| &v.name == vertex)
                    .and_then(|v| v.entry_ports.as_ref());

                if let Some(entry_ports) = entry_ports {
                    for entry_port in entry_ports {
                        dag_arg = mark_entry_input_port(
                            tx,
                            objects,
                            dag_arg,
                            vertex,
                            entry_port,
                            &entry_group.name,
                        )?;
                    }
                } else {
                    dag_arg = mark_entry_vertex(tx, objects, dag_arg, vertex, &entry_group.name)?;
                }
            }
        }
    } else {
        for vertex in &dag.vertices {
            let Some(entry_ports) = vertex.entry_ports.as_ref() else {
                continue;
            };

            for entry_port in entry_ports {
                dag_arg = mark_entry_input_port(
                    tx,
                    objects,
                    dag_arg,
                    &vertex.name,
                    entry_port,
                    DEFAULT_ENTRY_GROUP,
                )?;
            }
        }
    }

    Ok(dag_arg)
}

/// PTB template for creating a new DAG vertex.
pub fn create_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    vertex: &Vertex,
) -> anyhow::Result<sui::types::Argument> {
    // `name: Vertex`
    let name = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, &vertex.name)?;

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            workflow::Dag::off_chain_vertex_kind_from_fqn(tx, objects.workflow_pkg_id, tool_fqn)?
        }
        VertexKind::OnChain { tool_fqn } => {
            workflow::Dag::on_chain_vertex_kind_from_fqn(tx, objects.workflow_pkg_id, tool_fqn)?
        }
    };

    // `dag.with_vertex(name, kind)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_VERTEX.module,
            workflow::Dag::WITH_VERTEX.name,
            vec![],
        ),
        vec![dag, name, kind],
    ))
}

/// PTB template for creating a new DAG default value.
pub fn create_default_value(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    default_value: &DefaultValue,
) -> anyhow::Result<sui::types::Argument> {
    // `vertex: Vertex`
    let vertex =
        workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, &default_value.vertex)?;

    // `port: InputPort`
    let port =
        workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, &default_value.input_port)?;

    // `value: NexusData`
    let value = match &default_value.value.storage {
        StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
            tx,
            objects.primitives_pkg_id,
            &default_value.value.data,
            // Default values cannot be secret. Sensitive data should be passed
            // via entry ports at runtime.
            false,
        )?,
        StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
            tx,
            objects.primitives_pkg_id,
            &default_value.value.data,
            // Default values cannot be secret. Sensitive data should be passed
            // via entry ports at runtime.
            false,
        )?,
    };

    // `dag.with_default_value(vertex, port, value)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_DEFAULT_VALUE.module,
            workflow::Dag::WITH_DEFAULT_VALUE.name,
            vec![],
        ),
        vec![dag, vertex, port, value],
    ))
}

/// PTB template for creating a new DAG edge.
pub fn create_edge(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    edge: &Edge,
) -> anyhow::Result<sui::types::Argument> {
    // `from_vertex: Vertex`
    let from_vertex =
        workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, &edge.from.vertex)?;

    // `from_variant: OutputVariant`
    let from_variant = workflow::Dag::output_variant_from_str(
        tx,
        objects.workflow_pkg_id,
        &edge.from.output_variant,
    )?;

    // `from_port: OutputPort`
    let from_port =
        workflow::Dag::output_port_from_str(tx, objects.workflow_pkg_id, &edge.from.output_port)?;

    // `to_vertex: Vertex`
    let to_vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, &edge.to.vertex)?;

    // `to_port: InputPort`
    let to_port =
        workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, &edge.to.input_port)?;

    // `kind: EdgeKind`
    let kind = workflow::Dag::edge_kind_from_enum(tx, objects.workflow_pkg_id, &edge.kind);

    if edge.from.encrypted {
        // `dag.with_encrypted_edge(from_vertex, from_variant, from_port, to_vertex, to_port)`
        return Ok(tx.move_call(
            sui::tx::Function::new(
                objects.workflow_pkg_id,
                workflow::Dag::WITH_ENCRYPTED_EDGE.module,
                workflow::Dag::WITH_ENCRYPTED_EDGE.name,
                vec![],
            ),
            vec![
                dag,
                from_vertex,
                from_variant,
                from_port,
                to_vertex,
                to_port,
                kind,
            ],
        ));
    }

    // `dag.with_edge(from_vertex, from_variant, from_port, encrypted, to_vertex, to_port)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_EDGE.module,
            workflow::Dag::WITH_EDGE.name,
            vec![],
        ),
        vec![
            dag,
            from_vertex,
            from_variant,
            from_port,
            to_vertex,
            to_port,
            kind,
        ],
    ))
}

/// PTB template for creating a new DAG edge.
pub fn create_output(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    output: &FromPort,
) -> anyhow::Result<sui::types::Argument> {
    // `vertex: Vertex`
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, &output.vertex)?;

    // `variant: OutputVariant`
    let variant = workflow::Dag::output_variant_from_str(
        tx,
        objects.workflow_pkg_id,
        &output.output_variant,
    )?;

    // `port: OutputPort`
    let port =
        workflow::Dag::output_port_from_str(tx, objects.workflow_pkg_id, &output.output_port)?;

    if output.encrypted {
        // `dag.with_encrypted_output(vertex, variant, port)`
        return Ok(tx.move_call(
            sui::tx::Function::new(
                objects.workflow_pkg_id,
                workflow::Dag::WITH_ENCRYPTED_OUTPUT.module,
                workflow::Dag::WITH_ENCRYPTED_OUTPUT.name,
                vec![],
            ),
            vec![dag, vertex, variant, port],
        ));
    }

    // `dag.with_output(vertex, variant, port)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_OUTPUT.module,
            workflow::Dag::WITH_OUTPUT.name,
            vec![],
        ),
        vec![dag, vertex, variant, port],
    ))
}

/// PTB template for marking a vertex as an entry vertex.
pub fn mark_entry_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    vertex: &str,
    entry_group: &str,
) -> anyhow::Result<sui::types::Argument> {
    // `vertex: Vertex`
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;

    // `entry_group: EntryGroup`
    let entry_group =
        workflow::Dag::entry_group_from_str(tx, objects.workflow_pkg_id, entry_group)?;

    // `dag.with_entry_in_group(vertex, entry_group)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_ENTRY_IN_GROUP.module,
            workflow::Dag::WITH_ENTRY_IN_GROUP.name,
            vec![],
        ),
        vec![dag, vertex, entry_group],
    ))
}

/// PTB template for marking an input port as an input port.
pub fn mark_entry_input_port(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    vertex: &str,
    entry_port: &EntryPort,
    entry_group: &str,
) -> anyhow::Result<sui::types::Argument> {
    // `vertex: Vertex`
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;

    // `entry_port: InputPort`
    let entry_port = if entry_port.encrypted {
        workflow::Dag::encrypted_input_port_from_str(tx, objects.workflow_pkg_id, &entry_port.name)?
    } else {
        workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, &entry_port.name)?
    };

    // `entry_group: EntryGroup`
    let entry_group =
        workflow::Dag::entry_group_from_str(tx, objects.workflow_pkg_id, entry_group)?;

    // `dag.with_entry_port_in_group(vertex, entry_port, entry_group)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.module,
            workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.name,
            vec![],
        ),
        vec![dag, vertex, entry_port, entry_group],
    ))
}

/// PTB template to execute a DAG.
pub fn execute(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut DefaultTAP`
    let default_tap = tx.input(sui::tx::Input::shared(
        *objects.default_tap.object_id(),
        objects.default_tap.version(),
        true,
    ));

    // `dag: &DAG`
    let dag = tx.input(sui::tx::Input::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));

    // `gas_service: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `network: ID`
    let network = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;

    // `entry_group: EntryGroup`
    let entry_group =
        workflow::Dag::entry_group_from_str(tx, objects.workflow_pkg_id, entry_group)?;

    // `with_vertex_inputs: VecMap<Vertex, VecMap<InputPort, NexusData>>`
    let inner_vec_map_type = vec![
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::INPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::VERTEX),
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::VEC_MAP.module,
            sui_framework::VecMap::VEC_MAP.name,
            inner_vec_map_type.clone(),
        ))),
    ];

    let with_vertex_inputs = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module,
            sui_framework::VecMap::EMPTY.name,
            outer_vec_map_type.clone(),
        ),
        vec![],
    );

    for (vertex_name, data) in input_data {
        // `vertex: Vertex`
        let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex_name)?;

        // `with_vertex_input: VecMap<InputPort, NexusData>`
        let with_vertex_input = tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::EMPTY.module,
                sui_framework::VecMap::EMPTY.name,
                inner_vec_map_type.clone(),
            ),
            vec![],
        );

        for (port_name, value) in data {
            // `port: InputPort`
            let port = match value.is_encrypted() {
                true => workflow::Dag::encrypted_input_port_from_str(
                    tx,
                    objects.workflow_pkg_id,
                    port_name.as_str(),
                )?,
                false => workflow::Dag::input_port_from_str(
                    tx,
                    objects.workflow_pkg_id,
                    port_name.as_str(),
                )?,
            };

            // `value: NexusData`
            let value = match value.storage_kind() {
                StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
                    value.is_encrypted(),
                )?,
                StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
                    value.is_encrypted(),
                )?,
            };

            // `with_vertex_input.insert(port, value)`
            tx.move_call(
                sui::tx::Function::new(
                    sui_framework::PACKAGE_ID,
                    sui_framework::VecMap::INSERT.module,
                    sui_framework::VecMap::INSERT.name,
                    inner_vec_map_type.clone(),
                ),
                vec![with_vertex_input, port, value],
            );
        }

        // `with_vertex_inputs.insert(vertex, with_vertex_input)`
        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
                outer_vec_map_type.clone(),
            ),
            vec![with_vertex_inputs, vertex, with_vertex_input],
        );
    }

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.input(pure_arg(&priority_fee_per_gas_unit)?);

    // `workflow::default_tap::begin_dag_execution()`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::DefaultTap::BEGIN_DAG_EXECUTION.module,
            workflow::DefaultTap::BEGIN_DAG_EXECUTION.name,
            vec![],
        ),
        vec![
            default_tap,
            dag,
            gas_service,
            network,
            entry_group,
            with_vertex_inputs,
            priority_fee_per_gas_unit,
            clock,
        ],
    ))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            fqn,
            test_utils::sui_mocks,
            types::{Data, EdgeKind, FromPort, ToPort},
        },
        std::collections::HashMap,
    };

    #[test]
    fn test_empty() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        empty(&mut tx, &objects);
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create an empty DAG");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::NEW.module);
        assert_eq!(call.function, workflow::Dag::NEW.name);
        assert_eq!(call.type_arguments.len(), 0);
        assert_eq!(call.arguments.len(), 0);
    }

    #[test]
    fn test_publish() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);

        let mut tx = sui::tx::TransactionBuilder::new();
        publish(&mut tx, &objects, dag);
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to publish a DAG");
        };

        assert_eq!(call.package, sui_framework::PACKAGE_ID);
        assert_eq!(
            call.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module
        );
        assert_eq!(
            call.function,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name
        );
        assert_eq!(call.type_arguments.len(), 1);
        assert_eq!(call.arguments.len(), 1);
    }

    #[test]
    fn test_create_vertex() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let vertex = Vertex {
            name: "vertex1".to_string(),
            kind: VertexKind::OffChain {
                tool_fqn: fqn!("xyz.tool.test@1"),
            },
            entry_ports: None,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        create_vertex(&mut tx, &objects, dag, &vertex).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create a vertex");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_VERTEX.module);
        assert_eq!(call.function, workflow::Dag::WITH_VERTEX.name);
    }

    #[test]
    fn test_create_default_value() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let default_value = DefaultValue {
            vertex: "vertex1".to_string(),
            input_port: "port1".to_string(),
            value: Data {
                storage: StorageKind::Inline,
                data: serde_json::json!({"key": "value"}),
            },
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        create_default_value(&mut tx, &objects, dag, &default_value).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create a default value");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_DEFAULT_VALUE.module);
        assert_eq!(call.function, workflow::Dag::WITH_DEFAULT_VALUE.name);
    }

    #[test]
    fn test_create_edge() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let edge = Edge {
            from: FromPort {
                vertex: "vertex1".to_string(),
                output_variant: "variant1".to_string(),
                output_port: "port1".to_string(),
                encrypted: false,
            },
            to: ToPort {
                vertex: "vertex2".to_string(),
                input_port: "port2".to_string(),
            },
            kind: EdgeKind::Normal,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        create_edge(&mut tx, &objects, dag, &edge).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create an edge");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_EDGE.module);
        assert_eq!(call.function, workflow::Dag::WITH_EDGE.name);
    }

    #[test]
    fn test_mark_entry_vertex() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let vertex = "vertex1";
        let entry_group = "group1";

        let mut tx = sui::tx::TransactionBuilder::new();
        mark_entry_vertex(&mut tx, &objects, dag, vertex, entry_group).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to mark an entry vertex");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_ENTRY_IN_GROUP.module);
        assert_eq!(call.function, workflow::Dag::WITH_ENTRY_IN_GROUP.name);
    }

    #[test]
    fn test_mark_entry_input_port() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let vertex = "vertex1";
        let entry_port = &EntryPort {
            name: "test".to_string(),
            encrypted: false,
        };
        let entry_group = "group1";

        let mut tx = sui::tx::TransactionBuilder::new();
        mark_entry_input_port(
            &mut tx,
            &nexus_objects,
            dag,
            vertex,
            entry_port,
            entry_group,
        )
        .unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to mark an entry input port");
        };

        assert_eq!(call.package, nexus_objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.module);
        assert_eq!(call.function, workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.name);
    }

    #[test]
    fn test_execute() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let entry_group = "group1";
        let mut input_data = HashMap::new();

        input_data.insert(
            "vertex1".to_string(),
            HashMap::from([(
                "port1".to_string(),
                serde_json::json!({"kind": "inline", "encryption_mode": 0, "data": { "key": "value"} })
                    .try_into()
                    .expect("Failed to convert JSON to DataStorage"),
            )]),
        );

        let mut tx = sui::tx::TransactionBuilder::new();
        let priority_fee_per_gas_unit = 0;
        execute(
            &mut tx,
            &nexus_objects,
            &dag,
            priority_fee_per_gas_unit,
            entry_group,
            &input_data,
        )
        .unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to execute a DAG");
        };

        assert_eq!(call.package, nexus_objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultTap::BEGIN_DAG_EXECUTION.module
        );
        assert_eq!(
            call.function,
            workflow::DefaultTap::BEGIN_DAG_EXECUTION.name
        );
    }

    #[test]
    fn test_create_output_unencrypted() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let output = FromPort {
            vertex: "vertex1".to_string(),
            output_variant: "variant1".to_string(),
            output_port: "port1".to_string(),
            encrypted: false,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        create_output(&mut tx, &objects, dag, &output).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create an output");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_OUTPUT.module);
        assert_eq!(call.function, workflow::Dag::WITH_OUTPUT.name);
    }

    #[test]
    fn test_create_output_encrypted() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let output = FromPort {
            vertex: "vertex1".to_string(),
            output_variant: "variant1".to_string(),
            output_port: "port1".to_string(),
            encrypted: true,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        create_output(&mut tx, &objects, dag, &output).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create an encrypted output");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_ENCRYPTED_OUTPUT.module);
        assert_eq!(call.function, workflow::Dag::WITH_ENCRYPTED_OUTPUT.name);
    }
}
