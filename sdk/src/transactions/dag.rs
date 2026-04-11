use {
    crate::{
        idents::{move_std, primitives, pure_arg, sui_framework, workflow},
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
            OffChainSubmissionProofV1,
            OffChainToolResultAuxiliaryV1,
            OnChainToolResultSubmissionV1,
            PostFailureAction,
            PreparedToolOutputV1,
            RuntimeVertex,
            Storable,
            StorageKind,
            VerifierConfig,
            Vertex,
            VertexKind,
            DEFAULT_ENTRY_GROUP,
        },
    },
    std::collections::{HashMap, HashSet},
};

const TERMINAL_ERR_EVAL_VARIANT: &str = "_err_eval";
const TERMINAL_ERR_EVAL_REASON_PORT: &str = "reason";
const VERIFIER_V1_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_v1");
const PREPARED_TOOL_OUTPUT_PORT_V1_TYPE: sui::types::Identifier =
    sui::types::Identifier::from_static("PreparedToolOutputPortV1");
const NEW_PREPARED_TOOL_OUTPUT_PORT_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_prepared_tool_output_port_v1");
const NEW_PREPARED_TOOL_OUTPUT_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_prepared_tool_output_v1");
const PREPARED_TOOL_OUTPUT_V1_INTO_BCS_BYTES: sui::types::Identifier =
    sui::types::Identifier::from_static("prepared_tool_output_v1_into_bcs_bytes");
const VECTOR_APPEND: sui::types::Identifier = sui::types::Identifier::from_static("append");
const MAX_PURE_INPUT_BYTES: usize = 16_384;
const MAX_NEXUS_DATA_ARRAY_CHUNK_ARGS: usize = 64;

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

        if let Some(verifier) = &vertex.leader_verifier {
            dag_arg = create_vertex_leader_verifier(tx, objects, dag_arg, &vertex.name, verifier)?;
        }

        if let Some(verifier) = &vertex.tool_verifier {
            dag_arg = create_vertex_tool_verifier(tx, objects, dag_arg, &vertex.name, verifier)?;
        }
    }

    if let Some(action) = &dag.post_failure_action {
        dag_arg = create_post_failure_action(tx, objects, dag_arg, action)?;
    }

    if let Some(verifier) = &dag.leader_verifier {
        dag_arg = create_default_leader_verifier(tx, objects, dag_arg, verifier)?;
    }

    if let Some(verifier) = &dag.tool_verifier {
        dag_arg = create_default_tool_verifier(tx, objects, dag_arg, verifier)?;
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

fn verifier_registry_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.input(sui::tx::Input::shared(
        *objects.verifier_registry.object_id(),
        objects.verifier_registry.version(),
        false,
    )))
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

pub fn create_default_leader_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = workflow::Dag::verifier_config(tx, objects.workflow_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_DEFAULT_LEADER_VERIFIER.module,
            workflow::Dag::WITH_DEFAULT_LEADER_VERIFIER.name,
            vec![],
        ),
        vec![dag, verifier_registry, verifier],
    ))
}

pub fn create_default_tool_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = workflow::Dag::verifier_config(tx, objects.workflow_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_DEFAULT_TOOL_VERIFIER.module,
            workflow::Dag::WITH_DEFAULT_TOOL_VERIFIER.name,
            vec![],
        ),
        vec![dag, verifier_registry, verifier],
    ))
}

pub fn create_vertex_leader_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    vertex: &str,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = workflow::Dag::verifier_config(tx, objects.workflow_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_VERTEX_LEADER_VERIFIER.module,
            workflow::Dag::WITH_VERTEX_LEADER_VERIFIER.name,
            vec![],
        ),
        vec![dag, vertex, verifier_registry, verifier],
    ))
}

pub fn create_vertex_tool_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    vertex: &str,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex)?;
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = workflow::Dag::verifier_config(tx, objects.workflow_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::WITH_VERTEX_TOOL_VERIFIER.module,
            workflow::Dag::WITH_VERTEX_TOOL_VERIFIER.name,
            vec![],
        ),
        vec![dag, vertex, verifier_registry, verifier],
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

fn prepare_offchain_tool_result_bytes(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    result: &PreparedToolOutputV1,
) -> anyhow::Result<sui::types::Argument> {
    let output_variant = move_std::Ascii::ascii_string_from_str(tx, &result.output_variant)?;
    let port_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.interface_pkg_id,
        VERIFIER_V1_MODULE,
        PREPARED_TOOL_OUTPUT_PORT_V1_TYPE,
        vec![],
    )));

    let output_ports_data = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
            vec![port_type.clone()],
        ),
        vec![],
    );

    for output_port in &result.output_ports_data {
        let port = move_std::Ascii::ascii_string_from_str(tx, &output_port.port)?;
        let data = prepare_nexus_data(
            tx,
            objects.primitives_pkg_id,
            output_port.data.storage_kind(),
            output_port.data.as_json(),
        )?;
        let output_port = tx.move_call(
            sui::tx::Function::new(
                objects.interface_pkg_id,
                VERIFIER_V1_MODULE,
                NEW_PREPARED_TOOL_OUTPUT_PORT_V1,
                vec![],
            ),
            vec![port, data],
        );

        tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Vector::PUSH_BACK.module,
                move_std::Vector::PUSH_BACK.name,
                vec![port_type.clone()],
            ),
            vec![output_ports_data, output_port],
        );
    }

    let prepared_tool_output = tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_PREPARED_TOOL_OUTPUT_V1,
            vec![],
        ),
        vec![output_variant, output_ports_data],
    );

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            PREPARED_TOOL_OUTPUT_V1_INTO_BCS_BYTES,
            vec![],
        ),
        vec![prepared_tool_output],
    ))
}

fn prepare_nexus_data(
    tx: &mut sui::tx::TransactionBuilder,
    primitives_pkg_id: sui::types::Address,
    storage_kind: StorageKind,
    value: &serde_json::Value,
) -> anyhow::Result<sui::types::Argument> {
    let element_type = sui::types::TypeTag::Vector(Box::new(sui::types::TypeTag::U8));

    match value {
        serde_json::Value::Array(values) => {
            let array = if values.is_empty() {
                tx.make_move_vec(Some(element_type.clone()), vec![])
            } else {
                let mut chunks = values
                    .iter()
                    .map(serde_json::to_vec)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .try_fold(Vec::<Vec<Vec<u8>>>::new(), |mut chunks, value| {
                        let mut candidate = chunks.pop().unwrap_or_default();
                        candidate.push(value);

                        if candidate.len() > MAX_NEXUS_DATA_ARRAY_CHUNK_ARGS
                            || bcs::to_bytes(&candidate)?.len() > MAX_PURE_INPUT_BYTES
                        {
                            let last = candidate.pop().expect("candidate should not be empty");

                            if candidate.is_empty() {
                                anyhow::bail!(
                                    "single nexus data array element exceeds pure input size limit"
                                );
                            }

                            chunks.push(candidate);
                            chunks.push(vec![last]);
                        } else {
                            chunks.push(candidate);
                        }

                        Ok::<_, anyhow::Error>(chunks)
                    })?
                    .into_iter();
                let first = chunks
                    .next()
                    .expect("non-empty values should yield a first chunk");
                let first_args = first
                    .into_iter()
                    .map(|value| pure_arg(&value))
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into_iter()
                    .map(|input| tx.input(input))
                    .collect::<Vec<_>>();
                let array = tx.make_move_vec(Some(element_type.clone()), first_args);

                for chunk in chunks {
                    let chunk_args = chunk
                        .into_iter()
                        .map(|value| pure_arg(&value))
                        .collect::<anyhow::Result<Vec<_>>>()?
                        .into_iter()
                        .map(|input| tx.input(input))
                        .collect::<Vec<_>>();
                    let chunk = tx.make_move_vec(Some(element_type.clone()), chunk_args);
                    tx.move_call(
                        sui::tx::Function::new(
                            move_std::PACKAGE_ID,
                            move_std::Vector::EMPTY.module,
                            VECTOR_APPEND,
                            vec![element_type.clone()],
                        ),
                        vec![array, chunk],
                    );
                }

                array
            };

            let constructor = match storage_kind {
                StorageKind::Inline => primitives::Data::INLINE_MANY,
                StorageKind::Walrus => primitives::Data::WALRUS_MANY,
            };
            Ok(tx.move_call(
                sui::tx::Function::new(
                    primitives_pkg_id,
                    constructor.module,
                    constructor.name,
                    vec![],
                ),
                vec![array],
            ))
        }
        _ => match storage_kind {
            StorageKind::Inline => {
                primitives::Data::nexus_data_inline_from_json(tx, primitives_pkg_id, value)
            }
            StorageKind::Walrus => {
                primitives::Data::nexus_data_walrus_from_json(tx, primitives_pkg_id, value)
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn submit_off_chain_tool_result_for_walk_v1(
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
    result: &PreparedToolOutputV1,
    auxiliary: Option<&OffChainToolResultAuxiliaryV1>,
    requires_verifier_proof: bool,
    verifier_registry: Option<sui::types::Argument>,
    leader_registry: Option<sui::types::Argument>,
    network_auth: Option<sui::types::Argument>,
    leader_key_binding: Option<sui::types::Argument>,
    tool_key_binding: Option<sui::types::Argument>,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, objects, result)?;
    let auxiliary_bytes = tx.input(pure_arg(
        &auxiliary
            .map(OffChainToolResultAuxiliaryV1::to_bcs_bytes)
            .transpose()?,
    )?);
    let proof = auxiliary.map(|value| &value.proof);
    let (function, arguments) = if matches!(proof, None | Some(OffChainSubmissionProofV1::None)) {
        if requires_verifier_proof {
            anyhow::bail!("effective verifier policy requires proof-bearing offchain submission");
        }

        if verifier_registry.is_some()
            || leader_registry.is_some()
            || network_auth.is_some()
            || leader_key_binding.is_some()
            || tool_key_binding.is_some()
        {
            anyhow::bail!("proof-none offchain submission must not include verifier objects");
        }

        (
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1,
            vec![
                dag,
                execution,
                tool_registry,
                worksheet,
                leader_cap,
                request_walk_execution,
                walk_index,
                expected_vertex,
                result_bytes,
                auxiliary_bytes,
                clock,
            ],
        )
    } else {
        let verifier_registry = verifier_registry.ok_or_else(|| {
            anyhow::anyhow!("missing verifier_registry for proof-bearing offchain submission")
        })?;
        let leader_registry = leader_registry.ok_or_else(|| {
            anyhow::anyhow!("missing leader_registry for proof-bearing offchain submission")
        })?;
        let network_auth = network_auth.ok_or_else(|| {
            anyhow::anyhow!("missing network_auth for proof-bearing offchain submission")
        })?;
        let leader_key_binding = leader_key_binding.ok_or_else(|| {
            anyhow::anyhow!("missing leader_key_binding for proof-bearing offchain submission")
        })?;
        let tool_key_binding = tool_key_binding.ok_or_else(|| {
            anyhow::anyhow!("missing tool_key_binding for proof-bearing offchain submission")
        })?;

        (
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1,
            vec![
                dag,
                execution,
                tool_registry,
                worksheet,
                leader_cap,
                request_walk_execution,
                walk_index,
                expected_vertex,
                result_bytes,
                auxiliary_bytes,
                verifier_registry,
                leader_registry,
                network_auth,
                leader_key_binding,
                tool_key_binding,
                clock,
            ],
        )
    };

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            function.module,
            function.name,
            vec![],
        ),
        arguments,
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_on_chain_tool_result_for_walk_v1(
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
    submission: &OnChainToolResultSubmissionV1,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let submission = tx.input(pure_arg(&submission.to_bcs_bytes()?)?);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name,
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
            submission,
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
#[allow(clippy::too_many_arguments)]
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
#[allow(clippy::too_many_arguments)]
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
#[allow(clippy::too_many_arguments)]
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
            types::{
                Data,
                EdgeKind,
                FromPort,
                PostFailureAction,
                ToPort,
                VerifierConfig,
                VerifierMode,
            },
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

    fn mock_offchain_success_result() -> crate::types::PreparedToolOutputV1 {
        crate::types::PreparedToolOutputV1 {
            output_variant: "ok".to_string(),
            output_ports_data: vec![crate::types::PreparedToolOutputPortV1 {
                port: "result".to_string(),
                data: crate::types::NexusData::new_inline(serde_json::json!({ "value": 7 })),
            }],
        }
    }

    fn mock_offchain_proof_auxiliary() -> crate::types::OffChainToolResultAuxiliaryV1 {
        crate::types::OffChainToolResultAuxiliaryV1 {
            reported_failure_evidence_kind: None,
            proof: crate::types::OffChainSubmissionProofV1::ExternalVerifier {
                evidence: crate::types::ExternalVerifierSubmitEvidenceV2 {
                    result: crate::types::VerifierContractResultV1 {
                        method: "demo_verifier_v1".to_string(),
                        decision: crate::types::VerifierDecisionV1::Accept,
                        submission_kind: crate::types::VerificationSubmissionKind::Success,
                        failure_evidence_kind: crate::types::FailureEvidenceKind::ToolEvidence,
                        payload_or_reason_hash: vec![1, 2, 3],
                        credential: vec![4, 5],
                        detail: vec![6],
                    },
                    communication_evidence: vec![7, 8, 9],
                },
            },
        }
    }

    fn mock_offchain_failure_result() -> crate::types::PreparedToolOutputV1 {
        crate::types::PreparedToolOutputV1 {
            output_variant: "_err_eval".to_string(),
            output_ports_data: vec![crate::types::PreparedToolOutputPortV1 {
                port: "reason".to_string(),
                data: crate::types::NexusData::new_inline(serde_json::json!("failed")),
            }],
        }
    }

    fn mock_large_offchain_success_result() -> crate::types::PreparedToolOutputV1 {
        let blob_id =
            "walrus-blob-id-0123456789abcdef0123456789abcdef-fedcba98765432100123456789abcdef";
        crate::types::PreparedToolOutputV1 {
            output_variant: "ok".to_string(),
            output_ports_data: vec![crate::types::PreparedToolOutputPortV1 {
                port: "result".to_string(),
                data: crate::types::NexusData::new_walrus(serde_json::json!(std::iter::repeat_n(
                    blob_id, 1000
                )
                .collect::<Vec<_>>())),
            }],
        }
    }

    fn mock_offchain_failure_auxiliary() -> crate::types::OffChainToolResultAuxiliaryV1 {
        crate::types::OffChainToolResultAuxiliaryV1 {
            reported_failure_evidence_kind: Some(crate::types::FailureEvidenceKind::LeaderEvidence),
            proof: crate::types::OffChainSubmissionProofV1::None,
        }
    }

    fn mock_objects_with_verifier_registry() -> NexusObjects {
        let mut objects = sui_mocks::mock_nexus_objects();
        objects.verifier_registry = sui_mocks::mock_sui_object_ref();
        objects
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
            leader_verifier: None,
            tool_verifier: None,
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
    fn test_submit_off_chain_tool_result_for_walk_v1_with_proof() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_off_chain_tool_result_for_walk_v1(
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
            &mock_offchain_success_result(),
            Some(&mock_offchain_proof_auxiliary()),
            true,
            Some(sui::types::Argument::Result(6)),
            Some(sui::types::Argument::Result(7)),
            Some(sui::types::Argument::Result(8)),
            Some(sui::types::Argument::Result(9)),
            Some(sui::types::Argument::Result(10)),
            sui::types::Argument::Result(11),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 16);
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_v1_without_proof_routes_to_plain_submit() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_off_chain_tool_result_for_walk_v1(
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
            &mock_offchain_success_result(),
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.name
        );
        assert_eq!(call.arguments.len(), 11);
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_v1_without_proof_routes_to_failure_submit() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_off_chain_tool_result_for_walk_v1(
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
            &mock_offchain_failure_result(),
            Some(&mock_offchain_failure_auxiliary()),
            false,
            None,
            None,
            None,
            None,
            None,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.name
        );
        assert_eq!(call.arguments.len(), 11);
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_v1_large_result_avoids_oversized_pure_input() {
        let result = mock_large_offchain_success_result();
        assert!(
            result
                .to_bcs_bytes()
                .expect("large offchain result should encode")
                .len()
                > MAX_PURE_INPUT_BYTES
        );

        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_off_chain_tool_result_for_walk_v1(
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
            &result,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let submit_call = inspector.move_call(inspector.commands().len() - 1);
        let result_arg = submit_call
            .arguments
            .get(8)
            .expect("submit call should include result bytes argument");

        assert!(
            matches!(result_arg, sui::types::Argument::Result(_)),
            "large result should be produced by a prior Move call"
        );
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == VERIFIER_V1_MODULE
                        && call.function == PREPARED_TOOL_OUTPUT_V1_INTO_BCS_BYTES
            )
        }));
        assert!(inspector.inputs().iter().all(|input| match input {
            sui::types::Input::Pure { value } => value.len() <= MAX_PURE_INPUT_BYTES,
            _ => true,
        }));
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_v1_requires_verifier_objects_for_proof_auxiliary()
    {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        let error = submit_off_chain_tool_result_for_walk_v1(
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
            &mock_offchain_success_result(),
            Some(&mock_offchain_proof_auxiliary()),
            true,
            None,
            None,
            None,
            None,
            None,
            sui::types::Argument::Result(6),
        )
        .expect_err("proof-bearing auxiliary should require verifier objects");

        assert!(error
            .to_string()
            .contains("missing verifier_registry for proof-bearing offchain submission"));
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_v1_rejects_verifier_objects_for_proof_none_auxiliary(
    ) {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        let error = submit_off_chain_tool_result_for_walk_v1(
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
            &mock_offchain_failure_result(),
            Some(&mock_offchain_failure_auxiliary()),
            false,
            Some(sui::types::Argument::Result(6)),
            Some(sui::types::Argument::Result(7)),
            Some(sui::types::Argument::Result(8)),
            Some(sui::types::Argument::Result(9)),
            Some(sui::types::Argument::Result(10)),
            sui::types::Argument::Result(11),
        )
        .expect_err("proof-none auxiliary should reject verifier objects");

        assert!(error
            .to_string()
            .contains("proof-none offchain submission must not include verifier objects"));
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_v1_rejects_proof_none_for_verifier_aware_vertex()
    {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        let error = submit_off_chain_tool_result_for_walk_v1(
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
            &mock_offchain_failure_result(),
            Some(&mock_offchain_failure_auxiliary()),
            true,
            None,
            None,
            None,
            None,
            None,
            sui::types::Argument::Result(6),
        )
        .expect_err("verifier-aware vertex should reject proof-none routing");

        assert!(error
            .to_string()
            .contains("effective verifier policy requires proof-bearing offchain submission"));
    }

    #[test]
    fn test_submit_on_chain_tool_result_for_walk_v1() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let submission = crate::types::OnChainToolResultSubmissionV1 {
            observed_output: crate::types::PreparedToolOutputV1 {
                output_variant: "ok".to_string(),
                output_ports_data: vec![crate::types::PreparedToolOutputPortV1 {
                    port: "result".to_string(),
                    data: crate::types::NexusData::new_inline(serde_json::json!({ "value": 7 })),
                }],
            },
            raw_failure_evidence_kind: Some(crate::types::FailureEvidenceKind::ToolEvidence),
            submitted_failure_reason: None,
            tool_witness_id: sui::types::Address::ZERO,
        };

        submit_on_chain_tool_result_for_walk_v1(
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
            &submission,
            sui::types::Argument::Result(6),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 10);
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
                leader_verifier: None,
                tool_verifier: None,
            }],
            edges: vec![],
            default_values: None,
            post_failure_action: Some(PostFailureAction::TransientContinue),
            leader_verifier: None,
            tool_verifier: None,
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

    #[test]
    fn test_create_wires_verifier_config() {
        let objects = mock_objects_with_verifier_registry();
        let dag_arg = sui::types::Argument::Result(0);
        let dag = Dag {
            vertices: vec![Vertex {
                kind: VertexKind::OffChain {
                    tool_fqn: fqn!("xyz.tool.test@1"),
                },
                name: "vertex1".to_string(),
                entry_ports: None,
                post_failure_action: None,
                leader_verifier: None,
                tool_verifier: Some(VerifierConfig {
                    mode: VerifierMode::ToolVerifierContract,
                    method: "demo_verifier_v1".to_string(),
                }),
            }],
            edges: vec![],
            default_values: None,
            post_failure_action: None,
            leader_verifier: Some(VerifierConfig {
                mode: VerifierMode::LeaderRegisteredKey,
                method: "signed_http_v1".to_string(),
            }),
            tool_verifier: None,
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
            call.module == workflow::Dag::WITH_DEFAULT_LEADER_VERIFIER.module
                && call.function == workflow::Dag::WITH_DEFAULT_LEADER_VERIFIER.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.module == workflow::Dag::WITH_VERTEX_TOOL_VERIFIER.module
                && call.function == workflow::Dag::WITH_VERTEX_TOOL_VERIFIER.name
        }));
    }
}
