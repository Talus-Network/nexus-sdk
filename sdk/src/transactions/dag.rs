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
            FailureEvidenceKind,
            FromPort,
            NexusObjects,
            PostFailureAction,
            RuntimeVertex,
            Storable,
            StorageKind,
            Vertex,
            VertexKind,
            DEFAULT_ENTRY_GROUP,
        },
    },
    std::collections::{HashMap, HashSet},
};

const TERMINAL_ERR_EVAL_VARIANT: &str = "_err_eval";
const TERMINAL_ERR_EVAL_REASON_PORT: &str = "reason";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedToolOutput {
    pub output_variant: String,
    pub output_ports_data: HashMap<String, DataStorage>,
}

impl PreparedToolOutput {
    pub fn terminal_err_eval(reason: DataStorage) -> Self {
        Self {
            output_variant: TERMINAL_ERR_EVAL_VARIANT.to_string(),
            output_ports_data: HashMap::from([(TERMINAL_ERR_EVAL_REASON_PORT.to_string(), reason)]),
        }
    }
}

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

        if let Some(action) = vertex.post_failure_action {
            dag_arg =
                create_vertex_post_failure_action(tx, objects, dag_arg, &vertex.name, &action)?;
        }
    }

    if let Some(action) = &dag.post_failure_action {
        dag_arg = create_post_failure_action(tx, objects, dag_arg, action)?;
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

/// PTB template for configuring a DAG-level default post-failure action.
pub fn create_post_failure_action(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    action: &PostFailureAction,
) -> anyhow::Result<sui::types::Argument> {
    let action = workflow::Dag::post_failure_action_from_enum(tx, objects.workflow_pkg_id, action);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_POST_FAILURE_ACTION.module,
            workflow::Dag::WITH_POST_FAILURE_ACTION.name,
            vec![],
        ),
        vec![dag, action],
    ))
}

/// PTB template for configuring a vertex-level post-failure action override.
pub fn create_vertex_post_failure_action(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    vertex: &str,
    action: &PostFailureAction,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;
    let action = workflow::Dag::post_failure_action_from_enum(tx, objects.workflow_pkg_id, action);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_VERTEX_POST_FAILURE_ACTION.module,
            workflow::Dag::WITH_VERTEX_POST_FAILURE_ACTION.name,
            vec![],
        ),
        vec![dag, vertex, action],
    ))
}

/// PTB template for aborting an expired execution.
pub fn abort_expired_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> sui::types::Argument {
    let dag_arg = tx.input(sui::tx::Input::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));
    let execution_arg = tx.input(sui::tx::Input::shared(
        *execution.object_id(),
        execution.version(),
        true,
    ));
    let tool_registry_arg = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));
    let leader_registry_arg = tx.input(sui::tx::Input::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));
    let clock_arg = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::ABORT_EXPIRED_EXECUTION.module,
            workflow::Dag::ABORT_EXPIRED_EXECUTION.name,
            vec![],
        ),
        vec![
            dag_arg,
            execution_arg,
            tool_registry_arg,
            leader_registry_arg,
            clock_arg,
        ],
    )
}

/// PTB template for creating a failure evidence kind from an enum.
pub fn create_failure_evidence_kind(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    evidence_kind: &FailureEvidenceKind,
) -> sui::types::Argument {
    workflow::Dag::failure_evidence_kind_from_enum(tx, objects.workflow_pkg_id, evidence_kind)
}

fn prepare_tool_output(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    prepared: &PreparedToolOutput,
) -> anyhow::Result<(sui::types::Argument, sui::types::Argument)> {
    let output_variant = workflow::Dag::output_variant_from_str(
        tx,
        objects.workflow_pkg_id,
        &prepared.output_variant,
    )?;

    let map_generics = vec![
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::OUTPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let output_ports_data = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module,
            sui_framework::VecMap::EMPTY.name,
            map_generics.clone(),
        ),
        vec![],
    );

    for (output_port, dag_data) in &prepared.output_ports_data {
        let output_port =
            workflow::Dag::output_port_from_str(tx, objects.workflow_pkg_id, output_port)?;

        let value = match dag_data.storage_kind() {
            StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                tx,
                objects.primitives_pkg_id,
                dag_data.as_json(),
            )?,
            StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                tx,
                objects.primitives_pkg_id,
                dag_data.as_json(),
            )?,
        };

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
                map_generics.clone(),
            ),
            vec![output_ports_data, output_port, value],
        );
    }

    Ok((output_variant, output_ports_data))
}

#[allow(clippy::too_many_arguments)]
pub fn submit_off_chain_tool_eval_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    request_walk_execution: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    prepared_output: &PreparedToolOutput,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let (output_variant, output_ports_data) = prepare_tool_output(tx, objects, prepared_output)?;

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK.name,
            vec![],
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            request_walk_execution,
            walk_index,
            expected_vertex,
            output_variant,
            output_ports_data,
            clock,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_off_chain_tool_eval_for_walk_with_failure_evidence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    request_walk_execution: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    prepared_output: &PreparedToolOutput,
    failure_evidence_kind: &FailureEvidenceKind,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let failure_evidence_kind = create_failure_evidence_kind(tx, objects, failure_evidence_kind);
    let (output_variant, output_ports_data) = prepare_tool_output(tx, objects, prepared_output)?;

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.name,
            vec![],
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            request_walk_execution,
            walk_index,
            expected_vertex,
            output_variant,
            output_ports_data,
            failure_evidence_kind,
            clock,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_on_chain_tool_eval_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    request_walk_execution: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    prepared_output: &PreparedToolOutput,
    tool_witness_id: sui::types::Address,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let (output_variant, output_ports_data) = prepare_tool_output(tx, objects, prepared_output)?;
    let tool_witness_id = tx.input(pure_arg(&tool_witness_id)?);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK.name,
            vec![],
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            request_walk_execution,
            walk_index,
            expected_vertex,
            output_variant,
            output_ports_data,
            tool_witness_id,
            clock,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_on_chain_tool_eval_for_walk_with_failure_evidence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    request_walk_execution: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    prepared_output: &PreparedToolOutput,
    failure_evidence_kind: &FailureEvidenceKind,
    tool_witness_id: sui::types::Address,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let failure_evidence_kind = create_failure_evidence_kind(tx, objects, failure_evidence_kind);
    let (output_variant, output_ports_data) = prepare_tool_output(tx, objects, prepared_output)?;
    let tool_witness_id = tx.input(pure_arg(&tool_witness_id)?);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.name,
            vec![],
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            request_walk_execution,
            walk_index,
            expected_vertex,
            output_variant,
            output_ports_data,
            failure_evidence_kind,
            tool_witness_id,
            clock,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_on_chain_terminal_err_eval_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    request_walk_execution: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    reason: DataStorage,
    failure_evidence_kind: &FailureEvidenceKind,
    tool_witness_id: Option<sui::types::Address>,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    submit_on_chain_tool_eval_for_walk_with_failure_evidence(
        tx,
        objects,
        dag,
        execution,
        tool_registry,
        worksheet,
        leader_cap,
        request_walk_execution,
        walk_index,
        expected_vertex,
        &PreparedToolOutput::terminal_err_eval(reason),
        failure_evidence_kind,
        tool_witness_id.unwrap_or(sui::types::Address::ZERO),
        clock,
    )
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
        )?,
        StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
            tx,
            objects.primitives_pkg_id,
            &default_value.value.data,
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

    // `dag.with_edge(from_vertex, from_variant, from_port, to_vertex, to_port)`
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
    let entry_port =
        workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, &entry_port.name)?;

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

/// PTB template to prepare a DAG execution.
pub fn prepare_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    gas_service: sui::types::Argument,
    tool_registry: sui::types::Argument,
    dag: sui::types::Argument,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    clock: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut DefaultTAP`
    let default_tap = tx.input(sui::tx::Input::shared(
        *objects.default_tap.object_id(),
        objects.default_tap.version(),
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
            let port = workflow::Dag::input_port_from_str(
                tx,
                objects.workflow_pkg_id,
                port_name.as_str(),
            )?;

            // `value: NexusData`
            let value = match value.storage_kind() {
                StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
                )?,
                StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
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

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.input(pure_arg(&priority_fee_per_gas_unit)?);

    // `workflow::default_tap::begin_dag_execution()`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::DefaultTap::PREPARE_DAG_EXECUTION.module,
            workflow::DefaultTap::PREPARE_DAG_EXECUTION.name,
            vec![],
        ),
        vec![
            default_tap,
            dag,
            gas_service,
            tool_registry,
            network,
            entry_group,
            with_vertex_inputs,
            priority_fee_per_gas_unit,
            clock,
        ],
    ))
}

/// PTB template to lock gas state for the given tools.
pub fn lock_gas_state_for_tools(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution_gas: sui::types::Argument,
    invoker_gas: sui::types::Argument,
    tools_gas: Vec<sui::types::Argument>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    ticket: sui::types::Argument,
) {
    for tool_gas in tools_gas {
        // `nexus_workflow::gas::lock_gas_state_for_tool()`
        tx.move_call(
            sui::tx::Function::new(
                objects.workflow_pkg_id,
                workflow::Gas::LOCK_GAS_STATE_FOR_TOOL.module,
                workflow::Gas::LOCK_GAS_STATE_FOR_TOOL.name,
                vec![],
            ),
            vec![execution_gas, tool_gas, invoker_gas, dag, execution, ticket],
        );
    }
}

/// PTB template to execute a DAG.
pub fn execute(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    invoker_gas: &sui::types::ObjectReference,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
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

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    let results = prepare_execution(
        tx,
        objects,
        gas_service,
        tool_registry,
        dag,
        priority_fee_per_gas_unit,
        entry_group,
        input_data,
        clock,
    )?;

    // `ticket: RequestWalkExecution`
    let Some(ticket) = results.nested(0) else {
        return Err(anyhow::anyhow!("Failed to receive ticket argument"));
    };

    // `execution: DAGExecution`
    let Some(execution) = results.nested(1) else {
        return Err(anyhow::anyhow!("Failed to receive execution argument"));
    };

    // `execution_gas: ExecutionGas`
    let Some(execution_gas) = results.nested(2) else {
        return Err(anyhow::anyhow!("Failed to receive execution gas argument"));
    };

    // `invoker_gas: &mut InvokerGas`
    let invoker_gas = tx.input(sui::tx::Input::shared(
        *invoker_gas.object_id(),
        invoker_gas.version(),
        true,
    ));

    // `tools_gas: Vec<&mut ToolGas>`
    let tools_gas = tools_gas
        .iter()
        .map(|(address, version)| tx.input(sui::tx::Input::shared(*address, *version, true)))
        .collect();

    lock_gas_state_for_tools(
        tx,
        objects,
        execution_gas,
        invoker_gas,
        tools_gas,
        dag,
        execution,
        ticket,
    );

    // `leader_registry: &LeaderRegistry`
    let leader_registry = tx.input(sui::tx::Input::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    // `nexus_workflow::dag::request_network_to_execute_walks()`
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::REQUEST_NETWORK_TO_EXECUTE_WALKS.module,
            workflow::Dag::REQUEST_NETWORK_TO_EXECUTE_WALKS.name,
            vec![],
        ),
        vec![dag, execution, ticket, leader_registry, clock],
    );

    // `DAGExecution`
    let execution_type =
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::DAG_EXECUTION);

    // `sui::transfer::public_share_object<DAGExecution>`
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
            vec![execution_type],
        ),
        vec![execution],
    );

    // `ExecutionGas`
    let execution_gas_type =
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Gas::EXECUTION_GAS);

    // `sui::transfer::public_share_object<ExecutionGas>`
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
            vec![execution_gas_type],
        ),
        vec![execution_gas],
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            fqn,
            test_utils::sui_mocks,
            types::{Data, EdgeKind, FromPort, PostFailureAction, ToPort},
        },
        std::collections::HashMap,
    };

    struct TxInspector {
        tx: sui::types::Transaction,
    }

    impl TxInspector {
        fn new(tx: sui::types::Transaction) -> Self {
            Self { tx }
        }

        fn commands(&self) -> &Vec<sui::types::Command> {
            let sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction { commands, .. },
            ) = &self.tx.kind
            else {
                panic!("expected programmable transaction");
            };

            commands
        }

        fn inputs(&self) -> &Vec<sui::types::Input> {
            let sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction { inputs, .. },
            ) = &self.tx.kind
            else {
                panic!("expected programmable transaction");
            };

            inputs
        }

        fn move_call(&self, index: usize) -> &sui::types::MoveCall {
            match self.commands().get(index) {
                Some(sui::types::Command::MoveCall(call)) => call,
                Some(other) => panic!("expected move call, got {other:?}"),
                None => panic!("missing command at index {index}"),
            }
        }

        fn input(&self, argument: &sui::types::Argument) -> &sui::types::Input {
            let sui::types::Argument::Input(index) = argument else {
                panic!("expected argument input, got {argument:?}");
            };

            self.inputs()
                .get(*index as usize)
                .unwrap_or_else(|| panic!("missing input at index {index}"))
        }

        fn expect_address(&self, argument: &sui::types::Argument, expected: sui::types::Address) {
            let sui::types::Input::Pure { value } = self.input(argument) else {
                panic!("expected pure input, got {:?}", self.input(argument));
            };

            let actual: sui::types::Address =
                bcs::from_bytes(value).expect("address BCS should deserialize");
            assert_eq!(actual, expected);
        }
    }

    fn mock_runtime_vertex() -> RuntimeVertex {
        RuntimeVertex::plain("vertex1")
    }

    fn mock_prepared_tool_output() -> PreparedToolOutput {
        PreparedToolOutput {
            output_variant: "ok".to_string(),
            output_ports_data: HashMap::from([(
                "result".to_string(),
                DataStorage::try_from(serde_json::json!({
                    "kind": "inline",
                    "data": { "value": 7 }
                }))
                .expect("inline data storage"),
            )]),
        }
    }

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
            post_failure_action: None,
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
    fn test_create_post_failure_action() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);

        let mut tx = sui::tx::TransactionBuilder::new();
        create_post_failure_action(
            &mut tx,
            &objects,
            dag,
            &PostFailureAction::TransientContinue,
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
            panic!("Expected last command to be a MoveCall to set DAG post-failure action");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::WITH_POST_FAILURE_ACTION.module);
        assert_eq!(call.function, workflow::Dag::WITH_POST_FAILURE_ACTION.name);
    }

    #[test]
    fn test_create_vertex_post_failure_action() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);

        let mut tx = sui::tx::TransactionBuilder::new();
        create_vertex_post_failure_action(
            &mut tx,
            &objects,
            dag,
            "vertex1",
            &PostFailureAction::Terminate,
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
            panic!("Expected last command to be a MoveCall to set vertex post-failure action");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::WITH_VERTEX_POST_FAILURE_ACTION.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::WITH_VERTEX_POST_FAILURE_ACTION.name
        );
    }

    #[test]
    fn test_abort_expired_execution() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let execution = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        abort_expired_execution(&mut tx, &objects, &dag, &execution);
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to abort an expired execution");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Dag::ABORT_EXPIRED_EXECUTION.module);
        assert_eq!(call.function, workflow::Dag::ABORT_EXPIRED_EXECUTION.name);
        assert_eq!(call.arguments.len(), 5);
    }

    #[test]
    fn test_create_failure_evidence_kind() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        create_failure_evidence_kind(&mut tx, &objects, &FailureEvidenceKind::LeaderEvidence);
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create a failure evidence kind");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE.name
        );
    }

    #[test]
    fn test_submit_off_chain_tool_eval_for_walk() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_off_chain_tool_eval_for_walk(
            &mut tx,
            &objects,
            sui::types::Argument::Result(0),
            sui::types::Argument::Result(1),
            sui::types::Argument::Result(2),
            sui::types::Argument::Result(3),
            sui::types::Argument::Result(4),
            sui::types::Argument::Result(5),
            9,
            &mock_runtime_vertex(),
            &mock_prepared_tool_output(),
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK.name
        );
        assert_eq!(call.arguments.len(), 11);
    }

    #[test]
    fn test_submit_off_chain_tool_eval_for_walk_with_failure_evidence() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_off_chain_tool_eval_for_walk_with_failure_evidence(
            &mut tx,
            &objects,
            sui::types::Argument::Result(0),
            sui::types::Argument::Result(1),
            sui::types::Argument::Result(2),
            sui::types::Argument::Result(3),
            sui::types::Argument::Result(4),
            sui::types::Argument::Result(5),
            9,
            &mock_runtime_vertex(),
            &mock_prepared_tool_output(),
            &FailureEvidenceKind::LeaderEvidence,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.name
        );
        assert_eq!(call.arguments.len(), 12);
    }

    #[test]
    fn test_submit_on_chain_tool_eval_for_walk() {
        let objects = sui_mocks::mock_nexus_objects();
        let witness = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_on_chain_tool_eval_for_walk(
            &mut tx,
            &objects,
            sui::types::Argument::Result(0),
            sui::types::Argument::Result(1),
            sui::types::Argument::Result(2),
            sui::types::Argument::Result(3),
            sui::types::Argument::Result(4),
            sui::types::Argument::Result(5),
            11,
            &mock_runtime_vertex(),
            &mock_prepared_tool_output(),
            witness,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK.name
        );
        assert_eq!(call.arguments.len(), 12);
        inspector.expect_address(&call.arguments[10], witness);
    }

    #[test]
    fn test_submit_on_chain_tool_eval_for_walk_with_failure_evidence() {
        let objects = sui_mocks::mock_nexus_objects();
        let witness = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_on_chain_tool_eval_for_walk_with_failure_evidence(
            &mut tx,
            &objects,
            sui::types::Argument::Result(0),
            sui::types::Argument::Result(1),
            sui::types::Argument::Result(2),
            sui::types::Argument::Result(3),
            sui::types::Argument::Result(4),
            sui::types::Argument::Result(5),
            11,
            &mock_runtime_vertex(),
            &mock_prepared_tool_output(),
            &FailureEvidenceKind::ToolEvidence,
            witness,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.name
        );
        assert_eq!(call.arguments.len(), 13);
        inspector.expect_address(&call.arguments[11], witness);
    }

    #[test]
    fn test_submit_on_chain_terminal_err_eval_for_walk_defaults_zero_witness() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_on_chain_terminal_err_eval_for_walk(
            &mut tx,
            &objects,
            sui::types::Argument::Result(0),
            sui::types::Argument::Result(1),
            sui::types::Argument::Result(2),
            sui::types::Argument::Result(3),
            sui::types::Argument::Result(4),
            sui::types::Argument::Result(5),
            13,
            &mock_runtime_vertex(),
            DataStorage::try_from(serde_json::json!({
                "kind": "inline",
                "data": "tool failed"
            }))
            .expect("inline reason"),
            &FailureEvidenceKind::LeaderEvidence,
            None,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK_WITH_FAILURE_EVIDENCE.name
        );
        inspector.expect_address(&call.arguments[11], sui::types::Address::ZERO);
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
        let invoker_gas = sui_mocks::mock_sui_object_ref();
        let tools_gas = HashSet::from([(sui_mocks::mock_sui_address(), 0)]);

        input_data.insert(
            "vertex1".to_string(),
            HashMap::from([(
                "port1".to_string(),
                serde_json::json!({"kind": "inline", "data": { "key": "value"} })
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
            &invoker_gas,
            &tools_gas,
        )
        .unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.get(12).unwrap() else {
            panic!("Expected last command to be a MoveCall to execute a DAG");
        };

        assert_eq!(call.package, nexus_objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultTap::PREPARE_DAG_EXECUTION.module
        );
        assert_eq!(
            call.function,
            workflow::DefaultTap::PREPARE_DAG_EXECUTION.name
        );
    }

    #[test]
    fn test_create_output() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Argument::Result(0);
        let output = FromPort {
            vertex: "vertex1".to_string(),
            output_variant: "variant1".to_string(),
            output_port: "port1".to_string(),
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
    fn test_create_wires_post_failure_actions() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag_arg = sui::types::Argument::Result(0);
        let dag = Dag {
            vertices: vec![Vertex {
                kind: VertexKind::OffChain {
                    tool_fqn: fqn!("xyz.tool.test@1"),
                },
                name: "vertex1".to_string(),
                entry_ports: None,
                post_failure_action: Some(PostFailureAction::Terminate),
            }],
            edges: vec![],
            default_values: None,
            post_failure_action: Some(PostFailureAction::TransientContinue),
            entry_groups: None,
            outputs: None,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        create(&mut tx, &objects, dag_arg, dag).unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let move_calls = commands
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(move_calls.iter().any(|call| {
            call.module == workflow::Dag::WITH_VERTEX.module
                && call.function == workflow::Dag::WITH_VERTEX.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.module == workflow::Dag::WITH_VERTEX_POST_FAILURE_ACTION.module
                && call.function == workflow::Dag::WITH_VERTEX_POST_FAILURE_ACTION.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.module == workflow::Dag::WITH_POST_FAILURE_ACTION.module
                && call.function == workflow::Dag::WITH_POST_FAILURE_ACTION.name
        }));
    }
}
