use crate::{
    idents::{primitives, sui_framework, workflow},
    sui,
    types::{Dag, Data, DefaultValue, Edge, NexusObjects, Vertex, VertexKind, DEFAULT_ENTRY_GROUP},
};

/// PTB template for creating a new empty DAG.
pub fn empty(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
) -> sui::Argument {
    tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Dag::NEW.module.into(),
        workflow::Dag::NEW.name.into(),
        vec![],
        vec![],
    )
}

/// PTB template to publish a DAG.
pub fn publish(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag: sui::Argument,
) -> sui::Argument {
    let dag_type = workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::DAG);

    tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module.into(),
        sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name.into(),
        vec![dag_type],
        vec![dag],
    )
}

/// PTB template to publish a full [`crate::types::Dag`].
pub fn create(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    mut dag_arg: sui::Argument,
    dag: Dag,
) -> anyhow::Result<sui::Argument> {
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
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag: sui::Argument,
    vertex: &Vertex,
) -> anyhow::Result<sui::Argument> {
    // `name: Vertex`
    let name = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, &vertex.name)?;

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            // `tool_fqn: AsciiString`
            workflow::Dag::off_chain_vertex_kind_from_fqn(tx, objects.workflow_pkg_id, tool_fqn)?
        }
        VertexKind::OnChain { .. } => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
        }
    };

    // `dag.with_vertex(name, kind)`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Dag::WITH_VERTEX.module.into(),
        workflow::Dag::WITH_VERTEX.name.into(),
        vec![],
        vec![dag, name, kind],
    ))
}

/// PTB template for creating a new DAG default value.
pub fn create_default_value(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag: sui::Argument,
    default_value: &DefaultValue,
) -> anyhow::Result<sui::Argument> {
    // `vertex: Vertex`
    let vertex =
        workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, &default_value.vertex)?;

    // `port: InputPort`
    let port =
        workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, &default_value.input_port)?;

    // `value: NexusData`
    let value = match &default_value.value {
        Data::Inline { data } => {
            primitives::Data::nexus_data_from_json(tx, objects.primitives_pkg_id, data)?
        }
        // Allowing to remind us that any other data storages can be added here.
        #[allow(unreachable_patterns)]
        _ => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/30>")
        }
    };

    // `dag.with_default_value(vertex, port, value)`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Dag::WITH_DEFAULT_VALUE.module.into(),
        workflow::Dag::WITH_DEFAULT_VALUE.name.into(),
        vec![],
        vec![dag, vertex, port, value],
    ))
}

/// PTB template for creating a new DAG edge.
pub fn create_edge(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag: sui::Argument,
    edge: &Edge,
) -> anyhow::Result<sui::Argument> {
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

    // `dag.with_edge(frpm_vertex, from_variant, from_port, to_vertex, to_port)`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Dag::WITH_EDGE.module.into(),
        workflow::Dag::WITH_EDGE.name.into(),
        vec![],
        vec![
            dag,
            from_vertex,
            from_variant,
            from_port,
            to_vertex,
            to_port,
        ],
    ))
}

/// PTB template for marking a vertex as an entry vertex.
pub fn mark_entry_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag: sui::Argument,
    vertex: &str,
    entry_group: &str,
) -> anyhow::Result<sui::Argument> {
    // `vertex: Vertex`
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;

    // `entry_group: EntryGroup`
    let entry_group =
        workflow::Dag::entry_group_from_str(tx, objects.workflow_pkg_id, entry_group)?;

    // `dag.with_entry_in_group(vertex, entry_group)`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Dag::WITH_ENTRY_IN_GROUP.module.into(),
        workflow::Dag::WITH_ENTRY_IN_GROUP.name.into(),
        vec![],
        vec![dag, vertex, entry_group],
    ))
}

/// PTB template for marking an input port as an input port.
pub fn mark_entry_input_port(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag: sui::Argument,
    vertex: &str,
    entry_port: &str,
    entry_group: &str,
) -> anyhow::Result<sui::Argument> {
    // `vertex: Vertex`
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;

    // `entry_port: InputPort`
    let entry_port = workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, entry_port)?;

    // `entry_group: EntryGroup`
    let entry_group =
        workflow::Dag::entry_group_from_str(tx, objects.workflow_pkg_id, entry_group)?;

    // `dag.with_entry_port_in_group(vertex, entry_port, entry_group)`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.module.into(),
        workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.name.into(),
        vec![],
        vec![dag, vertex, entry_port, entry_group],
    ))
}

/// PTB template to execute a DAG.
pub fn execute(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::ObjectRef,
    entry_group: &str,
    input_json: serde_json::Value,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut DefaultSAP`
    let default_sap = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.default_sap.object_id,
        initial_shared_version: objects.default_sap.version,
        mutable: true,
    })?;

    // `dag: &DAG`
    let dag = tx.obj(sui::ObjectArg::SharedObject {
        id: dag.object_id,
        initial_shared_version: dag.version,
        mutable: false,
    })?;

    // `gas_service: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

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
        sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::FRAMEWORK_PACKAGE_ID,
            module: sui_framework::VecMap::VEC_MAP.module.into(),
            name: sui_framework::VecMap::VEC_MAP.name.into(),
            type_params: inner_vec_map_type.clone(),
        })),
    ];

    let with_vertex_inputs = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::VecMap::EMPTY.module.into(),
        sui_framework::VecMap::EMPTY.name.into(),
        outer_vec_map_type.clone(),
        vec![],
    );

    let Some(data) = input_json.as_object() else {
        anyhow::bail!(
            "Input JSON must be an object containing the entry vertices and their respective data."
        );
    };

    for (vertex, data) in data {
        let Some(data) = data.as_object() else {
            anyhow::bail!(
                "Values of input JSON must be an object containing the input ports and their respective values."
            );
        };

        // `vertex: Vertex`
        let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;

        // `with_vertex_input: VecMap<InputPort, NexusData>`
        let with_vertex_input = tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module.into(),
            sui_framework::VecMap::EMPTY.name.into(),
            inner_vec_map_type.clone(),
            vec![],
        );

        for (port, value) in data {
            // `port: InputPort`
            let port = workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, port)?;

            // `value: NexusData`
            let value =
                primitives::Data::nexus_data_from_json(tx, objects.primitives_pkg_id, value)?;

            // `with_vertex_input.insert(port, value)`
            tx.programmable_move_call(
                sui::FRAMEWORK_PACKAGE_ID,
                sui_framework::VecMap::INSERT.module.into(),
                sui_framework::VecMap::INSERT.name.into(),
                inner_vec_map_type.clone(),
                vec![with_vertex_input, port, value],
            );
        }

        // `with_vertex_inputs.insert(vertex, with_vertex_input)`
        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::INSERT.module.into(),
            sui_framework::VecMap::INSERT.name.into(),
            outer_vec_map_type.clone(),
            vec![with_vertex_inputs, vertex, with_vertex_input],
        );
    }

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    // `workflow::default_sap::begin_dag_execution()`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultSap::BEGIN_DAG_EXECUTION.module.into(),
        workflow::DefaultSap::BEGIN_DAG_EXECUTION.name.into(),
        vec![],
        vec![
            default_sap,
            dag,
            gas_service,
            network,
            entry_group,
            with_vertex_inputs,
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
            types::{FromPort, ToPort},
        },
    };

    #[test]
    fn test_empty() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        empty(&mut tx, &objects);
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create an empty DAG");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::NEW.module.to_string(),);
        assert_eq!(call.function, workflow::Dag::NEW.name.to_string());
        assert_eq!(call.type_arguments.len(), 0);
        assert_eq!(call.arguments.len(), 0);
    }

    #[test]
    fn test_publish() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::Argument::Result(0);

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        publish(&mut tx, &objects, dag);
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to publish a DAG");
        };

        assert_eq!(call.package, sui::FRAMEWORK_PACKAGE_ID);
        assert_eq!(
            call.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT
                .name
                .to_string()
        );
        assert_eq!(call.type_arguments.len(), 1);
        assert_eq!(call.arguments.len(), 1);
    }

    #[test]
    fn test_create_vertex() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::Argument::Result(0);
        let vertex = Vertex {
            name: "vertex1".to_string(),
            kind: VertexKind::OffChain {
                tool_fqn: fqn!("xyz.tool.test@1"),
            },
            entry_ports: None,
        };

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        create_vertex(&mut tx, &objects, dag, &vertex).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create a vertex");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_VERTEX.module.to_string(),);
        assert_eq!(call.function, workflow::Dag::WITH_VERTEX.name.to_string());
    }

    #[test]
    fn test_create_default_value() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::Argument::Result(0);
        let default_value = DefaultValue {
            vertex: "vertex1".to_string(),
            input_port: "port1".to_string(),
            value: Data::Inline {
                data: serde_json::json!({"key": "value"}),
            },
        };

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        create_default_value(&mut tx, &objects, dag, &default_value).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create a default value");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::WITH_DEFAULT_VALUE.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::Dag::WITH_DEFAULT_VALUE.name.to_string()
        );
    }

    #[test]
    fn test_create_edge() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::Argument::Result(0);
        let edge = Edge {
            from: FromPort {
                vertex: "vertex1".to_string(),
                output_variant: "variant1".to_string(),
                output_port: "port1".to_string(),
            },
            to: ToPort {
                vertex: "vertex2".to_string(),
                input_port: "port2".to_string(),
            },
        };

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        create_edge(&mut tx, &objects, dag, &edge).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create an edge");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_EDGE.module.to_string(),);
        assert_eq!(call.function, workflow::Dag::WITH_EDGE.name.to_string());
    }

    #[test]
    fn test_mark_entry_vertex() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::Argument::Result(0);
        let vertex = "vertex1";
        let entry_group = "group1";

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        mark_entry_vertex(&mut tx, &objects, dag, vertex, entry_group).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to mark an entry vertex");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::WITH_ENTRY_IN_GROUP.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::Dag::WITH_ENTRY_IN_GROUP.name.to_string()
        );
    }

    #[test]
    fn test_mark_entry_input_port() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui::Argument::Result(0);
        let vertex = "vertex1";
        let entry_port = "port1";
        let entry_group = "group1";

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        mark_entry_input_port(
            &mut tx,
            &nexus_objects,
            dag,
            vertex,
            entry_port,
            entry_group,
        )
        .unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to mark an entry input port");
        };

        assert_eq!(call.package, nexus_objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::Dag::WITH_ENTRY_PORT_IN_GROUP.name.to_string()
        );
    }

    #[test]
    fn test_execute() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let entry_group = "group1";
        let input_json = serde_json::json!({
            "vertex1": {
                "port1": {"key": "value"}
            }
        });

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        execute(&mut tx, &nexus_objects, &dag, entry_group, input_json).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to execute a DAG");
        };

        assert_eq!(call.package, nexus_objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultSap::BEGIN_DAG_EXECUTION.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::DefaultSap::BEGIN_DAG_EXECUTION.name.to_string()
        );
    }
}
