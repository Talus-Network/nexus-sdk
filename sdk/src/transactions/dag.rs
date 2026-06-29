use {
    crate::{
        idents::{interface, move_std, primitives, registry, sui_framework, workflow},
        sui,
        transactions::{agent_input::AgentInput, tap},
        types::{
            AgentId,
            AgentVertexAuthorizationTemplate,
            AuthenticatedOffchainRequestEvidence,
            AuthenticatedOffchainVerifierEvidence,
            Dag,
            DefaultValue,
            Edge,
            EntryPort,
            ExternalVerifierRuntimeCall,
            FailureEvidenceKind,
            FromPort,
            NexusData,
            NexusObjects,
            OffChainToolResultAuxiliary,
            OffChainVerifierProof,
            OffchainRequestEvidence,
            OffchainResponseEvidence,
            OffchainVerifierEvidence,
            PostFailureAction,
            RuntimeVertex,
            SkillId,
            StorageKind,
            VerifierConfig,
            Vertex,
            VertexKind,
            DEFAULT_ENTRY_GROUP,
        },
    },
    std::{
        collections::{HashMap, HashSet},
        str::FromStr,
    },
};

const TERMINAL_ERR_EVAL_VARIANT: &str = "_err_eval";
const TERMINAL_ERR_EVAL_REASON_PORT: &str = "reason";
const VECTOR_APPEND: sui::types::Identifier = sui::types::Identifier::from_static("append");
const MAX_PURE_INPUT_BYTES: usize = 16_384;
const MAX_NEXUS_DATA_ARRAY_CHUNK_ARGS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedToolOutput {
    pub output_variant: String,
    pub output_ports_data: HashMap<String, NexusData>,
}

impl PreparedToolOutput {
    pub fn terminal_err_eval(reason: NexusData) -> Self {
        Self {
            output_variant: TERMINAL_ERR_EVAL_VARIANT.to_string(),
            output_ports_data: HashMap::from([(TERMINAL_ERR_EVAL_REASON_PORT.to_string(), reason)]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDagExecuteInput {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub selected_dag: Option<sui::types::Address>,
    pub authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    pub payment_source: Vec<u8>,
    pub payment_coin: Option<sui::types::ObjectReference>,
    pub payment_coin_balance: Option<u64>,
    pub payment_max_budget: u64,
}

/// PTB template for creating a new empty DAG.
pub fn empty(tx: &mut sui::tx::TransactionBuilder, objects: &NexusObjects) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::NEW.module,
            interface::Dag::NEW.name,
        ),
        vec![],
    )
}

/// PTB template to publish a DAG.
pub fn publish(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
) -> sui::tx::Argument {
    let dag_type = interface::into_type_tag(objects.interface_pkg_id, interface::Dag::DAG);

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
        )
        .with_type_args(vec![dag_type]),
        vec![dag],
    )
}

/// PTB template to publish a full [`crate::types::Dag`].
pub fn create(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    mut dag_arg: sui::tx::Argument,
    dag: Dag,
) -> anyhow::Result<sui::tx::Argument> {
    // Create all vertices.
    for vertex in &dag.vertices {
        dag_arg = create_vertex(tx, objects, dag_arg, vertex)?;

        if let Some(action) = &vertex.post_failure_action {
            dag_arg =
                create_vertex_post_failure_action(tx, objects, dag_arg, &vertex.name, action)?;
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
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.object(sui::tx::ObjectInput::shared(
        *objects.verifier_registry.object_id(),
        objects.verifier_registry.version(),
        false,
    )))
}

/// PTB template for creating a new DAG vertex.
pub fn create_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    vertex: &Vertex,
) -> anyhow::Result<sui::tx::Argument> {
    // `name: Vertex`
    let name = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, &vertex.name)?;

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => interface::Graph::off_chain_vertex_kind_from_fqn(
            tx,
            objects.interface_pkg_id,
            tool_fqn,
        )?,
        VertexKind::OnChain { tool_fqn } => {
            interface::Graph::on_chain_vertex_kind_from_fqn(tx, objects.interface_pkg_id, tool_fqn)?
        }
    };

    // `dag.with_vertex(name, kind)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::WITH_VERTEX.module,
            interface::Dag::WITH_VERTEX.name,
        ),
        vec![dag, name, kind],
    ))
}

/// PTB template for configuring a DAG-level default post-failure action.
pub fn create_post_failure_action(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    action: &PostFailureAction,
) -> anyhow::Result<sui::tx::Argument> {
    let action =
        interface::Graph::post_failure_action_from_enum(tx, objects.interface_pkg_id, action);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::WITH_POST_FAILURE_ACTION.module,
            interface::Dag::WITH_POST_FAILURE_ACTION.name,
        ),
        vec![dag, action],
    ))
}

/// PTB template for configuring a vertex-level post-failure action override.
pub fn create_vertex_post_failure_action(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    vertex: &str,
    action: &PostFailureAction,
) -> anyhow::Result<sui::tx::Argument> {
    let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex)?;
    let action =
        interface::Graph::post_failure_action_from_enum(tx, objects.interface_pkg_id, action);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::WITH_VERTEX_POST_FAILURE_ACTION.module,
            interface::Dag::WITH_VERTEX_POST_FAILURE_ACTION.name,
        ),
        vec![dag, vertex, action],
    ))
}

pub fn create_default_leader_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::tx::Argument> {
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = interface::Verifier::verifier_config(tx, objects.interface_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.registry_pkg_id,
            registry::VerifierRegistry::WITH_DEFAULT_LEADER_VERIFIER.module,
            registry::VerifierRegistry::WITH_DEFAULT_LEADER_VERIFIER.name,
        ),
        vec![verifier_registry, dag, verifier],
    ))
}

pub fn create_default_tool_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::tx::Argument> {
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = interface::Verifier::verifier_config(tx, objects.interface_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.registry_pkg_id,
            registry::VerifierRegistry::WITH_DEFAULT_TOOL_VERIFIER.module,
            registry::VerifierRegistry::WITH_DEFAULT_TOOL_VERIFIER.name,
        ),
        vec![verifier_registry, dag, verifier],
    ))
}

pub fn create_vertex_leader_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    vertex: &str,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::tx::Argument> {
    let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex)?;
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = interface::Verifier::verifier_config(tx, objects.interface_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.registry_pkg_id,
            registry::VerifierRegistry::WITH_VERTEX_LEADER_VERIFIER.module,
            registry::VerifierRegistry::WITH_VERTEX_LEADER_VERIFIER.name,
        ),
        vec![verifier_registry, dag, vertex, verifier],
    ))
}

pub fn create_vertex_tool_verifier(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    vertex: &str,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::tx::Argument> {
    let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex)?;
    let verifier_registry = verifier_registry_arg(tx, objects)?;
    let verifier = interface::Verifier::verifier_config(tx, objects.interface_pkg_id, verifier)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.registry_pkg_id,
            registry::VerifierRegistry::WITH_VERTEX_TOOL_VERIFIER.module,
            registry::VerifierRegistry::WITH_VERTEX_TOOL_VERIFIER.name,
        ),
        vec![verifier_registry, dag, vertex, verifier],
    ))
}

/// PTB template for aborting an expired execution.
pub fn abort_expired_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> sui::tx::Argument {
    let dag_arg = tx.object(sui::tx::ObjectInput::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));
    let execution_arg = tx.object(sui::tx::ObjectInput::shared(
        *execution.object_id(),
        execution.version(),
        true,
    ));
    let tool_registry_arg = tx.object(sui::tx::ObjectInput::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));
    let leader_registry_arg = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));
    let clock_arg = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::ABORT_EXPIRED_EXECUTION.module,
            workflow::ExecutionSettlement::ABORT_EXPIRED_EXECUTION.name,
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

pub fn accomplish_tap_execution_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT.module,
            workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT.name,
        ),
        vec![execution],
    )
}

pub fn accomplish_tap_execution_payment_from_agent_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::tx::Argument,
    execution: sui::tx::Argument,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module,
            workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name,
        ),
        vec![agent, execution],
    )
}

pub fn refund_tap_execution_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
    refund_reason: Vec<u8>,
) -> anyhow::Result<sui::tx::Argument> {
    let refund_reason = tx.pure(&refund_reason);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::REFUND_TAP_EXECUTION_PAYMENT.module,
            workflow::ExecutionSettlement::REFUND_TAP_EXECUTION_PAYMENT.name,
        ),
        vec![execution, refund_reason],
    ))
}

pub fn refund_tap_execution_payment_from_agent_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::tx::Argument,
    execution: sui::tx::Argument,
    refund_reason: Vec<u8>,
) -> anyhow::Result<sui::tx::Argument> {
    let refund_reason = tx.pure(&refund_reason);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::REFUND_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module,
            workflow::ExecutionSettlement::REFUND_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name,
        ),
        vec![agent, execution, refund_reason],
    ))
}

pub fn refill_tap_execution_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
    coin: sui::tx::Argument,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT.module,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT.name,
        ),
        vec![execution, coin],
    )
}

pub fn refill_tap_execution_payment_from_agent_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::tx::Argument,
    execution: sui::tx::Argument,
    amount: u64,
) -> sui::tx::Argument {
    let amount = tx.pure(&amount);
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name,
        ),
        vec![agent, execution, amount],
    )
}

/// PTB template for creating a failure evidence kind from an enum.
pub fn create_failure_evidence_kind(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    evidence_kind: &FailureEvidenceKind,
) -> sui::tx::Argument {
    interface::Verifier::failure_evidence_kind_from_enum(
        tx,
        objects.interface_pkg_id,
        evidence_kind,
    )
}

fn prepare_tool_output(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    prepared: &PreparedToolOutput,
) -> anyhow::Result<(sui::tx::Argument, sui::tx::Argument)> {
    let output_variant = interface::Graph::output_variant_from_str(
        tx,
        objects.interface_pkg_id,
        &prepared.output_variant,
    )?;

    let map_generics = vec![
        interface::into_type_tag(objects.interface_pkg_id, interface::Graph::OUTPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let output_ports_data = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module,
            sui_framework::VecMap::EMPTY.name,
        )
        .with_type_args(map_generics.clone()),
        vec![],
    );

    for (output_port, dag_data) in &prepared.output_ports_data {
        let output_port =
            interface::Graph::output_port_from_str(tx, objects.interface_pkg_id, output_port)?;

        let value = match dag_data.storage_kind() {
            StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                tx,
                objects.primitives_pkg_id,
                &dag_data.as_json(),
            )?,
            StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                tx,
                objects.primitives_pkg_id,
                &dag_data.as_json(),
            )?,
        };

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
            )
            .with_type_args(map_generics.clone()),
            vec![output_ports_data, output_port, value],
        );
    }

    Ok((output_variant, output_ports_data))
}

fn prepare_offchain_tool_result_bytes(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    result: &crate::types::PreparedToolOutput,
) -> anyhow::Result<sui::tx::Argument> {
    let output_variant =
        move_std::Ascii::ascii_string_from_str(tx, result.output_variant.as_str())?;
    let port_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.interface_pkg_id,
        interface::Verifier::PREPARED_TOOL_OUTPUT_PORT.module,
        interface::Verifier::PREPARED_TOOL_OUTPUT_PORT.name,
        vec![],
    )));

    let output_ports_data = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
        )
        .with_type_args(vec![port_type.clone()]),
        vec![],
    );

    for output_port in &result.output_ports_data {
        let port = move_std::Ascii::ascii_string_from_str(tx, output_port.port.as_str())?;
        let data = prepare_nexus_data(tx, objects.primitives_pkg_id, &output_port.data)?;
        let output_port = tx.move_call(
            sui::tx::Function::new(
                objects.interface_pkg_id,
                interface::Verifier::NEW_PREPARED_TOOL_OUTPUT_PORT.module,
                interface::Verifier::NEW_PREPARED_TOOL_OUTPUT_PORT.name,
            ),
            vec![port, data],
        );

        tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Vector::PUSH_BACK.module,
                move_std::Vector::PUSH_BACK.name,
            )
            .with_type_args(vec![port_type.clone()]),
            vec![output_ports_data, output_port],
        );
    }

    let prepared_tool_output = tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_PREPARED_TOOL_OUTPUT.module,
            interface::Verifier::NEW_PREPARED_TOOL_OUTPUT.name,
        ),
        vec![output_variant, output_ports_data],
    );

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::PREPARED_TOOL_OUTPUT_INTO_BCS_BYTES.module,
            interface::Verifier::PREPARED_TOOL_OUTPUT_INTO_BCS_BYTES.name,
        ),
        vec![prepared_tool_output],
    ))
}

fn prepare_nexus_data(
    tx: &mut sui::tx::TransactionBuilder,
    primitives_pkg_id: sui::types::Address,
    value: &crate::types::generated::primitives_types::data::NexusData,
) -> anyhow::Result<sui::tx::Argument> {
    let element_type = sui::types::TypeTag::Vector(Box::new(sui::types::TypeTag::U8));
    let storage_kind = match value.storage.as_slice() {
        b"inline" => StorageKind::Inline,
        b"walrus" => StorageKind::Walrus,
        storage => anyhow::bail!(
            "unsupported NexusData storage tag: {}",
            hex::encode(storage)
        ),
    };

    if !value.many.is_empty() || value.one.is_empty() {
        let array = if value.many.is_empty() {
            tx.make_move_vec(Some(element_type.clone()), vec![])
        } else {
            let mut chunks = value
                .many
                .iter()
                .cloned()
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
                .map(|value| tx.pure(&value))
                .collect::<Vec<_>>();
            let array = tx.make_move_vec(Some(element_type.clone()), first_args);

            for chunk in chunks {
                let chunk_args = chunk
                    .into_iter()
                    .map(|value| tx.pure(&value))
                    .collect::<Vec<_>>();
                let chunk = tx.make_move_vec(Some(element_type.clone()), chunk_args);
                tx.move_call(
                    sui::tx::Function::new(
                        move_std::PACKAGE_ID,
                        move_std::Vector::EMPTY.module,
                        VECTOR_APPEND,
                    )
                    .with_type_args(vec![element_type.clone()]),
                    vec![array, chunk],
                );
            }

            array
        };

        let constructor = match storage_kind {
            StorageKind::Inline => primitives::Data::INLINE_MANY,
            StorageKind::Walrus => primitives::Data::WALRUS_MANY,
        };
        return Ok(tx.move_call(
            sui::tx::Function::new(primitives_pkg_id, constructor.module, constructor.name),
            vec![array],
        ));
    }

    let constructor = match storage_kind {
        StorageKind::Inline => primitives::Data::INLINE_ONE,
        StorageKind::Walrus => primitives::Data::WALRUS_ONE,
    };
    let one = tx.pure(&value.one);
    Ok(tx.move_call(
        sui::tx::Function::new(primitives_pkg_id, constructor.module, constructor.name),
        vec![one],
    ))
}

fn prepare_move_option_vec_u8(
    tx: &mut sui::tx::TransactionBuilder,
    value: &Option<Vec<u8>>,
) -> sui::tx::Argument {
    tx.pure(value)
}

fn prepare_move_option_failure_evidence_kind(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    value: Option<&FailureEvidenceKind>,
) -> sui::tx::Argument {
    match value {
        Some(value) => {
            let kind = create_failure_evidence_kind(tx, objects, value);
            tx.move_call(
                sui::tx::Function::new(
                    move_std::PACKAGE_ID,
                    move_std::Option::SOME.module,
                    move_std::Option::SOME.name,
                )
                .with_type_args(vec![workflow::into_type_tag(
                    objects.interface_pkg_id,
                    interface::Verifier::FAILURE_EVIDENCE_KIND,
                )]),
                vec![kind],
            )
        }
        None => tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Option::NONE.module,
                move_std::Option::NONE.name,
            )
            .with_type_args(vec![workflow::into_type_tag(
                objects.interface_pkg_id,
                interface::Verifier::FAILURE_EVIDENCE_KIND,
            )]),
            vec![],
        ),
    }
}

fn prepare_submission_kind(
    tx: &mut sui::tx::TransactionBuilder,
    interface_pkg_id: sui::types::Address,
    submission_kind: crate::types::VerificationSubmissionKind,
) -> sui::tx::Argument {
    let function = match submission_kind {
        crate::types::VerificationSubmissionKind::Success => {
            interface::Verifier::VERIFICATION_SUBMISSION_KIND_SUCCESS
        }
        crate::types::VerificationSubmissionKind::ErrEval => {
            interface::Verifier::VERIFICATION_SUBMISSION_KIND_ERR_EVAL
        }
    };

    tx.move_call(
        sui::tx::Function::new(interface_pkg_id, function.module, function.name),
        vec![],
    )
}

fn prepare_verifier_evidence_kind(
    tx: &mut sui::tx::TransactionBuilder,
    interface_pkg_id: sui::types::Address,
    failure_evidence_kind: FailureEvidenceKind,
) -> sui::tx::Argument {
    let function = match failure_evidence_kind {
        FailureEvidenceKind::ToolEvidence => {
            interface::Verifier::FAILURE_EVIDENCE_KIND_TOOL_EVIDENCE
        }
        FailureEvidenceKind::LeaderEvidence => {
            interface::Verifier::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE
        }
    };

    tx.move_call(
        sui::tx::Function::new(interface_pkg_id, function.module, function.name),
        vec![],
    )
}

fn prepare_verifier_decision(
    tx: &mut sui::tx::TransactionBuilder,
    interface_pkg_id: sui::types::Address,
    decision: crate::types::VerifierDecision,
) -> sui::tx::Argument {
    let function = match decision {
        crate::types::VerifierDecision::Accept => interface::Verifier::VERIFIER_DECISION_ACCEPT,
        crate::types::VerifierDecision::Reject => interface::Verifier::VERIFIER_DECISION_REJECT,
    };

    tx.move_call(
        sui::tx::Function::new(interface_pkg_id, function.module, function.name),
        vec![],
    )
}

fn prepare_verifier_contract_result(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    result: &crate::types::VerifierContractResult,
) -> anyhow::Result<sui::tx::Argument> {
    let method = move_std::Ascii::ascii_string_from_str(tx, &result.method)?;
    let decision = prepare_verifier_decision(tx, objects.interface_pkg_id, result.decision.clone());
    let submission_kind =
        prepare_submission_kind(tx, objects.interface_pkg_id, result.submission_kind.clone());
    let failure_evidence_kind = prepare_verifier_evidence_kind(
        tx,
        objects.interface_pkg_id,
        result.failure_evidence_kind.clone(),
    );
    let payload_or_reason_hash = tx.pure(&result.payload_or_reason_hash);
    let credential = tx.pure(&result.credential);
    let detail = tx.pure(&result.detail);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_VERIFIER_CONTRACT_RESULT.module,
            interface::Verifier::NEW_VERIFIER_CONTRACT_RESULT.name,
        ),
        vec![
            method,
            decision,
            submission_kind,
            failure_evidence_kind,
            payload_or_reason_hash,
            credential,
            detail,
        ],
    ))
}

fn prepare_external_verifier_submit_evidence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    evidence: &crate::types::ExternalVerifierSubmitEvidence,
) -> anyhow::Result<sui::tx::Argument> {
    let result = prepare_verifier_contract_result(tx, objects, &evidence.result)?;
    let communication_evidence = tx.pure(&evidence.communication_evidence);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE.module,
            interface::Verifier::NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE.name,
        ),
        vec![result, communication_evidence],
    ))
}

fn prepare_offchain_verifier_proof(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    proof: &OffChainVerifierProof,
) -> anyhow::Result<sui::tx::Argument> {
    match proof {
        OffChainVerifierProof::RegisteredKey {
            verifier_credential,
            communication_evidence,
        } => {
            let verifier_credential = tx.pure(verifier_credential);
            let communication_evidence = tx.pure(communication_evidence);
            Ok(tx.move_call(
                sui::tx::Function::new(
                    objects.interface_pkg_id,
                    interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY.module,
                    interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY.name,
                ),
                vec![verifier_credential, communication_evidence],
            ))
        }
        OffChainVerifierProof::ExternalVerifier { evidence } => {
            let evidence = prepare_external_verifier_submit_evidence(tx, objects, evidence)?;
            Ok(tx.move_call(
                sui::tx::Function::new(
                    objects.interface_pkg_id,
                    interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER.module,
                    interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER.name,
                ),
                vec![evidence],
            ))
        }
    }
}

fn prepare_authenticated_offchain_request_evidence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    expected_vertex: sui::tx::Argument,
    request: &AuthenticatedOffchainRequestEvidence,
) -> anyhow::Result<sui::tx::Argument> {
    let walk_index = tx.pure(&request.walk_index);
    let tool_fqn = move_std::Ascii::ascii_string_from_str(tx, &request.tool_fqn)?;
    let request_hash = tx.pure(&request.request_hash);
    let request_signature = tx.pure(&request.request_signature);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE.module,
            workflow::ExecutionSubmission::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE.name,
        ),
        vec![
            execution,
            leader_cap,
            walk_index,
            expected_vertex,
            tool_fqn,
            request_hash,
            request_signature,
        ],
    ))
}

fn prepare_raw_offchain_request_evidence_for_preflight(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    request: &OffchainRequestEvidence,
) -> anyhow::Result<sui::tx::Argument> {
    let execution = sui_framework::Object::id_from_object_id(tx, request.execution.clone().into())?;
    let walk_index = tx.pure(&request.walk_index);
    let vertex = move_std::Ascii::ascii_string_from_str(tx, &request.vertex)?;
    let tool_fqn = move_std::Ascii::ascii_string_from_str(tx, &request.tool_fqn)?;
    let leader_cap_id =
        sui_framework::Object::id_from_object_id(tx, request.leader_cap_id.clone().into())?;
    let request_hash = tx.pure(&request.request_hash);
    let request_signature = tx.pure(&request.request_signature);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_OFFCHAIN_REQUEST_EVIDENCE.module,
            interface::Verifier::NEW_OFFCHAIN_REQUEST_EVIDENCE.name,
        ),
        vec![
            execution,
            walk_index,
            vertex,
            tool_fqn,
            leader_cap_id,
            request_hash,
            request_signature,
        ],
    ))
}

fn prepare_offchain_response_evidence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    response: &OffchainResponseEvidence,
) -> anyhow::Result<sui::tx::Argument> {
    let status_code = tx.pure(&response.status_code);
    let response_hash = tx.pure(&response.response_hash);
    let response_signature = tx.pure(&response.response_signature);
    let normalized_err_eval_reason_hash =
        prepare_move_option_vec_u8(tx, &response.normalized_err_eval_reason_hash.0);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_OFFCHAIN_RESPONSE_EVIDENCE.module,
            interface::Verifier::NEW_OFFCHAIN_RESPONSE_EVIDENCE.name,
        ),
        vec![
            status_code,
            response_hash,
            response_signature,
            normalized_err_eval_reason_hash,
        ],
    ))
}

fn prepare_offchain_verifier_evidence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    expected_vertex: sui::tx::Argument,
    evidence: &AuthenticatedOffchainVerifierEvidence,
) -> anyhow::Result<sui::tx::Argument> {
    let submission_kind = prepare_submission_kind(
        tx,
        objects.interface_pkg_id,
        evidence.submission_kind.clone(),
    );
    let payload_or_reason_hash = tx.pure(&evidence.payload_or_reason_hash);
    let transport_proof = tx.pure(&evidence.transport_proof);
    let request = prepare_authenticated_offchain_request_evidence(
        tx,
        objects,
        execution,
        leader_cap,
        expected_vertex,
        &evidence.request,
    )?;
    let response = prepare_offchain_response_evidence(tx, objects, &evidence.response)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_OFFCHAIN_VERIFIER_EVIDENCE.module,
            interface::Verifier::NEW_OFFCHAIN_VERIFIER_EVIDENCE.name,
        ),
        vec![
            submission_kind,
            payload_or_reason_hash,
            transport_proof,
            request,
            response,
        ],
    ))
}

fn prepare_offchain_verifier_evidence_for_preflight(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    evidence: &OffchainVerifierEvidence,
) -> anyhow::Result<sui::tx::Argument> {
    let submission_kind = prepare_submission_kind(
        tx,
        objects.interface_pkg_id,
        evidence.submission_kind.clone(),
    );
    let payload_or_reason_hash = tx.pure(&evidence.payload_or_reason_hash);
    let transport_proof = tx.pure(&evidence.transport_proof);
    let request =
        prepare_raw_offchain_request_evidence_for_preflight(tx, objects, &evidence.request)?;
    let response = prepare_offchain_response_evidence(tx, objects, &evidence.response)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_OFFCHAIN_VERIFIER_EVIDENCE.module,
            interface::Verifier::NEW_OFFCHAIN_VERIFIER_EVIDENCE.name,
        ),
        vec![
            submission_kind,
            payload_or_reason_hash,
            transport_proof,
            request,
            response,
        ],
    ))
}

pub struct ExternalVerifierCallResult {
    pub worksheet: sui::tx::Argument,
    pub result: sui::tx::Argument,
}

fn external_verifier_call_results(
    call: sui::tx::Argument,
) -> anyhow::Result<ExternalVerifierCallResult> {
    let mut results = call.to_nested(2).into_iter();
    Ok(ExternalVerifierCallResult {
        worksheet: results
            .next()
            .ok_or_else(|| anyhow::anyhow!("external verifier call missing worksheet result"))?,
        result: results
            .next()
            .ok_or_else(|| anyhow::anyhow!("external verifier call missing verifier result"))?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn call_external_verifier_v1_with_authenticated_request(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    expected_vertex: sui::tx::Argument,
    verifier_evidence: &AuthenticatedOffchainVerifierEvidence,
    runtime_call: &ExternalVerifierRuntimeCall,
) -> anyhow::Result<ExternalVerifierCallResult> {
    let witness = tx.object(sui::tx::ObjectInput::shared(
        *runtime_call.witness.object_id(),
        runtime_call.witness.version(),
        true,
    ));
    let shared_objects = runtime_call
        .shared_objects
        .iter()
        .map(|(shared, object_ref)| {
            tx.object(sui::tx::ObjectInput::shared(
                *object_ref.object_id(),
                object_ref.version(),
                shared.ref_mut,
            ))
        })
        .collect::<Vec<_>>();
    let verifier_evidence = prepare_offchain_verifier_evidence(
        tx,
        objects,
        execution,
        leader_cap,
        expected_vertex,
        verifier_evidence,
    )?;
    let module = sui::types::Identifier::from_str(&runtime_call.module_name)?;
    let function = sui::types::Identifier::from_str(&runtime_call.function_name)?;

    let call = tx.move_call(
        sui::tx::Function::new(runtime_call.package_address, module, function),
        {
            let mut args = Vec::with_capacity(shared_objects.len() + 3);
            args.push(witness);
            args.extend(shared_objects);
            args.push(worksheet);
            args.push(verifier_evidence);
            args
        },
    );
    external_verifier_call_results(call)
}

/// Preflight-only verifier contract call.
///
/// Active submit builders create request evidence through the workflow package
/// from authenticated `DAGExecution` and `leader_cap` arguments instead.
pub fn call_external_verifier_v1(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    worksheet: sui::tx::Argument,
    verifier_evidence: &OffchainVerifierEvidence,
    runtime_call: &ExternalVerifierRuntimeCall,
) -> anyhow::Result<ExternalVerifierCallResult> {
    let witness = tx.object(sui::tx::ObjectInput::shared(
        *runtime_call.witness.object_id(),
        runtime_call.witness.version(),
        true,
    ));
    let shared_objects = runtime_call
        .shared_objects
        .iter()
        .map(|(shared, object_ref)| {
            tx.object(sui::tx::ObjectInput::shared(
                *object_ref.object_id(),
                object_ref.version(),
                shared.ref_mut,
            ))
        })
        .collect::<Vec<_>>();
    let verifier_evidence =
        prepare_offchain_verifier_evidence_for_preflight(tx, objects, verifier_evidence)?;
    let module = sui::types::Identifier::from_str(&runtime_call.module_name)?;
    let function = sui::types::Identifier::from_str(&runtime_call.function_name)?;

    let call = tx.move_call(
        sui::tx::Function::new(runtime_call.package_address, module, function),
        {
            let mut args = Vec::with_capacity(shared_objects.len() + 3);
            args.push(witness);
            args.extend(shared_objects);
            args.push(worksheet);
            args.push(verifier_evidence);
            args
        },
    );
    external_verifier_call_results(call)
}

#[allow(clippy::too_many_arguments)]
pub fn commit_off_chain_tool_result_for_walk_v1(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    tool_registry: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    result: &crate::types::PreparedToolOutput,
    auxiliary: Option<&OffChainToolResultAuxiliary>,
    proof: &OffChainVerifierProof,
    verifier_registry: sui::tx::Argument,
    leader_registry: sui::tx::Argument,
    network_auth: sui::tx::Argument,
    leader_key_binding: sui::tx::Argument,
    tool_key_binding: sui::tx::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.pure(&walk_index);
    let expected_vertex =
        interface::Graph::runtime_vertex_from_enum(tx, objects.interface_pkg_id, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, objects, result)?;
    let auxiliary_bytes = tx.pure(&auxiliary.map(bcs::to_bytes).transpose()?);
    let proof = prepare_offchain_verifier_proof(tx, objects, proof)?;

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name,
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            walk_index,
            expected_vertex,
            result_bytes,
            auxiliary_bytes,
            proof,
            verifier_registry,
            leader_registry,
            network_auth,
            leader_key_binding,
            tool_key_binding,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn commit_off_chain_tool_result_for_walk_without_verifier_v1(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    result: &crate::types::PreparedToolOutput,
    auxiliary: Option<&OffChainToolResultAuxiliary>,
    leader_registry: sui::tx::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.pure(&walk_index);
    let expected_vertex =
        interface::Graph::runtime_vertex_from_enum(tx, objects.interface_pkg_id, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, objects, result)?;
    let auxiliary_bytes = tx.pure(&auxiliary.map(bcs::to_bytes).transpose()?);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.module,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.name,
        ),
        vec![
            dag,
            execution,
            worksheet,
            leader_cap,
            walk_index,
            expected_vertex,
            result_bytes,
            auxiliary_bytes,
            leader_registry,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn commit_off_chain_tool_result_for_walk_with_external_verifier_proof_v1(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    tool_registry: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    result: &crate::types::PreparedToolOutput,
    verifier_evidence: &AuthenticatedOffchainVerifierEvidence,
    communication_evidence: &[u8],
    runtime_call: &ExternalVerifierRuntimeCall,
    verifier_registry: sui::tx::Argument,
    leader_registry: sui::tx::Argument,
    network_auth: sui::tx::Argument,
    leader_key_binding: sui::tx::Argument,
    tool_key_binding: sui::tx::Argument,
) -> anyhow::Result<()> {
    let expected_vertex_arg =
        interface::Graph::runtime_vertex_from_enum(tx, objects.interface_pkg_id, expected_vertex)?;
    let verifier_call = call_external_verifier_v1_with_authenticated_request(
        tx,
        objects,
        execution,
        leader_cap,
        worksheet,
        expected_vertex_arg,
        verifier_evidence,
        runtime_call,
    )?;
    let worksheet = verifier_call.worksheet;
    let communication_evidence = tx.pure(&communication_evidence.to_vec());
    let external_verifier_evidence = tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE.module,
            interface::Verifier::NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE.name,
        ),
        vec![verifier_call.result, communication_evidence],
    );
    let proof = tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER.module,
            interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER.name,
        ),
        vec![external_verifier_evidence],
    );
    let auxiliary = tx.pure(&Option::<Vec<u8>>::None);
    let walk_index = tx.pure(&walk_index);
    let expected_vertex =
        interface::Graph::runtime_vertex_from_enum(tx, objects.interface_pkg_id, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, objects, result)?;

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name,
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            walk_index,
            expected_vertex,
            result_bytes,
            auxiliary,
            proof,
            verifier_registry,
            leader_registry,
            network_auth,
            leader_key_binding,
            tool_key_binding,
        ],
    );

    Ok(())
}

pub fn prepare_tool_result_submission_worksheet(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    walk_index: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let walk_index = tx.pure(&walk_index);

    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        false,
    ));
    let leader_registry = leader_registry_arg(tx, objects);
    let clock = clock_arg(tx);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::PREPARE_TOOL_RESULT_SUBMISSION_WORKSHEET.module,
            workflow::ExecutionSubmission::PREPARE_TOOL_RESULT_SUBMISSION_WORKSHEET.name,
        ),
        vec![
            dag,
            agent_registry,
            leader_registry,
            execution,
            leader_cap,
            walk_index,
            clock,
        ],
    ))
}

fn leader_registry_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> sui::tx::Argument {
    tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ))
}

fn clock_arg(tx: &mut sui::tx::TransactionBuilder) -> sui::tx::Argument {
    tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn release_vertex_authorization_for_onchain_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    walk_index: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let walk_index = tx.pure(&walk_index);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::RELEASE_VERTEX_AUTHORIZATION_FOR_ONCHAIN_WALK.module,
            workflow::ExecutionSubmission::RELEASE_VERTEX_AUTHORIZATION_FOR_ONCHAIN_WALK.name,
        ),
        vec![dag, execution, worksheet, leader_cap, walk_index],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn commit_on_chain_tool_result_for_walk_v1(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    tool_registry: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    leader_registry: sui::tx::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    prepared_output: &PreparedToolOutput,
    failure_evidence_kind: Option<&FailureEvidenceKind>,
    submitted_failure_reason: Option<Vec<u8>>,
    tool_witness_id: sui::types::Address,
) -> anyhow::Result<()> {
    let walk_index = tx.pure(&walk_index);
    let expected_vertex =
        interface::Graph::runtime_vertex_from_enum(tx, objects.interface_pkg_id, expected_vertex)?;
    let (output_variant, output_ports_data) = prepare_tool_output(tx, objects, prepared_output)?;
    let failure_evidence_kind =
        prepare_move_option_failure_evidence_kind(tx, objects, failure_evidence_kind);
    let submitted_failure_reason = prepare_move_option_vec_u8(tx, &submitted_failure_reason);
    let tool_witness_id = sui_framework::Object::id_from_object_id(tx, tool_witness_id)?;

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name,
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            leader_registry,
            walk_index,
            expected_vertex,
            output_variant,
            output_ports_data,
            failure_evidence_kind,
            submitted_failure_reason,
            tool_witness_id,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn commit_on_chain_tool_result_for_walk_v1_with_args(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    tool_registry: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    leader_registry: sui::tx::Argument,
    walk_index: u64,
    expected_vertex: sui::tx::Argument,
    output_variant: sui::tx::Argument,
    output_ports_data: sui::tx::Argument,
    failure_evidence_kind: Option<&FailureEvidenceKind>,
    submitted_failure_reason: Option<Vec<u8>>,
    tool_witness_id: sui::types::Address,
) -> anyhow::Result<()> {
    let walk_index = tx.pure(&walk_index);
    let failure_evidence_kind =
        prepare_move_option_failure_evidence_kind(tx, objects, failure_evidence_kind);
    let submitted_failure_reason = prepare_move_option_vec_u8(tx, &submitted_failure_reason);
    let tool_witness_id = sui_framework::Object::id_from_object_id(tx, tool_witness_id)?;

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name,
        ),
        vec![
            dag,
            execution,
            tool_registry,
            worksheet,
            leader_cap,
            leader_registry,
            walk_index,
            expected_vertex,
            output_variant,
            output_ports_data,
            failure_evidence_kind,
            submitted_failure_reason,
            tool_witness_id,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn commit_on_chain_terminal_err_eval_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    tool_registry: sui::tx::Argument,
    worksheet: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    leader_registry: sui::tx::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    reason: NexusData,
    failure_evidence_kind: &FailureEvidenceKind,
    tool_witness_id: Option<sui::types::Address>,
) -> anyhow::Result<()> {
    commit_on_chain_tool_result_for_walk_v1(
        tx,
        objects,
        dag,
        execution,
        tool_registry,
        worksheet,
        leader_cap,
        leader_registry,
        walk_index,
        expected_vertex,
        &PreparedToolOutput::terminal_err_eval(reason),
        Some(failure_evidence_kind),
        None,
        tool_witness_id.unwrap_or(sui::types::Address::ZERO),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn record_committed_tool_result_gas_charge_by_leader(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) {
    let walk_index = tx.pure(&walk_index);
    let commit_tx_digest = tx.pure(&commit_tx_digest);
    let commit_gas_charge = tx.pure(&commit_gas_charge);
    let settlement_gas_charge = tx.pure(&settlement_gas_charge);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::RECORD_COMMITTED_TOOL_RESULT_GAS_CHARGE_BY_LEADER.module,
            workflow::ExecutionSettlement::RECORD_COMMITTED_TOOL_RESULT_GAS_CHARGE_BY_LEADER.name,
        ),
        vec![
            execution,
            leader_cap,
            walk_index,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
        ],
    );
}

#[allow(clippy::too_many_arguments)]
pub fn settle_committed_tool_result_for_walk_by_leader(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    tool_registry: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
    clock: sui::tx::Argument,
) {
    let walk_index = tx.pure(&walk_index);
    let commit_tx_digest = tx.pure(&commit_tx_digest);
    let commit_gas_charge = tx.pure(&commit_gas_charge);
    let settlement_gas_charge = tx.pure(&settlement_gas_charge);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK_BY_LEADER.module,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK_BY_LEADER.name,
        ),
        vec![
            dag,
            execution,
            tool_registry,
            leader_cap,
            walk_index,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
            clock,
        ],
    );
}

pub fn settle_committed_tool_result_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    tool_registry: sui::tx::Argument,
    walk_index: u64,
    clock: sui::tx::Argument,
) {
    let walk_index = tx.pure(&walk_index);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK.module,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK.name,
        ),
        vec![dag, execution, tool_registry, walk_index, clock],
    );
}

pub fn emit_payment_ready_walk_requests(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
    leader_registry: sui::tx::Argument,
    clock: sui::tx::Argument,
) {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionSettlement::EMIT_PAYMENT_READY_WALK_REQUESTS.module,
            workflow::ExecutionSettlement::EMIT_PAYMENT_READY_WALK_REQUESTS.name,
        ),
        vec![dag, execution, leader_registry, clock],
    );
}

pub fn committed_tool_result_settlement_status_raw(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::tx::Argument,
    walk_index: u64,
) -> sui::tx::Argument {
    let walk_index = tx.pure(&walk_index);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Execution::COMMITTED_TOOL_RESULT_SETTLEMENT_STATUS_RAW.module,
            workflow::Execution::COMMITTED_TOOL_RESULT_SETTLEMENT_STATUS_RAW.name,
        ),
        vec![execution, walk_index],
    )
}

/// PTB template for creating a new DAG default value.
pub fn create_default_value(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    default_value: &DefaultValue,
) -> anyhow::Result<sui::tx::Argument> {
    // `vertex: Vertex`
    let vertex =
        interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, &default_value.vertex)?;

    // `port: InputPort`
    let port = interface::Graph::input_port_from_str(
        tx,
        objects.interface_pkg_id,
        &default_value.input_port,
    )?;

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
            objects.interface_pkg_id,
            interface::Dag::WITH_DEFAULT_VALUE.module,
            interface::Dag::WITH_DEFAULT_VALUE.name,
        ),
        vec![dag, vertex, port, value],
    ))
}

/// PTB template for creating a new DAG edge.
pub fn create_edge(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    edge: &Edge,
) -> anyhow::Result<sui::tx::Argument> {
    // `from_vertex: Vertex`
    let from_vertex =
        interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, &edge.from.vertex)?;

    // `from_variant: OutputVariant`
    let from_variant = interface::Graph::output_variant_from_str(
        tx,
        objects.interface_pkg_id,
        &edge.from.output_variant,
    )?;

    // `from_port: OutputPort`
    let from_port = interface::Graph::output_port_from_str(
        tx,
        objects.interface_pkg_id,
        &edge.from.output_port,
    )?;

    // `to_vertex: Vertex`
    let to_vertex =
        interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, &edge.to.vertex)?;

    // `to_port: InputPort`
    let to_port =
        interface::Graph::input_port_from_str(tx, objects.interface_pkg_id, &edge.to.input_port)?;

    // `kind: EdgeKind`
    let kind = interface::Graph::edge_kind_from_enum(tx, objects.interface_pkg_id, &edge.kind);

    // `dag.with_edge(from_vertex, from_variant, from_port, to_vertex, to_port)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::WITH_EDGE.module,
            interface::Dag::WITH_EDGE.name,
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
    dag: sui::tx::Argument,
    output: &FromPort,
) -> anyhow::Result<sui::tx::Argument> {
    // `vertex: Vertex`
    let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, &output.vertex)?;

    // `variant: OutputVariant`
    let variant = interface::Graph::output_variant_from_str(
        tx,
        objects.interface_pkg_id,
        &output.output_variant,
    )?;

    // `port: OutputPort`
    let port =
        interface::Graph::output_port_from_str(tx, objects.interface_pkg_id, &output.output_port)?;

    // `dag.with_output(vertex, variant, port)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::WITH_OUTPUT.module,
            interface::Dag::WITH_OUTPUT.name,
        ),
        vec![dag, vertex, variant, port],
    ))
}

/// PTB template for marking a vertex as an entry vertex.
pub fn mark_entry_vertex(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    vertex: &str,
    entry_group: &str,
) -> anyhow::Result<sui::tx::Argument> {
    // `vertex: Vertex`
    let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex)?;

    // `entry_group: EntryGroup`
    let entry_group =
        interface::Graph::entry_group_from_str(tx, objects.interface_pkg_id, entry_group)?;

    // `dag.with_entry_in_group(vertex, entry_group)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::WITH_ENTRY_IN_GROUP.module,
            interface::Dag::WITH_ENTRY_IN_GROUP.name,
        ),
        vec![dag, vertex, entry_group],
    ))
}

/// PTB template for marking an input port as an input port.
pub fn mark_entry_input_port(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::tx::Argument,
    vertex: &str,
    entry_port: &EntryPort,
    entry_group: &str,
) -> anyhow::Result<sui::tx::Argument> {
    // `vertex: Vertex`
    let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex)?;

    // `entry_port: InputPort`
    let entry_port =
        interface::Graph::input_port_from_str(tx, objects.interface_pkg_id, &entry_port.name)?;

    // `entry_group: EntryGroup`
    let entry_group =
        interface::Graph::entry_group_from_str(tx, objects.interface_pkg_id, entry_group)?;

    // `dag.with_entry_port_in_group(vertex, entry_port, entry_group)`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            interface::Dag::WITH_ENTRY_PORT_IN_GROUP.module,
            interface::Dag::WITH_ENTRY_PORT_IN_GROUP.name,
        ),
        vec![dag, vertex, entry_port, entry_group],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn begin_user_funded_agent_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_registry: sui::tx::Argument,
    agent_registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    dag: sui::tx::Argument,
    _dag_id: sui::tx::Argument,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    payment_coin: sui::tx::Argument,
    clock: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    // `network: ID`
    let network = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;

    // `entry_group: EntryGroup`
    let entry_group =
        interface::Graph::entry_group_from_str(tx, objects.interface_pkg_id, entry_group)?;

    // `with_vertex_inputs: VecMap<Vertex, VecMap<InputPort, NexusData>>`
    let inner_vec_map_type = vec![
        interface::into_type_tag(objects.interface_pkg_id, interface::Graph::INPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        interface::into_type_tag(objects.interface_pkg_id, interface::Graph::VERTEX),
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
        )
        .with_type_args(outer_vec_map_type.clone()),
        vec![],
    );

    for (vertex_name, data) in input_data {
        let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex_name)?;
        let with_vertex_input = tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::EMPTY.module,
                sui_framework::VecMap::EMPTY.name,
            )
            .with_type_args(inner_vec_map_type.clone()),
            vec![],
        );

        for (port_name, value) in data {
            let port = interface::Graph::input_port_from_str(
                tx,
                objects.interface_pkg_id,
                port_name.as_str(),
            )?;

            let value = match value.storage_kind() {
                StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    &value.as_json(),
                )?,
                StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    &value.as_json(),
                )?,
            };

            tx.move_call(
                sui::tx::Function::new(
                    sui_framework::PACKAGE_ID,
                    sui_framework::VecMap::INSERT.module,
                    sui_framework::VecMap::INSERT.name,
                )
                .with_type_args(inner_vec_map_type.clone()),
                vec![with_vertex_input, port, value],
            );
        }

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
            )
            .with_type_args(outer_vec_map_type.clone()),
            vec![with_vertex_inputs, vertex, with_vertex_input],
        );
    }

    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);

    let agent_id = sui_framework::Object::id_from_object_id(tx, agent_execution.agent_id)?;
    let agent_config = tap::agent_execution_config_arg(
        tx,
        objects,
        agent_id,
        network,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
        agent_execution.skill_id,
        agent_execution.selected_dag,
        &agent_execution.authorization_templates,
    )?;

    let payment_max_budget = tx.pure(&agent_execution.payment_max_budget);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.module,
            workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.name,
        ),
        vec![
            dag,
            agent_registry,
            agent,
            tool_registry,
            agent_config,
            payment_coin,
            payment_max_budget,
            clock,
        ],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn begin_default_dag_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_registry: sui::tx::Argument,
    agent_registry: sui::tx::Argument,
    dag: sui::tx::Argument,
    dag_id: sui::tx::Argument,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    payment_coin: sui::tx::Argument,
    clock: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    // `network: ID`
    let network = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;

    // `entry_group: EntryGroup`
    let entry_group =
        interface::Graph::entry_group_from_str(tx, objects.interface_pkg_id, entry_group)?;

    // `with_vertex_inputs: VecMap<Vertex, VecMap<InputPort, NexusData>>`
    let inner_vec_map_type = vec![
        interface::into_type_tag(objects.interface_pkg_id, interface::Graph::INPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        interface::into_type_tag(objects.interface_pkg_id, interface::Graph::VERTEX),
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
        )
        .with_type_args(outer_vec_map_type.clone()),
        vec![],
    );

    for (vertex_name, data) in input_data {
        let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex_name)?;
        let with_vertex_input = tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::EMPTY.module,
                sui_framework::VecMap::EMPTY.name,
            )
            .with_type_args(inner_vec_map_type.clone()),
            vec![],
        );

        for (port_name, value) in data {
            let port = interface::Graph::input_port_from_str(
                tx,
                objects.interface_pkg_id,
                port_name.as_str(),
            )?;

            let value = match value.storage_kind() {
                StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    &value.as_json(),
                )?,
                StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    &value.as_json(),
                )?,
            };

            tx.move_call(
                sui::tx::Function::new(
                    sui_framework::PACKAGE_ID,
                    sui_framework::VecMap::INSERT.module,
                    sui_framework::VecMap::INSERT.name,
                )
                .with_type_args(inner_vec_map_type.clone()),
                vec![with_vertex_input, port, value],
            );
        }

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
            )
            .with_type_args(outer_vec_map_type.clone()),
            vec![with_vertex_inputs, vertex, with_vertex_input],
        );
    }

    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);
    let config = tap::default_agent_execution_config_arg(
        tx,
        objects,
        dag_id,
        network,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
    )?;

    let payment_max_budget = tx.pure(&agent_execution.payment_max_budget);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionEntries::BEGIN_DEFAULT_DAG_EXECUTION.module,
            workflow::ExecutionEntries::BEGIN_DEFAULT_DAG_EXECUTION.name,
        ),
        vec![
            dag,
            agent_registry,
            tool_registry,
            config,
            payment_coin,
            payment_max_budget,
            clock,
        ],
    ))
}

/// PTB template to lock execution payment state for the given tools.
#[allow(clippy::too_many_arguments)]
pub fn lock_payment_state_for_tools(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tools_gas: Vec<sui::tx::Argument>,
    dag: sui::tx::Argument,
    execution: sui::tx::Argument,
) {
    for tool_gas in tools_gas {
        // `nexus_workflow::gas::lock_payment_state_for_tool()`
        tx.move_call(
            sui::tx::Function::new(
                objects.workflow_pkg_id,
                workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.module,
                workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.name,
            ),
            vec![tool_gas, dag, execution],
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_agent_dag(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    agent: AgentInput,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    execute_agent_dag_internal(
        tx,
        objects,
        dag,
        Some(agent),
        priority_fee_per_gas_unit,
        entry_group,
        input_data,
        agent_execution,
        tools_gas,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn execute_default_agent_dag(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    execute_agent_dag_internal(
        tx,
        objects,
        dag,
        None,
        priority_fee_per_gas_unit,
        entry_group,
        input_data,
        agent_execution,
        tools_gas,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_agent_dag_internal(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    agent: Option<AgentInput>,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
    default_executor: bool,
) -> anyhow::Result<()> {
    let dag_id = sui_framework::Object::id_from_object_id(tx, *dag.object_id())?;
    let dag = tx.object(sui::tx::ObjectInput::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));

    let agent = match agent {
        Some(agent) => Some(agent.mutable_argument(tx)?),
        None => None,
    };

    let tool_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        false,
    ));

    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    let payment_coin = if let Some(payment_coin_ref) = agent_execution.payment_coin.as_ref() {
        let owned_payment_coin = tx.object(sui::tx::ObjectInput::owned(
            *payment_coin_ref.object_id(),
            payment_coin_ref.version(),
            *payment_coin_ref.digest(),
        ));
        match agent_execution.payment_coin_balance {
            Some(balance) if balance > agent_execution.payment_max_budget => {
                let payment_amount = tx.pure(&agent_execution.payment_max_budget);
                tx.split_coins(owned_payment_coin, vec![payment_amount])
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("failed to split TAP execution payment coin"))?
            }
            _ => owned_payment_coin,
        }
    } else {
        let payment_amount = tx.pure(&agent_execution.payment_max_budget);
        let gas = tx.gas();
        tx.split_coins(gas, vec![payment_amount])
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("failed to split TAP execution payment coin"))?
    };

    let results = if default_executor {
        begin_default_dag_execution(
            tx,
            objects,
            tool_registry,
            agent_registry,
            dag,
            dag_id,
            priority_fee_per_gas_unit,
            entry_group,
            input_data,
            agent_execution,
            payment_coin,
            clock,
        )?
    } else {
        let agent =
            agent.ok_or_else(|| anyhow::anyhow!("agent DAG execution requires an Agent input"))?;
        begin_user_funded_agent_execution(
            tx,
            objects,
            tool_registry,
            agent_registry,
            agent,
            dag,
            dag_id,
            priority_fee_per_gas_unit,
            entry_group,
            input_data,
            agent_execution,
            payment_coin,
            clock,
        )?
    };

    let execution = results;
    let gas_service = tx.object(sui::tx::ObjectInput::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        false,
    ));
    let tools_gas = tools_gas
        .iter()
        .map(|(address, version)| tx.object(sui::tx::ObjectInput::shared(*address, *version, true)))
        .collect();

    crate::transactions::gas::snapshot_dag_tool_costs(tx, objects, gas_service, execution, dag);
    lock_payment_state_for_tools(tx, objects, tools_gas, dag, execution);

    let leader_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionEntries::START_EXECUTION.module,
            workflow::ExecutionEntries::START_EXECUTION.name,
        ),
        vec![dag, execution, leader_registry, clock],
    );

    let execution_type =
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Execution::DAG_EXECUTION);
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
        )
        .with_type_args(vec![execution_type]),
        vec![execution],
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
        assert_matches::assert_matches,
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

        fn move_call_indices_to(
            &self,
            package: sui::types::Address,
            module: &sui::types::Identifier,
            function: &sui::types::Identifier,
        ) -> Vec<usize> {
            self.commands()
                .iter()
                .enumerate()
                .filter_map(|(index, command)| match command {
                    sui::types::Command::MoveCall(call)
                        if call.package == package
                            && &call.module == module
                            && &call.function == function =>
                    {
                        Some(index)
                    }
                    _ => None,
                })
                .collect()
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
            let sui::types::Input::Pure(value) = self.input(argument) else {
                panic!("expected pure input, got {:?}", self.input(argument));
            };

            let actual: sui::types::Address =
                bcs::from_bytes(value).expect("address BCS should deserialize");
            assert_eq!(actual, expected);
        }

        fn expect_u64(&self, argument: &sui::types::Argument, expected: u64) {
            let sui::types::Input::Pure(value) = self.input(argument) else {
                panic!("expected pure input, got {:?}", self.input(argument));
            };

            let actual: u64 = bcs::from_bytes(value).expect("u64 BCS should deserialize");
            assert_eq!(actual, expected);
        }

        fn expect_shared_object(
            &self,
            argument: &sui::types::Argument,
            expected: &sui::types::ObjectReference,
            expected_mutable: bool,
        ) {
            let sui::types::Input::Shared(shared) = self.input(argument) else {
                panic!("expected shared input, got {:?}", self.input(argument));
            };
            assert_eq!(shared.object_id(), *expected.object_id());
            assert_eq!(shared.version(), expected.version());
            assert_eq!(shared.mutability().is_mutable(), expected_mutable);
        }

        fn expect_string(&self, argument: &sui::types::Argument, expected: &str) {
            let sui::types::Input::Pure(value) = self.input(argument) else {
                panic!("expected pure input, got {:?}", self.input(argument));
            };

            let actual: String = bcs::from_bytes(value).expect("string BCS should deserialize");
            assert_eq!(actual, expected);
        }

        fn expect_ascii_string_result(&self, argument: &sui::types::Argument, expected: &str) {
            let sui::types::Argument::Result(index) = argument else {
                panic!("expected result argument, got {argument:?}");
            };
            let call = self.move_call(*index as usize);

            assert_eq!(call.package, crate::idents::move_std::PACKAGE_ID);
            assert_eq!(call.module, crate::idents::move_std::Ascii::STRING.module);
            assert_eq!(call.function, crate::idents::move_std::Ascii::STRING.name);
            self.expect_string(&call.arguments[0], expected);
        }
    }

    fn mock_runtime_vertex() -> RuntimeVertex {
        RuntimeVertex::plain("vertex1")
    }

    fn generated_id(
        bytes: sui::types::Address,
    ) -> crate::types::generated::sui_framework_types::object::ID {
        crate::types::sui_address_to_id(bytes)
    }

    fn generated_nexus_data(
        value: crate::types::NexusData,
    ) -> crate::types::generated::primitives_types::data::NexusData {
        bcs::from_bytes(&bcs::to_bytes(&value).expect("SDK NexusData should encode"))
            .expect("generated NexusData should decode")
    }

    fn mock_prepared_tool_output() -> PreparedToolOutput {
        PreparedToolOutput {
            output_variant: "ok".to_string(),
            output_ports_data: HashMap::from([(
                "result".to_string(),
                NexusData::try_from(serde_json::json!({
                    "kind": "inline",
                    "data": { "value": 7 }
                }))
                .expect("inline data storage"),
            )]),
        }
    }

    fn mock_offchain_success_result() -> crate::types::PreparedToolOutput {
        crate::types::PreparedToolOutput {
            output_variant: "ok".into(),
            output_ports_data: vec![crate::types::PreparedToolOutputPort {
                port: "result".into(),
                data: generated_nexus_data(crate::types::NexusData::new_inline(
                    serde_json::json!({ "value": 7 }),
                )),
            }],
        }
    }

    fn mock_authenticated_offchain_verifier_evidence(
    ) -> crate::types::AuthenticatedOffchainVerifierEvidence {
        crate::types::AuthenticatedOffchainVerifierEvidence {
            submission_kind: crate::types::VerificationSubmissionKind::Success,
            payload_or_reason_hash: vec![1, 2, 3],
            transport_proof: vec![4, 5, 6],
            request: crate::types::AuthenticatedOffchainRequestEvidence {
                walk_index: 9,
                tool_fqn: "xyz.test.tool@1".to_string(),
                request_hash: vec![7, 8],
                request_signature: vec![9, 10],
            },
            response: crate::types::OffchainResponseEvidence {
                status_code: 200,
                response_hash: vec![11, 12],
                response_signature: vec![13, 14],
                normalized_err_eval_reason_hash: crate::types::MoveOption(None),
            },
        }
    }

    fn mock_raw_offchain_verifier_evidence() -> crate::types::OffchainVerifierEvidence {
        crate::types::OffchainVerifierEvidence {
            submission_kind: crate::types::VerificationSubmissionKind::Success,
            payload_or_reason_hash: vec![1, 2, 3],
            transport_proof: vec![4, 5, 6],
            request: crate::types::OffchainRequestEvidence {
                execution: generated_id(sui_mocks::mock_sui_address()),
                walk_index: 9,
                vertex: "vertex1".into(),
                tool_fqn: "xyz.test.tool@1".into(),
                leader_cap_id: generated_id(sui_mocks::mock_sui_address()),
                request_hash: vec![7, 8],
                request_signature: vec![9, 10],
            },
            response: crate::types::OffchainResponseEvidence {
                status_code: 200,
                response_hash: vec![11, 12],
                response_signature: vec![13, 14],
                normalized_err_eval_reason_hash: crate::types::MoveOption(None),
            },
        }
    }

    fn mock_external_verifier_runtime_call() -> crate::types::ExternalVerifierRuntimeCall {
        let witness = sui_mocks::mock_sui_object_ref();
        let shared = sui_mocks::mock_sui_object_ref();
        crate::types::ExternalVerifierRuntimeCall {
            package_address: sui_mocks::mock_sui_address(),
            module_name: "demo_verifier".to_string(),
            function_name: "verify_offchain_result".to_string(),
            witness,
            shared_objects: vec![(
                crate::types::SharedObjectRef::new_imm(*shared.object_id()),
                shared,
            )],
        }
    }

    fn mock_offchain_failure_result() -> crate::types::PreparedToolOutput {
        crate::types::PreparedToolOutput {
            output_variant: "_err_eval".into(),
            output_ports_data: vec![crate::types::PreparedToolOutputPort {
                port: "reason".into(),
                data: generated_nexus_data(crate::types::NexusData::new_inline(serde_json::json!(
                    "failed"
                ))),
            }],
        }
    }

    fn mock_large_offchain_success_result() -> crate::types::PreparedToolOutput {
        let blob_id =
            "walrus-blob-id-0123456789abcdef0123456789abcdef-fedcba98765432100123456789abcdef";
        crate::types::PreparedToolOutput {
            output_variant: "ok".into(),
            output_ports_data: vec![crate::types::PreparedToolOutputPort {
                port: "result".into(),
                data: generated_nexus_data(crate::types::NexusData::new_walrus(serde_json::json!(
                    std::iter::repeat_n(blob_id, 1000).collect::<Vec<_>>()
                ))),
            }],
        }
    }

    fn mock_offchain_failure_auxiliary() -> crate::types::OffChainToolResultAuxiliary {
        crate::types::OffChainToolResultAuxiliary {
            reported_failure_evidence_kind: crate::types::MoveOption(Some(
                crate::types::FailureEvidenceKind::LeaderEvidence,
            )),
        }
    }

    fn mock_objects_with_verifier_registry() -> NexusObjects {
        let mut objects = sui_mocks::mock_nexus_objects();
        objects.verifier_registry = sui_mocks::mock_sui_object_ref();
        objects
    }

    #[test]
    fn test_prepared_tool_output_terminal_err_eval_shape() {
        let reason = NexusData::try_from(serde_json::json!({
            "kind": "inline",
            "data": "tool failed"
        }))
        .expect("inline terminal reason");
        let expected_reason = reason.as_json().clone();

        let prepared = PreparedToolOutput::terminal_err_eval(reason);

        assert_eq!(prepared.output_variant, "_err_eval");
        assert_eq!(prepared.output_ports_data.len(), 1);
        let reason = prepared
            .output_ports_data
            .get("reason")
            .expect("terminal reason port should be present");
        assert_eq!(reason.storage_kind(), StorageKind::Inline);
        assert_eq!(reason.as_json(), expected_reason);
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::NEW.module);
        assert_eq!(call.function, interface::Dag::NEW.name);
        assert_eq!(call.type_arguments.len(), 0);
        assert_eq!(call.arguments.len(), 0);
    }

    #[test]
    fn test_publish() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
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
    fn runtime_vertex_from_enum_builds_iterator_constructor_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let runtime_vertex = RuntimeVertex::with_iterator("loop_body", 2, 5);

        interface::Graph::runtime_vertex_from_enum(
            &mut tx,
            objects.interface_pkg_id,
            &runtime_vertex,
        )
        .unwrap();
        let tx = sui_mocks::mock_finish_transaction(tx);
        let inspector = TxInspector::new(tx);

        let indices = inspector.move_call_indices_to(
            objects.interface_pkg_id,
            &interface::Graph::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.module,
            &interface::Graph::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.name,
        );
        assert_eq!(indices.len(), 1);
        let call = inspector.move_call(indices[0]);
        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(
            call.module,
            interface::Graph::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.module
        );
        assert_eq!(
            call.function,
            interface::Graph::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.name
        );
        inspector.expect_ascii_string_result(&call.arguments[0], "loop_body");
        inspector.expect_u64(&call.arguments[1], 2);
        inspector.expect_u64(&call.arguments[2], 5);
    }

    #[test]
    fn test_create_vertex() {
        let objects = sui_mocks::mock_nexus_objects();
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
        let dag = tx.pure(&0u64);
        create_vertex(&mut tx, &objects, dag, &vertex).unwrap();
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
            call.package == objects.interface_pkg_id
                && call.module == interface::Graph::VERTEX_OFF_CHAIN.module
                && call.function == interface::Graph::VERTEX_OFF_CHAIN.name
        }));

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to create a vertex");
        };

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::WITH_VERTEX.module);
        assert_eq!(call.function, interface::Dag::WITH_VERTEX.name);
    }

    #[test]
    fn test_create_default_value() {
        let objects = sui_mocks::mock_nexus_objects();
        let default_value = DefaultValue {
            vertex: "vertex1".to_string(),
            input_port: "port1".to_string(),
            value: Data {
                storage: StorageKind::Inline,
                data: serde_json::json!({"key": "value"}),
            },
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::WITH_DEFAULT_VALUE.module);
        assert_eq!(call.function, interface::Dag::WITH_DEFAULT_VALUE.name);
    }

    #[test]
    fn test_create_post_failure_action() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::WITH_POST_FAILURE_ACTION.module);
        assert_eq!(call.function, interface::Dag::WITH_POST_FAILURE_ACTION.name);
    }

    #[test]
    fn test_create_vertex_post_failure_action() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(
            call.module,
            interface::Dag::WITH_VERTEX_POST_FAILURE_ACTION.module
        );
        assert_eq!(
            call.function,
            interface::Dag::WITH_VERTEX_POST_FAILURE_ACTION.name
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
        assert_eq!(
            call.module,
            workflow::ExecutionSettlement::ABORT_EXPIRED_EXECUTION.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSettlement::ABORT_EXPIRED_EXECUTION.name
        );
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(
            call.module,
            interface::Verifier::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE.module
        );
        assert_eq!(
            call.function,
            interface::Verifier::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE.name
        );
    }

    fn mock_offchain_registered_key_proof() -> crate::types::OffChainVerifierProof {
        crate::types::OffChainVerifierProof::RegisteredKey {
            verifier_credential: vec![1, 2, 3],
            communication_evidence: vec![4, 5, 6],
        }
    }

    fn mock_offchain_none_auxiliary() -> crate::types::OffChainToolResultAuxiliary {
        crate::types::OffChainToolResultAuxiliary {
            reported_failure_evidence_kind: crate::types::MoveOption(None),
        }
    }

    #[test]
    fn test_commit_off_chain_tool_result_for_walk_v1_with_registered_key_proof() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);
        let __ph_5 = tx.pure(&5u64);
        let __ph_6 = tx.pure(&6u64);
        let __ph_7 = tx.pure(&7u64);
        let __ph_8 = tx.pure(&8u64);
        let __ph_9 = tx.pure(&9u64);

        commit_off_chain_tool_result_for_walk_v1(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            __ph_4,
            9,
            &mock_runtime_vertex(),
            &mock_offchain_success_result(),
            Some(&mock_offchain_none_auxiliary()),
            &mock_offchain_registered_key_proof(),
            __ph_5,
            __ph_6,
            __ph_7,
            __ph_8,
            __ph_9,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 15);
        assert!(matches!(call.arguments[9], sui::types::Argument::Result(_)));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module
                            == interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY.module
                        && call.function
                            == interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY.name
            )
        }));
    }

    #[test]
    fn test_commit_off_chain_tool_result_for_walk_without_verifier_v1_routes_plain_commit() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);

        commit_off_chain_tool_result_for_walk_without_verifier_v1(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            9,
            &mock_runtime_vertex(),
            &mock_offchain_success_result(),
            None,
            __ph_4,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.name
        );
        assert_eq!(call.arguments.len(), 9);
    }

    #[test]
    fn test_commit_off_chain_tool_result_for_walk_without_verifier_v1_routes_failure_commit() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);

        commit_off_chain_tool_result_for_walk_without_verifier_v1(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            9,
            &mock_runtime_vertex(),
            &mock_offchain_failure_result(),
            Some(&mock_offchain_failure_auxiliary()),
            __ph_4,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.name
        );
        assert_eq!(call.arguments.len(), 9);
        assert!(matches!(call.arguments[6], sui::types::Argument::Result(_)));
        assert!(matches!(call.arguments[7], sui::types::Argument::Input(_)));
    }
    #[test]
    fn test_commit_off_chain_tool_result_for_walk_v1_large_result_avoids_oversized_pure_input() {
        let result = mock_large_offchain_success_result();
        assert!(
            bcs::to_bytes(&result)
                .expect("large offchain result should encode")
                .len()
                > MAX_PURE_INPUT_BYTES
        );

        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);

        commit_off_chain_tool_result_for_walk_without_verifier_v1(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            9,
            &mock_runtime_vertex(),
            &result,
            None,
            __ph_4,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let commit_call = inspector.move_call(inspector.commands().len() - 1);
        let result_arg = commit_call
            .arguments
            .get(6)
            .expect("commit call should include result bytes argument");

        assert!(
            matches!(result_arg, sui::types::Argument::Result(_)),
            "large result should be produced by a prior Move call"
        );
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module
                            == interface::Verifier::PREPARED_TOOL_OUTPUT_INTO_BCS_BYTES.module
                        && call.function
                            == interface::Verifier::PREPARED_TOOL_OUTPUT_INTO_BCS_BYTES.name
            )
        }));
        assert!(inspector.inputs().iter().all(|input| match input {
            sui::types::Input::Pure(value) => value.len() <= MAX_PURE_INPUT_BYTES,
            _ => true,
        }));
    }

    #[test]
    fn test_commit_off_chain_tool_result_for_walk_v1_with_external_verifier_proof_builds_typed_ptb()
    {
        let objects = sui_mocks::mock_nexus_objects();
        let runtime_call = mock_external_verifier_runtime_call();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);
        let __ph_5 = tx.pure(&5u64);
        let __ph_6 = tx.pure(&6u64);
        let __ph_7 = tx.pure(&7u64);
        let __ph_8 = tx.pure(&8u64);
        let __ph_9 = tx.pure(&9u64);

        commit_off_chain_tool_result_for_walk_with_external_verifier_proof_v1(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            __ph_4,
            9,
            &mock_runtime_vertex(),
            &mock_offchain_success_result(),
            &mock_authenticated_offchain_verifier_evidence(),
            &[21, 22, 23],
            &runtime_call,
            __ph_5,
            __ph_6,
            __ph_7,
            __ph_8,
            __ph_9,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let verifier_call_index = inspector
            .commands()
            .iter()
            .position(|command| {
                matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.package == runtime_call.package_address
                            && call.module
                                == sui::types::Identifier::from_static("demo_verifier")
                            && call.function
                                == sui::types::Identifier::from_static("verify_offchain_result")
                )
            })
            .expect("expected external verifier runtime call");
        let verifier_call = inspector.move_call(verifier_call_index);
        assert_eq!(verifier_call.arguments.len(), 4);
        assert!(matches!(
            verifier_call.arguments[0],
            sui::types::Argument::Input(_)
        ));
        assert!(matches!(
            verifier_call.arguments[1],
            sui::types::Argument::Input(_)
        ));
        inspector.expect_u64(&verifier_call.arguments[2], 3);
        assert!(matches!(
            verifier_call.arguments[3],
            sui::types::Argument::Result(_)
        ));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.workflow_pkg_id
                        && call.module
                            == workflow::ExecutionSubmission::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE.module
                        && call.function
                            == workflow::ExecutionSubmission::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE.name
            )
        }));
        assert!(!inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == interface::Verifier::NEW_OFFCHAIN_REQUEST_EVIDENCE.module
                        && call.function == interface::Verifier::NEW_OFFCHAIN_REQUEST_EVIDENCE.name
            )
        }));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module
                            == interface::Verifier::NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE.module
                        && call.function
                            == interface::Verifier::NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE.name
            )
        }));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module
                            == interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER.module
                        && call.function
                            == interface::Verifier::NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER.name
            )
        }));

        let commit_call = inspector.move_call(inspector.commands().len() - 1);
        assert_eq!(commit_call.package, objects.workflow_pkg_id);
        assert_eq!(
            commit_call.module,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            commit_call.function,
            workflow::ExecutionSubmission::COMMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(commit_call.arguments.len(), 15);
        assert!(matches!(
            commit_call.arguments[9],
            sui::types::Argument::Result(_)
        ));
    }

    #[test]
    fn test_call_external_verifier_v1_keeps_raw_request_evidence_for_preflight() {
        let objects = sui_mocks::mock_nexus_objects();
        let runtime_call = mock_external_verifier_runtime_call();
        let mut tx = sui::tx::TransactionBuilder::new();
        let worksheet_ref = sui_mocks::mock_sui_object_ref();
        let worksheet = tx.object(sui::tx::ObjectInput::owned(
            *worksheet_ref.object_id(),
            worksheet_ref.version(),
            *worksheet_ref.digest(),
        ));

        let call = call_external_verifier_v1(
            &mut tx,
            &objects,
            worksheet,
            &mock_raw_offchain_verifier_evidence(),
            &runtime_call,
        )
        .unwrap();

        let _worksheet = call.worksheet;
        let _result = call.result;

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == interface::Verifier::NEW_OFFCHAIN_REQUEST_EVIDENCE.module
                        && call.function == interface::Verifier::NEW_OFFCHAIN_REQUEST_EVIDENCE.name
            )
        }));
        assert!(!inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.workflow_pkg_id
                        && call.module
                            == workflow::ExecutionSubmission::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE.module
                        && call.function
                            == workflow::ExecutionSubmission::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE.name
            )
        }));
    }

    #[test]
    fn test_commit_on_chain_tool_result_for_walk_v1() {
        let objects = sui_mocks::mock_nexus_objects();
        let witness = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);
        let __ph_5 = tx.pure(&5u64);

        commit_on_chain_tool_result_for_walk_v1(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            __ph_4,
            __ph_5,
            9,
            &mock_runtime_vertex(),
            &mock_prepared_tool_output(),
            None,
            None,
            witness,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_on_chain_submit_call_uses_sdk_move_values(
            &inspector,
            call,
            objects.workflow_pkg_id,
            objects.interface_pkg_id,
            witness,
        );
    }

    #[test]
    fn test_prepare_tool_result_submission_worksheet_matches_current_move_signature() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
        let execution = tx.pure(&1u64);
        let leader_cap = tx.pure(&2u64);

        prepare_tool_result_submission_worksheet(&mut tx, &objects, dag, execution, leader_cap, 3)
            .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::PREPARE_TOOL_RESULT_SUBMISSION_WORKSHEET.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::PREPARE_TOOL_RESULT_SUBMISSION_WORKSHEET.name
        );
        assert_eq!(call.arguments.len(), 7);
    }

    #[test]
    fn test_record_committed_tool_result_gas_charge_by_leader_builds_record_only_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let execution = tx.pure(&1u64);
        let leader_cap = tx.pure(&2u64);

        record_committed_tool_result_gas_charge_by_leader(
            &mut tx,
            &objects,
            execution,
            leader_cap,
            7,
            vec![9, 8, 7],
            123,
            45,
        );

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSettlement::RECORD_COMMITTED_TOOL_RESULT_GAS_CHARGE_BY_LEADER.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSettlement::RECORD_COMMITTED_TOOL_RESULT_GAS_CHARGE_BY_LEADER.name
        );
        assert_eq!(call.arguments.len(), 6);
        inspector.expect_u64(&call.arguments[2], 7);
        let sui::types::Input::Pure(digest_bytes) = inspector.input(&call.arguments[3]) else {
            panic!("expected digest pure input");
        };
        let digest: Vec<u8> = bcs::from_bytes(digest_bytes).expect("digest vector BCS");
        assert_eq!(digest, vec![9, 8, 7]);
        inspector.expect_u64(&call.arguments[4], 123);
        inspector.expect_u64(&call.arguments[5], 45);
    }

    #[test]
    fn test_settle_committed_tool_result_for_walk_by_leader_submits_gas_update() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
        let execution = tx.pure(&1u64);
        let tool_registry = tx.pure(&2u64);
        let leader_cap = tx.pure(&3u64);
        let clock = tx.pure(&4u64);

        settle_committed_tool_result_for_walk_by_leader(
            &mut tx,
            &objects,
            dag,
            execution,
            tool_registry,
            leader_cap,
            7,
            vec![9, 8, 7],
            123,
            45,
            clock,
        );

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK_BY_LEADER.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK_BY_LEADER.name
        );
        assert_eq!(call.arguments.len(), 9);
        inspector.expect_u64(&call.arguments[4], 7);
        let sui::types::Input::Pure(digest_bytes) = inspector.input(&call.arguments[5]) else {
            panic!("expected digest pure input");
        };
        let digest: Vec<u8> = bcs::from_bytes(digest_bytes).expect("digest vector BCS");
        assert_eq!(digest, vec![9, 8, 7]);
        inspector.expect_u64(&call.arguments[6], 123);
        inspector.expect_u64(&call.arguments[7], 45);
    }

    #[test]
    fn test_settle_committed_tool_result_for_walk_builds_permissionless_settlement() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
        let execution = tx.pure(&1u64);
        let tool_registry = tx.pure(&2u64);
        let clock = tx.pure(&3u64);

        settle_committed_tool_result_for_walk(
            &mut tx,
            &objects,
            dag,
            execution,
            tool_registry,
            11,
            clock,
        );

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSettlement::SETTLE_COMMITTED_TOOL_RESULT_FOR_WALK.name
        );
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_u64(&call.arguments[3], 11);
    }

    #[test]
    fn test_emit_payment_ready_walk_requests_builds_post_lock_request_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
        let execution = tx.pure(&1u64);
        let leader_registry = tx.pure(&2u64);
        let clock = tx.pure(&3u64);

        emit_payment_ready_walk_requests(&mut tx, &objects, dag, execution, leader_registry, clock);

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSettlement::EMIT_PAYMENT_READY_WALK_REQUESTS.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSettlement::EMIT_PAYMENT_READY_WALK_REQUESTS.name
        );
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn test_refill_tap_execution_payment_builds_coin_refill_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let execution = tx.pure(&1u64);
        let coin = tx.pure(&2u64);

        refill_tap_execution_payment(&mut tx, &objects, execution, coin);

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT.name
        );
        assert_eq!(call.arguments.len(), 2);
    }

    #[test]
    fn test_refill_tap_execution_payment_from_agent_vault_builds_vault_refill_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let agent = tx.pure(&1u64);
        let execution = tx.pure(&2u64);

        refill_tap_execution_payment_from_agent_vault(&mut tx, &objects, agent, execution, 333);

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSettlement::REFILL_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name
        );
        assert_eq!(call.arguments.len(), 3);
        inspector.expect_u64(&call.arguments[2], 333);
    }

    #[test]
    fn test_committed_tool_result_settlement_status_raw_builds_execution_view_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let execution = tx.pure(&0u64);

        committed_tool_result_settlement_status_raw(&mut tx, &objects, execution, 19);

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Execution::COMMITTED_TOOL_RESULT_SETTLEMENT_STATUS_RAW.module
        );
        assert_eq!(
            call.function,
            workflow::Execution::COMMITTED_TOOL_RESULT_SETTLEMENT_STATUS_RAW.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_u64(&call.arguments[1], 19);
    }

    #[test]
    fn test_commit_on_chain_tool_result_for_walk_v1_with_args_constructs_move_values() {
        let objects = sui_mocks::mock_nexus_objects();
        let witness = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);
        let __ph_5 = tx.pure(&5u64);
        let __ph_6 = tx.pure(&6u64);
        let __ph_7 = tx.pure(&7u64);
        let __ph_8 = tx.pure(&8u64);

        commit_on_chain_tool_result_for_walk_v1_with_args(
            &mut tx, &objects, __ph_0, __ph_1, __ph_2, __ph_3, __ph_4, __ph_5, 9, __ph_6, __ph_7,
            __ph_8, None, None, witness,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_on_chain_submit_call_uses_sdk_move_values(
            &inspector,
            call,
            objects.workflow_pkg_id,
            objects.interface_pkg_id,
            witness,
        );
    }

    #[test]
    fn test_release_vertex_authorization_for_onchain_walk_uses_active_walk_args() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
        let execution = tx.pure(&1u64);
        let worksheet = tx.pure(&2u64);
        let leader_cap = tx.pure(&3u64);

        release_vertex_authorization_for_onchain_walk(
            &mut tx, &objects, dag, execution, worksheet, leader_cap, 17,
        )
        .expect("release helper should build");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::RELEASE_VERTEX_AUTHORIZATION_FOR_ONCHAIN_WALK.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::RELEASE_VERTEX_AUTHORIZATION_FOR_ONCHAIN_WALK.name
        );
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_u64(&call.arguments[0], 0);
        inspector.expect_u64(&call.arguments[1], 1);
        inspector.expect_u64(&call.arguments[2], 2);
        inspector.expect_u64(&call.arguments[3], 3);
        inspector.expect_u64(&call.arguments[4], 17);
    }

    fn assert_on_chain_submit_call_uses_sdk_move_values(
        inspector: &TxInspector,
        call: &sui::types::MoveCall,
        workflow_pkg_id: sui::types::Address,
        interface_pkg_id: sui::types::Address,
        expected_tool_witness_id: sui::types::Address,
    ) {
        assert_eq!(call.package, workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 13);
        assert!(matches!(
            call.arguments[10],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        assert!(matches!(
            call.arguments[12],
            sui::types::Argument::Result(_)
        ));

        let failure_option = match call.arguments[10] {
            sui::types::Argument::Result(index) => inspector.move_call(index as usize),
            other => panic!("expected failure option to be a command result, got {other:?}"),
        };
        assert_eq!(failure_option.package, move_std::PACKAGE_ID);
        assert_eq!(failure_option.module, move_std::Option::NONE.module);
        assert_eq!(failure_option.function, move_std::Option::NONE.name);
        assert_eq!(
            failure_option.type_arguments,
            vec![workflow::into_type_tag(
                interface_pkg_id,
                interface::Verifier::FAILURE_EVIDENCE_KIND
            )]
        );

        let tool_witness_id = match call.arguments[12] {
            sui::types::Argument::Result(index) => inspector.move_call(index as usize),
            other => panic!("expected tool witness ID to be a command result, got {other:?}"),
        };
        assert_eq!(tool_witness_id.package, sui_framework::PACKAGE_ID);
        assert_eq!(
            tool_witness_id.function,
            sui_framework::Object::ID_FROM_ADDRESS.name
        );
        inspector.expect_address(&tool_witness_id.arguments[0], expected_tool_witness_id);
    }

    #[test]
    fn test_commit_on_chain_tool_result_for_walk_v1_with_failure_evidence() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_witness_id = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);
        let __ph_5 = tx.pure(&5u64);

        commit_on_chain_tool_result_for_walk_v1(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            __ph_4,
            __ph_5,
            11,
            &mock_runtime_vertex(),
            &mock_prepared_tool_output(),
            Some(&FailureEvidenceKind::ToolEvidence),
            Some(b"tool failed".to_vec()),
            tool_witness_id,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 13);
        assert!(matches!(
            call.arguments[10],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        assert!(matches!(
            call.arguments[12],
            sui::types::Argument::Result(_)
        ));
    }

    #[test]
    fn test_commit_on_chain_terminal_err_eval_for_walk_defaults_zero_tool_witness_id() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);
        let __ph_5 = tx.pure(&5u64);

        commit_on_chain_terminal_err_eval_for_walk(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            __ph_4,
            __ph_5,
            13,
            &mock_runtime_vertex(),
            NexusData::try_from(serde_json::json!({
                "kind": "inline",
                "data": "tool failed"
            }))
            .expect("inline reason"),
            &FailureEvidenceKind::LeaderEvidence,
            None,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 13);
        assert!(matches!(
            call.arguments[10],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        assert!(matches!(
            call.arguments[12],
            sui::types::Argument::Result(_)
        ));
    }

    #[test]
    fn test_commit_on_chain_terminal_err_eval_for_walk_builds_terminal_output_with_tool_witness_id()
    {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_witness_id = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let __ph_0 = tx.pure(&0u64);
        let __ph_1 = tx.pure(&1u64);
        let __ph_2 = tx.pure(&2u64);
        let __ph_3 = tx.pure(&3u64);
        let __ph_4 = tx.pure(&4u64);
        let __ph_5 = tx.pure(&5u64);

        commit_on_chain_terminal_err_eval_for_walk(
            &mut tx,
            &objects,
            __ph_0,
            __ph_1,
            __ph_2,
            __ph_3,
            __ph_4,
            __ph_5,
            13,
            &mock_runtime_vertex(),
            NexusData::try_from(serde_json::json!({
                "kind": "inline",
                "data": "tool failed"
            }))
            .expect("inline reason"),
            &FailureEvidenceKind::LeaderEvidence,
            Some(tool_witness_id),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::ExecutionSubmission::COMMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 13);
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        let sui::types::Argument::Result(tool_witness_id_index) = &call.arguments[12] else {
            panic!("expected tool witness ID result argument");
        };
        let tool_witness_id_call = inspector.move_call(*tool_witness_id_index as usize);
        assert_eq!(
            tool_witness_id_call.module,
            sui_framework::Object::ID_FROM_ADDRESS.module
        );
        assert_eq!(
            tool_witness_id_call.function,
            sui_framework::Object::ID_FROM_ADDRESS.name
        );
        inspector.expect_address(&tool_witness_id_call.arguments[0], tool_witness_id);

        let sui::types::Argument::Result(output_variant_index) = &call.arguments[8] else {
            panic!("expected output variant result argument");
        };
        let output_variant_call = inspector.move_call(*output_variant_index as usize);
        assert_eq!(output_variant_call.package, objects.interface_pkg_id);
        assert_eq!(
            output_variant_call.module,
            interface::Graph::OUTPUT_VARIANT_FROM_STRING.module
        );
        assert_eq!(
            output_variant_call.function,
            interface::Graph::OUTPUT_VARIANT_FROM_STRING.name
        );
        inspector.expect_ascii_string_result(&output_variant_call.arguments[0], "_err_eval");

        let output_port_indices = inspector.move_call_indices_to(
            objects.interface_pkg_id,
            &interface::Graph::OUTPUT_PORT_FROM_STRING.module,
            &interface::Graph::OUTPUT_PORT_FROM_STRING.name,
        );
        assert_eq!(
            output_port_indices.len(),
            1,
            "terminal _err_eval should build one output port"
        );
        let output_port_call = inspector.move_call(output_port_indices[0]);
        inspector.expect_ascii_string_result(&output_port_call.arguments[0], "reason");
    }

    #[test]
    fn test_create_edge() {
        let objects = sui_mocks::mock_nexus_objects();
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
        let dag = tx.pure(&0u64);
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::WITH_EDGE.module);
        assert_eq!(call.function, interface::Dag::WITH_EDGE.name);
    }

    #[test]
    fn test_mark_entry_vertex() {
        let objects = sui_mocks::mock_nexus_objects();
        let vertex = "vertex1";
        let entry_group = "group1";

        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::WITH_ENTRY_IN_GROUP.module);
        assert_eq!(call.function, interface::Dag::WITH_ENTRY_IN_GROUP.name);
    }

    #[test]
    fn test_mark_entry_input_port() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let vertex = "vertex1";
        let entry_port = &EntryPort {
            name: "test".to_string(),
        };
        let entry_group = "group1";

        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
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

        assert_eq!(call.package, nexus_objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::WITH_ENTRY_PORT_IN_GROUP.module);
        assert_eq!(call.function, interface::Dag::WITH_ENTRY_PORT_IN_GROUP.name);
    }

    #[test]
    fn test_execute_agent_dag() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let entry_group = "group1";
        let mut input_data = HashMap::new();
        let tools_gas = HashSet::from([(sui_mocks::mock_sui_address(), 0)]);

        input_data.insert(
            "vertex1".to_string(),
            HashMap::from([(
                "port1".to_string(),
                serde_json::json!({"kind": "inline", "data": { "key": "value"} })
                    .try_into()
                    .expect("Failed to convert JSON to NexusData"),
            )]),
        );

        let mut tx = sui::tx::TransactionBuilder::new();
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui_mocks::mock_sui_address(),
            skill_id: 11,
            selected_dag: None,
            authorization_templates: Vec::new(),
            payment_source: vec![1, 2],
            payment_coin: None,
            payment_coin_balance: None,
            payment_max_budget: 55,
        };

        execute_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            AgentInput::Shared(agent.clone()),
            13,
            entry_group,
            &input_data,
            &agent_execution,
            &tools_gas,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let begin_index = inspector
            .commands()
            .iter()
            .position(|command| match command {
                sui::types::Command::MoveCall(call) => {
                    call.function
                        == workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.name
                }
                _ => false,
            })
            .expect("agent begin call");
        let agent_config_index = inspector
            .move_call_indices_to(
                nexus_objects.interface_pkg_id,
                &crate::idents::interface::Agent::NEW_AGENT_EXECUTION_CONFIG.module,
                &crate::idents::interface::Agent::NEW_AGENT_EXECUTION_CONFIG.name,
            )
            .into_iter()
            .next()
            .expect("agent execution config call");
        let start_index = inspector
            .commands()
            .iter()
            .position(|command| match command {
                sui::types::Command::MoveCall(call) => {
                    call.function == workflow::ExecutionEntries::START_EXECUTION.name
                }
                _ => false,
            })
            .expect("start execution call");
        let agent_config_call = inspector.move_call(agent_config_index);
        assert_eq!(
            agent_config_call.function,
            crate::idents::interface::Agent::NEW_AGENT_EXECUTION_CONFIG.name
        );
        assert_eq!(agent_config_call.arguments.len(), 8);
        inspector.expect_u64(&agent_config_call.arguments[5], agent_execution.skill_id);

        let begin_call = inspector.move_call(begin_index);
        assert_eq!(
            begin_call.function,
            workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.name
        );
        assert_eq!(begin_call.arguments.len(), 8);
        inspector.expect_shared_object(&begin_call.arguments[2], &agent, true);
        assert_matches!(&begin_call.arguments[4], sui::types::Argument::Result(_));
        assert_matches!(
            &begin_call.arguments[5],
            sui::types::Argument::NestedResult(_, 0)
        );

        let lock_call = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call)
                    if call.function == workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.name =>
                {
                    Some(call)
                }
                _ => None,
            })
            .next()
            .expect("tool payment lock call");
        assert_eq!(lock_call.arguments.len(), 3);

        let start_call = inspector.move_call(start_index);
        assert_eq!(
            start_call.function,
            workflow::ExecutionEntries::START_EXECUTION.name
        );
        assert_eq!(start_call.arguments.len(), 4);

        assert!(!inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.function == sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name
                        && call.type_arguments.first()
                            == Some(&sui::types::TypeTag::Struct(Box::new(
                                sui::types::StructTag::new(
                                    nexus_objects.interface_pkg_id,
                                    crate::idents::tap::STANDARD_PAYMENT_MODULE,
                                    sui::types::Identifier::from_static("ExecutionPayment"),
                                    vec![],
                                ),
                            )))
            )
        }));
    }

    #[test]
    fn execute_agent_dag_rejects_immutable_agent() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let entry_group = "group1";
        let input_data = HashMap::new();
        let tools_gas = HashSet::new();
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            selected_dag: None,
            authorization_templates: Vec::new(),
            payment_source: vec![1],
            payment_coin: None,
            payment_coin_balance: None,
            payment_max_budget: 55,
        };
        let mut tx = sui::tx::TransactionBuilder::new();

        let error = execute_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            AgentInput::Immutable(agent.clone()),
            0,
            entry_group,
            &input_data,
            &agent_execution,
            &tools_gas,
        )
        .expect_err("immutable agent cannot start execution");

        assert!(
            error.to_string().contains(&agent.object_id().to_string()),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn execute_agent_dag_with_owned_payment_coin_builds_move_values() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let payment_coin = sui_mocks::mock_sui_object_ref();
        let entry_group = "group1";
        let input_data = HashMap::new();
        let tools_gas = HashSet::new();
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            selected_dag: None,
            authorization_templates: Vec::new(),
            payment_source: vec![1, 2],
            payment_coin: Some(payment_coin.clone()),
            payment_coin_balance: Some(1_000),
            payment_max_budget: 55,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        execute_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            AgentInput::Shared(agent.clone()),
            0,
            entry_group,
            &input_data,
            &agent_execution,
            &tools_gas,
        )
        .expect("agent DAG builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert!(
            inspector
                .commands()
                .iter()
                .any(|command| matches!(command, sui::types::Command::SplitCoins(_))),
            "owned payment coin with excess balance should be split"
        );

        let begin_index = inspector
            .move_call_indices_to(
                nexus_objects.workflow_pkg_id,
                &workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.module,
                &workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.name,
            )
            .into_iter()
            .next()
            .expect("agent begin call");
        let begin_call = inspector.move_call(begin_index);
        assert_matches!(
            begin_call.arguments[5],
            sui::types::Argument::NestedResult(_, 0)
        );
    }

    #[test]
    fn execute_default_agent_dag_uses_registry_owned_default_entrypoint() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            selected_dag: None,
            authorization_templates: Vec::new(),
            payment_source: vec![1],
            payment_coin: None,
            payment_coin_balance: None,
            payment_max_budget: 3,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        execute_default_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            17,
            "group1",
            &HashMap::new(),
            &agent_execution,
            &HashSet::new(),
        )
        .expect("default agent DAG builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(calls.iter().any(|call| {
            call.package == nexus_objects.interface_pkg_id
                && call.module
                    == crate::idents::interface::Agent::NEW_DEFAULT_AGENT_EXECUTION_CONFIG.module
                && call.function
                    == crate::idents::interface::Agent::NEW_DEFAULT_AGENT_EXECUTION_CONFIG.name
        }));
        assert!(calls.iter().any(|call| {
            call.package == nexus_objects.workflow_pkg_id
                && call.module == workflow::ExecutionEntries::BEGIN_DEFAULT_DAG_EXECUTION.module
                && call.function == workflow::ExecutionEntries::BEGIN_DEFAULT_DAG_EXECUTION.name
        }));
        let config_index = inspector
            .move_call_indices_to(
                nexus_objects.interface_pkg_id,
                &crate::idents::interface::Agent::NEW_DEFAULT_AGENT_EXECUTION_CONFIG.module,
                &crate::idents::interface::Agent::NEW_DEFAULT_AGENT_EXECUTION_CONFIG.name,
            )
            .into_iter()
            .next()
            .expect("default execution config call");
        inspector.expect_u64(&inspector.move_call(config_index).arguments[4], 17);
        assert!(!calls.iter().any(|call| {
            call.package == nexus_objects.interface_pkg_id
                && call.module == crate::idents::interface::Agent::NEW_AGENT_EXECUTION_CONFIG.module
                && call.function == crate::idents::interface::Agent::NEW_AGENT_EXECUTION_CONFIG.name
        }));
        let shared_inputs = inspector
            .inputs()
            .iter()
            .filter_map(|input| match input {
                sui::types::Input::Shared(shared) => {
                    Some((shared.object_id(), shared.mutability().is_mutable()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(shared_inputs
            .iter()
            .any(|(id, mutable)| { id == nexus_objects.agent_registry.object_id() && !*mutable }));
    }

    #[test]
    fn agent_dag_builders_do_not_use_legacy_workflow_witness_stamp_idents() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let entry_group = "group1";
        let mut input_data = HashMap::new();
        let tools_gas = HashSet::from([(sui_mocks::mock_sui_address(), 0)]);

        input_data.insert(
            "vertex1".to_string(),
            HashMap::from([(
                "port1".to_string(),
                serde_json::json!({"kind": "inline", "data": { "key": "value"} })
                    .try_into()
                    .expect("Failed to convert JSON to NexusData"),
            )]),
        );

        let mut tx = sui::tx::TransactionBuilder::new();
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui_mocks::mock_sui_address(),
            skill_id: 11,
            selected_dag: None,
            authorization_templates: Vec::new(),
            payment_source: vec![1],
            payment_coin: None,
            payment_coin_balance: None,
            payment_max_budget: 3,
        };
        execute_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            AgentInput::Shared(agent.clone()),
            0,
            entry_group,
            &input_data,
            &agent_execution,
            &tools_gas,
        )
        .expect("agent DAG builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            calls.iter().any(|call| {
                call.package == nexus_objects.workflow_pkg_id
                    && call.module
                        == workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.module
                    && call.function
                        == workflow::ExecutionEntries::BEGIN_USER_FUNDED_AGENT_EXECUTION.name
            }),
            "agent DAG execution must use the explicit agent entrypoint"
        );
    }

    #[test]
    fn test_create_output() {
        let objects = sui_mocks::mock_nexus_objects();
        let output = FromPort {
            vertex: "vertex1".to_string(),
            output_variant: "variant1".to_string(),
            output_port: "port1".to_string(),
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.pure(&0u64);
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

        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, interface::Dag::WITH_OUTPUT.module);
        assert_eq!(call.function, interface::Dag::WITH_OUTPUT.name);
    }

    #[test]
    fn test_create_wires_post_failure_actions() {
        let objects = sui_mocks::mock_nexus_objects();
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
        let dag_arg = tx.pure(&0u64);
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
            call.package == objects.interface_pkg_id
                && call.module == interface::Dag::WITH_VERTEX.module
                && call.function == interface::Dag::WITH_VERTEX.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.package == objects.interface_pkg_id
                && call.module == interface::Dag::WITH_VERTEX_POST_FAILURE_ACTION.module
                && call.function == interface::Dag::WITH_VERTEX_POST_FAILURE_ACTION.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.package == objects.interface_pkg_id
                && call.module == interface::Dag::WITH_POST_FAILURE_ACTION.module
                && call.function == interface::Dag::WITH_POST_FAILURE_ACTION.name
        }));
    }

    #[test]
    fn test_create_wires_verifier_config() {
        let objects = mock_objects_with_verifier_registry();
        let dag = Dag {
            vertices: vec![Vertex {
                kind: VertexKind::OffChain {
                    tool_fqn: fqn!("xyz.tool.test@1"),
                },
                name: "vertex1".to_string(),
                entry_ports: None,
                post_failure_action: None,
                leader_verifier: Some(VerifierConfig {
                    mode: VerifierMode::LeaderRegisteredKey,
                    method: "nautilus_v1".into(),
                }),
                tool_verifier: Some(VerifierConfig {
                    mode: VerifierMode::ToolVerifierContract,
                    method: "demo_verifier_v1".into(),
                }),
            }],
            edges: vec![],
            default_values: None,
            post_failure_action: None,
            leader_verifier: Some(VerifierConfig {
                mode: VerifierMode::LeaderRegisteredKey,
                method: "signed_http_v1".into(),
            }),
            tool_verifier: Some(VerifierConfig {
                mode: VerifierMode::None,
                method: "none".into(),
            }),
            entry_groups: None,
            outputs: None,
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        let dag_arg = tx.pure(&0u64);
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
            call.package == objects.registry_pkg_id
                && call.module == registry::VerifierRegistry::WITH_DEFAULT_LEADER_VERIFIER.module
                && call.function == registry::VerifierRegistry::WITH_DEFAULT_LEADER_VERIFIER.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.package == objects.registry_pkg_id
                && call.module == registry::VerifierRegistry::WITH_DEFAULT_TOOL_VERIFIER.module
                && call.function == registry::VerifierRegistry::WITH_DEFAULT_TOOL_VERIFIER.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.package == objects.registry_pkg_id
                && call.module == registry::VerifierRegistry::WITH_VERTEX_LEADER_VERIFIER.module
                && call.function == registry::VerifierRegistry::WITH_VERTEX_LEADER_VERIFIER.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.package == objects.registry_pkg_id
                && call.module == registry::VerifierRegistry::WITH_VERTEX_TOOL_VERIFIER.module
                && call.function == registry::VerifierRegistry::WITH_VERTEX_TOOL_VERIFIER.name
        }));
        assert_eq!(
            move_calls
                .iter()
                .filter(|call| {
                    call.package == objects.interface_pkg_id
                        && call.module == interface::Verifier::VERIFIER_CONFIG.module
                        && call.function == interface::Verifier::VERIFIER_CONFIG.name
                })
                .count(),
            4
        );
    }
}
