use crate::{
    idents::{move_std, primitives, sui_framework, workflow},
    sui,
    types::{Data, DefaultValue, Edge, EntryVertex, Vertex, VertexKind},
};

/// PTB template for creating a new empty DAG.
pub fn empty(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
) -> sui::Argument {
    tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::NEW.module.into(),
        workflow::Dag::NEW.name.into(),
        vec![],
        vec![],
    )
}

/// PTB template to publish a DAG.
pub fn publish(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
) -> sui::Argument {
    let dag_type = workflow::into_type_tag(workflow_pkg_id, workflow::Dag::DAG);

    tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module.into(),
        sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name.into(),
        vec![dag_type],
        vec![dag],
    )
}

/// PTB template for creating a new DAG entry vertex.
pub fn create_entry_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    vertex: EntryVertex,
    groups: Vec<String>,
) -> anyhow::Result<sui::Argument> {
    // `entry_groups: vector<EntryGroup>`
    let entry_group_type = workflow::into_type_tag(workflow_pkg_id, workflow::Dag::ENTRY_GROUP);

    let entry_groups = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        move_std::Vector::EMPTY.module.into(),
        move_std::Vector::EMPTY.name.into(),
        vec![entry_group_type.clone()],
        vec![],
    );

    for entry_group in groups {
        // `entry_group: EntryGroup`
        let entry_group = workflow::Dag::entry_group_from_str(tx, workflow_pkg_id, entry_group)?;

        // `entry_groups.push_back(entry_group)`
        tx.programmable_move_call(
            sui::MOVE_STDLIB_PACKAGE_ID,
            move_std::Vector::PUSH_BACK.module.into(),
            move_std::Vector::PUSH_BACK.name.into(),
            vec![entry_group_type.clone()],
            vec![entry_groups, entry_group],
        );
    }

    // `name: Vertex`
    let name = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, vertex.name)?;

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            // `tool_fqn: AsciiString`
            workflow::Dag::off_chain_vertex_kind_from_fqn(tx, workflow_pkg_id, tool_fqn)?
        }
        VertexKind::OnChain { .. } => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
        }
    };

    // `input_ports: VecSet<InputPort>`
    let input_port_type = workflow::into_type_tag(workflow_pkg_id, workflow::Dag::INPUT_PORT);

    let input_ports = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::VecSet::EMPTY.module.into(),
        sui_framework::VecSet::EMPTY.name.into(),
        vec![input_port_type.clone()],
        vec![],
    );

    for input_port in vertex.input_ports {
        // `input_port: InputPort`
        let input_port = workflow::Dag::input_port_from_str(tx, workflow_pkg_id, input_port)?;

        // `input_ports.insert(input_port)`
        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecSet::INSERT.module.into(),
            sui_framework::VecSet::INSERT.name.into(),
            vec![input_port_type.clone()],
            vec![input_ports, input_port],
        );
    }

    // `dag.with_entry_vertex(name, kind, input_ports)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::WITH_ENTRY_VERTEX_IN_GROUPS.module.into(),
        workflow::Dag::WITH_ENTRY_VERTEX_IN_GROUPS.name.into(),
        vec![],
        vec![dag, entry_groups, name, kind, input_ports],
    ))
}

/// PTB template for creating a new DAG vertex.
pub fn create_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    vertex: &Vertex,
) -> anyhow::Result<sui::Argument> {
    // `name: Vertex`
    let name = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &vertex.name)?;

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            // `tool_fqn: AsciiString`
            workflow::Dag::off_chain_vertex_kind_from_fqn(tx, workflow_pkg_id, tool_fqn)?
        }
        VertexKind::OnChain { .. } => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
        }
    };

    // `dag.with_vertex(name, kind)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::WITH_VERTEX.module.into(),
        workflow::Dag::WITH_VERTEX.name.into(),
        vec![],
        vec![dag, name, kind],
    ))
}

/// PTB template for creating a new DAG default value.
pub fn create_default_value(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    primitives_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    default_value: &DefaultValue,
) -> anyhow::Result<sui::Argument> {
    // `vertex: Vertex`
    let vertex = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &default_value.vertex)?;

    // `port: InputPort`
    let port = workflow::Dag::input_port_from_str(tx, workflow_pkg_id, &default_value.input_port)?;

    // `value: NexusData`
    let value = match &default_value.value {
        Data::Inline { data } => {
            primitives::Data::nexus_data_from_json(tx, primitives_pkg_id, data)?
        }
        // Allowing to remind us that any other data storages can be added here.
        #[allow(unreachable_patterns)]
        _ => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/30>")
        }
    };

    // `dag.with_default_value(vertex, port, value)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Dag::WITH_DEFAULT_VALUE.module.into(),
        workflow::Dag::WITH_DEFAULT_VALUE.name.into(),
        vec![],
        vec![dag, vertex, port, value],
    ))
}

/// PTB template for creating a new DAG edge.
pub fn create_edge(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    edge: &Edge,
) -> anyhow::Result<sui::Argument> {
    // `from_vertex: Vertex`
    let from_vertex = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &edge.from.vertex)?;

    // `from_variant: OutputVariant`
    let from_variant =
        workflow::Dag::output_variant_from_str(tx, workflow_pkg_id, &edge.from.output_variant)?;

    // `from_port: OutputPort`
    let from_port =
        workflow::Dag::output_port_from_str(tx, workflow_pkg_id, &edge.from.output_port)?;

    // `to_vertex: Vertex`
    let to_vertex = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, &edge.to.vertex)?;

    // `to_port: InputPort`
    let to_port = workflow::Dag::input_port_from_str(tx, workflow_pkg_id, &edge.to.input_port)?;

    // `dag.with_edge(frpm_vertex, from_variant, from_port, to_vertex, to_port)`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
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

/// PTB template to execute a DAG.
pub fn execute(
    tx: &mut sui::ProgrammableTransactionBuilder,
    default_sap: sui::ObjectRef,
    dag: sui::ObjectRef,
    entry_group: String,
    input_json: serde_json::Value,
    workflow_pkg_id: sui::ObjectID,
    primitives_pkg_id: sui::ObjectID,
    network_id: sui::ObjectID,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut DefaultSAP`
    let default_sap = tx.obj(sui::ObjectArg::SharedObject {
        id: default_sap.object_id,
        initial_shared_version: default_sap.version,
        mutable: true,
    })?;

    // `dag: &DAG`
    let dag = tx.obj(sui::ObjectArg::SharedObject {
        id: dag.object_id,
        initial_shared_version: dag.version,
        mutable: false,
    })?;

    // `network: ID`
    let network = sui_framework::Object::id_from_object_id(tx, network_id)?;

    // `entry_group: EntryGroup`
    let entry_group = workflow::Dag::entry_group_from_str(tx, workflow_pkg_id, entry_group)?;

    // `with_vertex_inputs: VecMap<Vertex, VecMap<InputPort, NexusData>>`
    let inner_vec_map_type = vec![
        workflow::into_type_tag(workflow_pkg_id, workflow::Dag::INPUT_PORT),
        primitives::into_type_tag(primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        workflow::into_type_tag(workflow_pkg_id, workflow::Dag::VERTEX),
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
        let vertex = workflow::Dag::vertex_from_str(tx, workflow_pkg_id, vertex)?;

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
            let port = workflow::Dag::input_port_from_str(tx, workflow_pkg_id, port)?;

            // `value: NexusData`
            let value = primitives::Data::nexus_data_from_json(tx, primitives_pkg_id, value)?;

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

    // `workflow::default_sap::begin_dag_execution()`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::DefaultSap::BEGIN_DAG_EXECUTION.module.into(),
        workflow::DefaultSap::BEGIN_DAG_EXECUTION.name.into(),
        vec![],
        vec![default_sap, dag, network, entry_group, with_vertex_inputs],
    ))
}
