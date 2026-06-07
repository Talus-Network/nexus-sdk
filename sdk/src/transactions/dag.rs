use {
    crate::{
        idents::{move_std, primitives, pure_arg, sui_framework, workflow},
        sui,
        transactions::tap,
        types::{
            AgentId,
            AuthenticatedOffchainRequestEvidenceV1,
            AuthenticatedOffchainVerifierEvidenceV1,
            Dag,
            DataStorage,
            DefaultValue,
            Edge,
            EntryPort,
            ExternalVerifierRuntimeCallV1,
            FailureEvidenceKind,
            FromPort,
            NexusObjects,
            OffChainToolResultAuxiliaryV1,
            OffChainVerifierProofV1,
            OffchainRequestEvidenceV1,
            OffchainResponseEvidenceV1,
            OffchainVerifierEvidenceV1,
            PostFailureAction,
            PreparedToolOutputV1,
            RuntimeVertex,
            SkillId,
            Storable,
            StorageKind,
            TapVertexAuthorizationPlanEntry,
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
const NEW_OFFCHAIN_REQUEST_EVIDENCE_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_offchain_request_evidence_v1");
const NEW_OFFCHAIN_RESPONSE_EVIDENCE_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_offchain_response_evidence_v1");
const NEW_OFFCHAIN_VERIFIER_EVIDENCE_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_offchain_verifier_evidence_v1");
const NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_external_verifier_submit_evidence_v1");
const NEW_VERIFIER_CONTRACT_RESULT_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_verifier_contract_result_v1");
const NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_off_chain_verifier_proof_registered_key_v1");
const NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER_V1: sui::types::Identifier =
    sui::types::Identifier::from_static("new_off_chain_verifier_proof_external_verifier_v1");
const VERIFIER_SUBMISSION_KIND_SUCCESS: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_submission_kind_success");
const VERIFIER_SUBMISSION_KIND_ERR_EVAL: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_submission_kind_err_eval");
const VERIFIER_EVIDENCE_KIND_TOOL_EVIDENCE: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_evidence_kind_tool_evidence");
const VERIFIER_EVIDENCE_KIND_LEADER_EVIDENCE: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_evidence_kind_leader_evidence");
const VERIFIER_DECISION_ACCEPT: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_decision_accept");
const VERIFIER_DECISION_REJECT: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_decision_reject");
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentDagExecuteInput {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub payment_source: Vec<u8>,
    pub payment_coin: Option<sui::types::ObjectReference>,
    pub payment_coin_balance: Option<u64>,
    pub payment_max_budget: u64,
    pub payment_total_budget: Option<u64>,
    pub payment_refund_mode: u8,
    pub authorization_plan_commitment: Option<Vec<u8>>,
    pub authorization_plan: Vec<crate::types::TapVertexAuthorizationPlanEntry>,
}

fn tap_authorization_grant_ref_type(objects: &NexusObjects) -> sui::types::TypeTag {
    workflow::into_type_tag(
        objects.workflow_pkg_id,
        workflow::Dag::TAP_AUTHORIZATION_GRANT_REF,
    )
}

fn prepare_option_address(
    tx: &mut sui::tx::TransactionBuilder,
    value: Option<sui::types::Address>,
) -> anyhow::Result<sui::types::Argument> {
    let element = sui::types::TypeTag::Address;
    match value {
        Some(value) => {
            let value = tx.input(pure_arg(&value)?);
            Ok(move_std::Option::some(tx, element, value))
        }
        None => Ok(move_std::Option::none(tx, element)),
    }
}

fn prepare_option_u64(
    tx: &mut sui::tx::TransactionBuilder,
    value: Option<u64>,
) -> anyhow::Result<sui::types::Argument> {
    let element = sui::types::TypeTag::U64;
    match value {
        Some(value) => {
            let value = tx.input(pure_arg(&value)?);
            Ok(move_std::Option::some(tx, element, value))
        }
        None => Ok(move_std::Option::none(tx, element)),
    }
}

fn prepare_option_interface_revision(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    value: Option<crate::types::InterfaceRevision>,
) -> anyhow::Result<sui::types::Argument> {
    let element = crate::idents::tap::interface_revision_type(objects.interface_pkg_id);
    match value {
        Some(value) => {
            let value = crate::transactions::tap::interface_revision(tx, objects, value)?;
            Ok(move_std::Option::some(tx, element, value))
        }
        None => Ok(move_std::Option::none(tx, element)),
    }
}

fn tap_authorization_grant_ref(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    entry: &TapVertexAuthorizationPlanEntry,
) -> anyhow::Result<sui::types::Argument> {
    let vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, &entry.vertex)?;
    let grant_id = tx.input(pure_arg(&entry.grant_id)?);
    let tool_package = tx.input(pure_arg(&entry.tool_package)?);
    let tool_module = move_std::Ascii::ascii_string_from_str(tx, &entry.tool_module)?;
    let tool_function = move_std::Ascii::ascii_string_from_str(tx, &entry.tool_function)?;
    let operation_commitment = tx.input(pure_arg(&entry.operation_commitment)?);
    let constraints_commitment = tx.input(pure_arg(&entry.constraints_commitment)?);
    let endpoint_revision =
        prepare_option_interface_revision(tx, objects, entry.endpoint_revision)?;
    let payment_id = prepare_option_address(tx, entry.payment_id)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::TAP_AUTHORIZATION_GRANT_REF_CONSTRUCTOR.module,
            workflow::Dag::TAP_AUTHORIZATION_GRANT_REF_CONSTRUCTOR.name,
            vec![],
        ),
        vec![
            vertex,
            grant_id,
            tool_package,
            tool_module,
            tool_function,
            operation_commitment,
            constraints_commitment,
            endpoint_revision,
            payment_id,
        ],
    ))
}

pub(crate) fn tap_authorization_plan(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    entries: &[TapVertexAuthorizationPlanEntry],
) -> anyhow::Result<sui::types::Argument> {
    let element_type = tap_authorization_grant_ref_type(objects);
    let mut args = Vec::with_capacity(entries.len());
    for entry in entries {
        args.push(tap_authorization_grant_ref(tx, objects, entry)?);
    }

    Ok(tx.make_move_vec(Some(element_type), args))
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

fn priority_fee_vault_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.input(sui::tx::Input::shared(
        *objects.priority_fee_vault.object_id(),
        objects.priority_fee_vault.version(),
        true,
    )))
}

fn required_total_budget(
    payment_max_budget: u64,
    priority_fee_excess_quote: Option<u64>,
) -> anyhow::Result<u64> {
    let effective_priority_fee_quote =
        crate::types::effective_priority_fee_quote(priority_fee_excess_quote)?;
    let priority_fee =
        crate::types::priority_fee_for_gas(payment_max_budget, effective_priority_fee_quote)?;
    payment_max_budget
        .checked_add(priority_fee)
        .ok_or_else(|| anyhow::anyhow!("payment total budget overflows u64"))
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

pub fn refund_tap_execution_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
    refund_reason: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let refund_reason = tx.input(pure_arg(&refund_reason)?);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::REFUND_TAP_EXECUTION_PAYMENT.module,
            workflow::Dag::REFUND_TAP_EXECUTION_PAYMENT.name,
            vec![],
        ),
        vec![execution, refund_reason],
    ))
}

pub fn refund_tap_execution_payment_from_agent_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
    execution: sui::types::Argument,
    refund_reason: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let refund_reason = tx.input(pure_arg(&refund_reason)?);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::REFUND_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module,
            workflow::Dag::REFUND_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name,
            vec![],
        ),
        vec![agent, execution, refund_reason],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn refund_scheduled_tap_execution_payment_from_agent_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
    scheduled_task: sui::types::Argument,
    execution: sui::types::Argument,
    refund_reason: Vec<u8>,
    continue_recurring: bool,
    next_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let refund_reason = tx.input(pure_arg(&refund_reason)?);
    let continue_recurring = tx.input(pure_arg(&continue_recurring)?);
    let next_after_ms = tx.input(pure_arg(&next_after_ms)?);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::REFUND_SCHEDULED_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.module,
            workflow::Dag::REFUND_SCHEDULED_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT.name,
            vec![],
        ),
        vec![
            agent,
            scheduled_task,
            execution,
            refund_reason,
            continue_recurring,
            next_after_ms,
        ],
    ))
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

fn prepare_move_option_vec_u8(
    tx: &mut sui::tx::TransactionBuilder,
    value: &Option<Vec<u8>>,
) -> sui::types::Argument {
    tx.input(pure_arg(value).expect("option<vector<u8>> should encode"))
}

fn prepare_move_option_failure_evidence_kind(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    value: Option<&FailureEvidenceKind>,
) -> sui::types::Argument {
    match value {
        Some(value) => {
            let kind = create_failure_evidence_kind(tx, objects, value);
            tx.move_call(
                sui::tx::Function::new(
                    move_std::PACKAGE_ID,
                    move_std::Option::SOME.module,
                    move_std::Option::SOME.name,
                    vec![workflow::into_type_tag(
                        objects.workflow_pkg_id,
                        workflow::Dag::FAILURE_EVIDENCE_KIND,
                    )],
                ),
                vec![kind],
            )
        }
        None => tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Option::NONE.module,
                move_std::Option::NONE.name,
                vec![workflow::into_type_tag(
                    objects.workflow_pkg_id,
                    workflow::Dag::FAILURE_EVIDENCE_KIND,
                )],
            ),
            vec![],
        ),
    }
}

fn prepare_submission_kind(
    tx: &mut sui::tx::TransactionBuilder,
    interface_pkg_id: sui::types::Address,
    submission_kind: crate::types::VerificationSubmissionKind,
) -> sui::types::Argument {
    let function = match submission_kind {
        crate::types::VerificationSubmissionKind::Success => VERIFIER_SUBMISSION_KIND_SUCCESS,
        crate::types::VerificationSubmissionKind::ErrEval => VERIFIER_SUBMISSION_KIND_ERR_EVAL,
    };

    tx.move_call(
        sui::tx::Function::new(interface_pkg_id, VERIFIER_V1_MODULE, function, vec![]),
        vec![],
    )
}

fn prepare_verifier_evidence_kind(
    tx: &mut sui::tx::TransactionBuilder,
    interface_pkg_id: sui::types::Address,
    failure_evidence_kind: FailureEvidenceKind,
) -> sui::types::Argument {
    let function = match failure_evidence_kind {
        FailureEvidenceKind::ToolEvidence => VERIFIER_EVIDENCE_KIND_TOOL_EVIDENCE,
        FailureEvidenceKind::LeaderEvidence => VERIFIER_EVIDENCE_KIND_LEADER_EVIDENCE,
    };

    tx.move_call(
        sui::tx::Function::new(interface_pkg_id, VERIFIER_V1_MODULE, function, vec![]),
        vec![],
    )
}

fn prepare_verifier_decision(
    tx: &mut sui::tx::TransactionBuilder,
    interface_pkg_id: sui::types::Address,
    decision: crate::types::VerifierDecisionV1,
) -> sui::types::Argument {
    let function = match decision {
        crate::types::VerifierDecisionV1::Accept => VERIFIER_DECISION_ACCEPT,
        crate::types::VerifierDecisionV1::Reject => VERIFIER_DECISION_REJECT,
    };

    tx.move_call(
        sui::tx::Function::new(interface_pkg_id, VERIFIER_V1_MODULE, function, vec![]),
        vec![],
    )
}

fn prepare_failure_evidence_kind_option(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    value: Option<&FailureEvidenceKind>,
) -> sui::types::Argument {
    let element = workflow::into_type_tag(
        objects.workflow_pkg_id,
        workflow::Dag::FAILURE_EVIDENCE_KIND,
    );

    match value {
        Some(kind) => {
            let kind = create_failure_evidence_kind(tx, objects, kind);
            move_std::Option::some(tx, element, kind)
        }
        None => move_std::Option::none(tx, element),
    }
}

fn prepare_object_id(
    tx: &mut sui::tx::TransactionBuilder,
    object_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    sui_framework::Object::id_from_object_id(tx, object_id)
}

pub fn prepare_on_chain_tool_result_submission_v1_bytes(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    output_variant: sui::types::Argument,
    output_ports_data: sui::types::Argument,
    failure_evidence_kind: Option<&FailureEvidenceKind>,
    submitted_failure_reason: &Option<Vec<u8>>,
    tool_witness_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let failure_evidence_kind =
        prepare_failure_evidence_kind_option(tx, objects, failure_evidence_kind);
    let submitted_failure_reason = prepare_move_option_vec_u8(tx, submitted_failure_reason);
    let tool_witness_id = prepare_object_id(tx, tool_witness_id)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::ON_CHAIN_TOOL_RESULT_SUBMISSION_V1_BYTES.module,
            workflow::Dag::ON_CHAIN_TOOL_RESULT_SUBMISSION_V1_BYTES.name,
            vec![],
        ),
        vec![
            output_variant,
            output_ports_data,
            failure_evidence_kind,
            submitted_failure_reason,
            tool_witness_id,
        ],
    ))
}

fn prepare_verifier_contract_result(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    result: &crate::types::VerifierContractResultV1,
) -> anyhow::Result<sui::types::Argument> {
    let method = move_std::Ascii::ascii_string_from_str(tx, &result.method)?;
    let decision = prepare_verifier_decision(tx, objects.interface_pkg_id, result.decision);
    let submission_kind =
        prepare_submission_kind(tx, objects.interface_pkg_id, result.submission_kind);
    let failure_evidence_kind =
        prepare_verifier_evidence_kind(tx, objects.interface_pkg_id, result.failure_evidence_kind);
    let payload_or_reason_hash = tx.input(pure_arg(&result.payload_or_reason_hash)?);
    let credential = tx.input(pure_arg(&result.credential)?);
    let detail = tx.input(pure_arg(&result.detail)?);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_VERIFIER_CONTRACT_RESULT_V1,
            vec![],
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
    evidence: &crate::types::ExternalVerifierSubmitEvidenceV1,
) -> anyhow::Result<sui::types::Argument> {
    let result = prepare_verifier_contract_result(tx, objects, &evidence.result)?;
    let communication_evidence = tx.input(pure_arg(&evidence.communication_evidence)?);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE_V1,
            vec![],
        ),
        vec![result, communication_evidence],
    ))
}

fn prepare_offchain_verifier_proof(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    proof: &OffChainVerifierProofV1,
) -> anyhow::Result<sui::types::Argument> {
    match proof {
        OffChainVerifierProofV1::RegisteredKey {
            verifier_credential,
            communication_evidence,
        } => {
            let verifier_credential = tx.input(pure_arg(verifier_credential)?);
            let communication_evidence = tx.input(pure_arg(communication_evidence)?);
            Ok(tx.move_call(
                sui::tx::Function::new(
                    objects.interface_pkg_id,
                    VERIFIER_V1_MODULE,
                    NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY_V1,
                    vec![],
                ),
                vec![verifier_credential, communication_evidence],
            ))
        }
        OffChainVerifierProofV1::ExternalVerifier { evidence } => {
            let evidence = prepare_external_verifier_submit_evidence(tx, objects, evidence)?;
            Ok(tx.move_call(
                sui::tx::Function::new(
                    objects.interface_pkg_id,
                    VERIFIER_V1_MODULE,
                    NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER_V1,
                    vec![],
                ),
                vec![evidence],
            ))
        }
    }
}

fn prepare_authenticated_offchain_request_evidence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    expected_vertex: sui::types::Argument,
    request: &AuthenticatedOffchainRequestEvidenceV1,
) -> anyhow::Result<sui::types::Argument> {
    let walk_index = tx.input(pure_arg(&request.walk_index)?);
    let tool_fqn = move_std::Ascii::ascii_string_from_str(tx, &request.tool_fqn)?;
    let request_hash = tx.input(pure_arg(&request.request_hash)?);
    let request_signature = tx.input(pure_arg(&request.request_signature)?);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1.module,
            workflow::Dag::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1.name,
            vec![],
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
    request: &OffchainRequestEvidenceV1,
) -> anyhow::Result<sui::types::Argument> {
    let execution = sui_framework::Object::id_from_object_id(tx, request.execution)?;
    let walk_index = tx.input(pure_arg(&request.walk_index)?);
    let vertex = move_std::Ascii::ascii_string_from_str(tx, &request.vertex)?;
    let tool_fqn = move_std::Ascii::ascii_string_from_str(tx, &request.tool_fqn)?;
    let leader_cap_id = sui_framework::Object::id_from_object_id(tx, request.leader_cap_id)?;
    let request_hash = tx.input(pure_arg(&request.request_hash)?);
    let request_signature = tx.input(pure_arg(&request.request_signature)?);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_OFFCHAIN_REQUEST_EVIDENCE_V1,
            vec![],
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
    response: &OffchainResponseEvidenceV1,
) -> anyhow::Result<sui::types::Argument> {
    let status_code = tx.input(pure_arg(&response.status_code)?);
    let response_hash = tx.input(pure_arg(&response.response_hash)?);
    let response_signature = tx.input(pure_arg(&response.response_signature)?);
    let normalized_err_eval_reason_hash =
        prepare_move_option_vec_u8(tx, &response.normalized_err_eval_reason_hash);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_OFFCHAIN_RESPONSE_EVIDENCE_V1,
            vec![],
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
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    expected_vertex: sui::types::Argument,
    evidence: &AuthenticatedOffchainVerifierEvidenceV1,
) -> anyhow::Result<sui::types::Argument> {
    let submission_kind =
        prepare_submission_kind(tx, objects.interface_pkg_id, evidence.submission_kind);
    let payload_or_reason_hash = tx.input(pure_arg(&evidence.payload_or_reason_hash)?);
    let transport_proof = tx.input(pure_arg(&evidence.transport_proof)?);
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
            VERIFIER_V1_MODULE,
            NEW_OFFCHAIN_VERIFIER_EVIDENCE_V1,
            vec![],
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
    evidence: &OffchainVerifierEvidenceV1,
) -> anyhow::Result<sui::types::Argument> {
    let submission_kind =
        prepare_submission_kind(tx, objects.interface_pkg_id, evidence.submission_kind);
    let payload_or_reason_hash = tx.input(pure_arg(&evidence.payload_or_reason_hash)?);
    let transport_proof = tx.input(pure_arg(&evidence.transport_proof)?);
    let request =
        prepare_raw_offchain_request_evidence_for_preflight(tx, objects, &evidence.request)?;
    let response = prepare_offchain_response_evidence(tx, objects, &evidence.response)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_OFFCHAIN_VERIFIER_EVIDENCE_V1,
            vec![],
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

pub fn call_external_verifier_v1_with_authenticated_request(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    expected_vertex: sui::types::Argument,
    verifier_evidence: &AuthenticatedOffchainVerifierEvidenceV1,
    runtime_call: &ExternalVerifierRuntimeCallV1,
) -> anyhow::Result<sui::types::Argument> {
    let witness = tx.input(sui::tx::Input::shared(
        *runtime_call.witness.object_id(),
        runtime_call.witness.version(),
        true,
    ));
    let shared_objects = runtime_call
        .shared_objects
        .iter()
        .map(|(shared, object_ref)| {
            tx.input(sui::tx::Input::shared(
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

    Ok(tx.move_call(
        sui::tx::Function::new(runtime_call.package_address, module, function, vec![]),
        {
            let mut args = Vec::with_capacity(shared_objects.len() + 2);
            args.push(witness);
            args.extend(shared_objects);
            args.push(verifier_evidence);
            args
        },
    ))
}

/// Preflight-only verifier contract call.
///
/// Active submit builders create request evidence through the workflow package
/// from authenticated `DAGExecution` and `leader_cap` arguments instead.
pub fn call_external_verifier_v1(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    verifier_evidence: &OffchainVerifierEvidenceV1,
    runtime_call: &ExternalVerifierRuntimeCallV1,
) -> anyhow::Result<sui::types::Argument> {
    let witness = tx.input(sui::tx::Input::shared(
        *runtime_call.witness.object_id(),
        runtime_call.witness.version(),
        true,
    ));
    let shared_objects = runtime_call
        .shared_objects
        .iter()
        .map(|(shared, object_ref)| {
            tx.input(sui::tx::Input::shared(
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

    Ok(tx.move_call(
        sui::tx::Function::new(runtime_call.package_address, module, function, vec![]),
        {
            let mut args = Vec::with_capacity(shared_objects.len() + 2);
            args.push(witness);
            args.extend(shared_objects);
            args.push(verifier_evidence);
            args
        },
    ))
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
    proof: &OffChainVerifierProofV1,
    verifier_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
    network_auth: sui::types::Argument,
    leader_key_binding: sui::types::Argument,
    tool_key_binding: sui::types::Argument,
    gas_charge: u64,
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
    let proof = prepare_offchain_verifier_proof(tx, objects, proof)?;
    let priority_fee_vault = priority_fee_vault_arg(tx, objects)?;
    let gas_charge = tx.input(pure_arg(&gas_charge)?);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name,
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
            result_bytes,
            auxiliary_bytes,
            proof,
            verifier_registry,
            leader_registry,
            priority_fee_vault,
            network_auth,
            leader_key_binding,
            tool_key_binding,
            gas_charge,
            clock,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_off_chain_tool_result_for_walk_without_verifier_v1(
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
    leader_registry: sui::types::Argument,
    gas_charge: u64,
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
    let priority_fee_vault = priority_fee_vault_arg(tx, objects)?;
    let gas_charge = tx.input(pure_arg(&gas_charge)?);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1.name,
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
            result_bytes,
            auxiliary_bytes,
            leader_registry,
            priority_fee_vault,
            gas_charge,
            clock,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_off_chain_tool_result_for_walk_with_external_verifier_proof_v1(
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
    verifier_evidence: &AuthenticatedOffchainVerifierEvidenceV1,
    communication_evidence: &[u8],
    runtime_call: &ExternalVerifierRuntimeCallV1,
    verifier_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
    network_auth: sui::types::Argument,
    leader_key_binding: sui::types::Argument,
    tool_key_binding: sui::types::Argument,
    gas_charge: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let expected_vertex_arg =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let verifier_result = call_external_verifier_v1_with_authenticated_request(
        tx,
        objects,
        execution,
        leader_cap,
        expected_vertex_arg,
        verifier_evidence,
        runtime_call,
    )?;
    let communication_evidence = tx.input(pure_arg(&communication_evidence.to_vec())?);
    let external_verifier_evidence = tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE_V1,
            vec![],
        ),
        vec![verifier_result, communication_evidence],
    );
    let proof = tx.move_call(
        sui::tx::Function::new(
            objects.interface_pkg_id,
            VERIFIER_V1_MODULE,
            NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER_V1,
            vec![],
        ),
        vec![external_verifier_evidence],
    );
    let auxiliary = tx.input(pure_arg(&Option::<Vec<u8>>::None)?);
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, objects, result)?;
    let priority_fee_vault = priority_fee_vault_arg(tx, objects)?;
    let gas_charge = tx.input(pure_arg(&gas_charge)?);

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name,
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
            result_bytes,
            auxiliary,
            proof,
            verifier_registry,
            leader_registry,
            priority_fee_vault,
            network_auth,
            leader_key_binding,
            tool_key_binding,
            gas_charge,
            clock,
        ],
    );

    Ok(())
}

pub fn leader_stamp_tap_worksheet(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    leader_registry: sui::types::Argument,
    execution: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::LEADER_STAMP_TAP_WORKSHEET.module,
            workflow::Dag::LEADER_STAMP_TAP_WORKSHEET.name,
            vec![],
        ),
        vec![leader_registry, execution, worksheet, leader_cap],
    )
}

pub fn pre_stamp_tap_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    vertex: &RuntimeVertex,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, vertex)?;
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::PRE_STAMP_TAP_EXECUTION.module,
            workflow::Dag::PRE_STAMP_TAP_EXECUTION.name,
            vec![],
        ),
        vec![execution, worksheet, leader_cap, vertex],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn create_vertex_authorization_grant(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    vertex: &RuntimeVertex,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);
    let vertex = workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, vertex)?;
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::CREATE_VERTEX_AUTHORIZATION_GRANT.module,
            workflow::Dag::CREATE_VERTEX_AUTHORIZATION_GRANT.name,
            vec![],
        ),
        vec![dag, execution, tool_registry, agent, skill_id, vertex],
    ))
}

pub fn request_walk_execution_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
    walk_index: u64,
) -> anyhow::Result<sui::types::Argument> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::REQUEST_WALK_EXECUTION_FOR_WALK.module,
            workflow::Dag::REQUEST_WALK_EXECUTION_FOR_WALK.name,
            vec![],
        ),
        vec![execution, walk_index],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn mint_vertex_authorization_check_cap_for_onchain_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
) -> anyhow::Result<sui::types::Argument> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::MINT_VERTEX_AUTHORIZATION_CHECK_CAP_FOR_ONCHAIN_WALK.module,
            workflow::Dag::MINT_VERTEX_AUTHORIZATION_CHECK_CAP_FOR_ONCHAIN_WALK.name,
            vec![],
        ),
        vec![dag, execution, leader_cap, walk_index],
    ))
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
    prepared_output: &PreparedToolOutput,
    failure_evidence_kind: Option<&FailureEvidenceKind>,
    submitted_failure_reason: Option<Vec<u8>>,
    tool_witness_id: sui::types::Address,
    leader_registry: sui::types::Argument,
    gas_charge: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let expected_vertex =
        workflow::Dag::runtime_vertex_from_enum(tx, objects.workflow_pkg_id, expected_vertex)?;
    let (output_variant, output_ports_data) = prepare_tool_output(tx, objects, prepared_output)?;
    let failure_evidence_kind =
        prepare_move_option_failure_evidence_kind(tx, objects, failure_evidence_kind);
    let submitted_failure_reason = prepare_move_option_vec_u8(tx, &submitted_failure_reason);
    let tool_witness_id = sui_framework::Object::id_from_object_id(tx, tool_witness_id)?;
    let priority_fee_vault = priority_fee_vault_arg(tx, objects)?;
    let gas_charge = tx.input(pure_arg(&gas_charge)?);

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
            output_variant,
            output_ports_data,
            failure_evidence_kind,
            submitted_failure_reason,
            tool_witness_id,
            leader_registry,
            priority_fee_vault,
            gas_charge,
            clock,
        ],
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_on_chain_tool_result_for_walk_v1_with_args(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    request_walk_execution: sui::types::Argument,
    walk_index: u64,
    expected_vertex: sui::types::Argument,
    output_variant: sui::types::Argument,
    output_ports_data: sui::types::Argument,
    failure_evidence_kind: Option<&FailureEvidenceKind>,
    submitted_failure_reason: Option<Vec<u8>>,
    tool_witness_id: sui::types::Address,
    leader_registry: sui::types::Argument,
    gas_charge: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.input(pure_arg(&walk_index)?);
    let failure_evidence_kind =
        prepare_move_option_failure_evidence_kind(tx, objects, failure_evidence_kind);
    let submitted_failure_reason = prepare_move_option_vec_u8(tx, &submitted_failure_reason);
    let tool_witness_id = sui_framework::Object::id_from_object_id(tx, tool_witness_id)?;
    let priority_fee_vault = priority_fee_vault_arg(tx, objects)?;
    let gas_charge = tx.input(pure_arg(&gas_charge)?);

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
            output_variant,
            output_ports_data,
            failure_evidence_kind,
            submitted_failure_reason,
            tool_witness_id,
            leader_registry,
            priority_fee_vault,
            gas_charge,
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
    leader_registry: sui::types::Argument,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    submit_on_chain_tool_result_for_walk_v1(
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
        Some(failure_evidence_kind),
        None,
        tool_witness_id.unwrap_or(sui::types::Address::ZERO),
        leader_registry,
        0,
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

#[allow(clippy::too_many_arguments)]
pub fn prepare_agent_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_registry: sui::types::Argument,
    agent_registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag: sui::types::Argument,
    dag_id: sui::types::Argument,
    priority_fee_excess_quote: Option<u64>,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    agent_execution: &AgentDagExecuteInput,
    payment_coin: sui::types::Argument,
    clock: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
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
        let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex_name)?;
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
            let port = workflow::Dag::input_port_from_str(
                tx,
                objects.workflow_pkg_id,
                port_name.as_str(),
            )?;

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

    let priority_fee_excess_quote = prepare_option_u64(tx, priority_fee_excess_quote)?;
    let agent_id = tap::agent_id_from_address(tx, objects, agent_execution.agent_id)?;
    let skill_id = tx.input(pure_arg(&agent_execution.skill_id)?);
    let authorization_plan_commitment =
        tx.input(pure_arg(&agent_execution.authorization_plan_commitment)?);
    let authorization_plan =
        tap_authorization_plan(tx, objects, &agent_execution.authorization_plan)?;

    let config = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::NEW_AGENT_EXECUTION_CONFIG.module,
            workflow::Dag::NEW_AGENT_EXECUTION_CONFIG.name,
            vec![],
        ),
        vec![
            dag_id,
            network,
            entry_group,
            with_vertex_inputs,
            priority_fee_excess_quote,
            agent_id,
            skill_id,
            authorization_plan_commitment,
            authorization_plan,
        ],
    );

    let payment_source = tx.input(pure_arg(&agent_execution.payment_source)?);
    let payment_max_budget = tx.input(pure_arg(&agent_execution.payment_max_budget)?);
    let payment_refund_mode = tx.input(pure_arg(&agent_execution.payment_refund_mode)?);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.module,
            workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.name,
            vec![],
        ),
        vec![
            dag,
            agent_registry,
            agent,
            tool_registry,
            config,
            payment_coin,
            payment_source,
            payment_max_budget,
            payment_refund_mode,
            clock,
        ],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_default_agent_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_registry: sui::types::Argument,
    agent_registry: sui::types::Argument,
    dag: sui::types::Argument,
    dag_id: sui::types::Argument,
    priority_fee_excess_quote: Option<u64>,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    agent_execution: &AgentDagExecuteInput,
    payment_coin: sui::types::Argument,
    clock: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
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
        let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex_name)?;
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
            let port = workflow::Dag::input_port_from_str(
                tx,
                objects.workflow_pkg_id,
                port_name.as_str(),
            )?;

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

    let priority_fee_excess_quote = prepare_option_u64(tx, priority_fee_excess_quote)?;
    let config = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::NEW_DAG_EXECUTION_CONFIG.module,
            workflow::Dag::NEW_DAG_EXECUTION_CONFIG.name,
            vec![],
        ),
        vec![
            dag_id,
            network,
            entry_group,
            with_vertex_inputs,
            priority_fee_excess_quote,
        ],
    );

    let payment_source = tx.input(pure_arg(&agent_execution.payment_source)?);
    let payment_max_budget = tx.input(pure_arg(&agent_execution.payment_max_budget)?);
    let payment_refund_mode = tx.input(pure_arg(&agent_execution.payment_refund_mode)?);
    let authorization_plan_commitment =
        tx.input(pure_arg(&agent_execution.authorization_plan_commitment)?);
    let authorization_plan =
        tap_authorization_plan(tx, objects, &agent_execution.authorization_plan)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::BEGIN_DAG_EXECUTION_WITH_CONFIG.module,
            workflow::Dag::BEGIN_DAG_EXECUTION_WITH_CONFIG.name,
            vec![],
        ),
        vec![
            dag,
            agent_registry,
            tool_registry,
            config,
            payment_coin,
            payment_source,
            payment_max_budget,
            payment_refund_mode,
            authorization_plan_commitment,
            authorization_plan,
            clock,
        ],
    ))
}

/// PTB template to lock execution payment state for the given tools.
#[allow(clippy::too_many_arguments)]
pub fn lock_payment_state_for_tools(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tools_gas: Vec<sui::types::Argument>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    ticket: sui::types::Argument,
) {
    for tool_gas in tools_gas {
        // `nexus_workflow::gas::lock_payment_state_for_tool()`
        tx.move_call(
            sui::tx::Function::new(
                objects.workflow_pkg_id,
                workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.module,
                workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.name,
                vec![],
            ),
            vec![tool_gas, dag, execution, ticket],
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_agent_dag(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    agent: &sui::types::ObjectReference,
    priority_fee_excess_quote: Option<u64>,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    execute_agent_dag_internal(
        tx,
        objects,
        dag,
        Some(agent),
        priority_fee_excess_quote,
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
    priority_fee_excess_quote: Option<u64>,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    execute_agent_dag_internal(
        tx,
        objects,
        dag,
        None,
        priority_fee_excess_quote,
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
    agent: Option<&sui::types::ObjectReference>,
    priority_fee_excess_quote: Option<u64>,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
    default_executor: bool,
) -> anyhow::Result<()> {
    let payment_total_budget =
        agent_execution
            .payment_total_budget
            .unwrap_or(required_total_budget(
                agent_execution.payment_max_budget,
                priority_fee_excess_quote,
            )?);
    let dag_id = sui_framework::Object::id_from_object_id(tx, *dag.object_id())?;
    let dag = tx.input(sui::tx::Input::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));

    let agent = agent.map(|agent| {
        tx.input(sui::tx::Input::shared(
            *agent.object_id(),
            agent.version(),
            true,
        ))
    });

    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    let agent_registry = tx.input(sui::tx::Input::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        false,
    ));

    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    let payment_coin = if let Some(payment_coin_ref) = agent_execution.payment_coin.as_ref() {
        let owned_payment_coin = tx.input(sui::tx::Input::owned(
            *payment_coin_ref.object_id(),
            payment_coin_ref.version(),
            *payment_coin_ref.digest(),
        ));
        match agent_execution.payment_coin_balance {
            Some(balance) if balance > payment_total_budget => {
                let payment_amount = tx.input(pure_arg(&payment_total_budget)?);
                tx.split_coins(owned_payment_coin, vec![payment_amount])
                    .nested(0)
                    .ok_or_else(|| anyhow::anyhow!("failed to split TAP execution payment coin"))?
            }
            _ => owned_payment_coin,
        }
    } else {
        let payment_amount = tx.input(pure_arg(&payment_total_budget)?);
        tx.split_coins(tx.gas(), vec![payment_amount])
            .nested(0)
            .ok_or_else(|| anyhow::anyhow!("failed to split TAP execution payment coin"))?
    };

    let results = if default_executor {
        prepare_default_agent_execution(
            tx,
            objects,
            tool_registry,
            agent_registry,
            dag,
            dag_id,
            priority_fee_excess_quote,
            entry_group,
            input_data,
            agent_execution,
            payment_coin,
            clock,
        )?
    } else {
        let agent =
            agent.ok_or_else(|| anyhow::anyhow!("agent DAG execution requires an Agent input"))?;
        prepare_agent_execution(
            tx,
            objects,
            tool_registry,
            agent_registry,
            agent,
            dag,
            dag_id,
            priority_fee_excess_quote,
            entry_group,
            input_data,
            agent_execution,
            payment_coin,
            clock,
        )?
    };

    let Some(execution) = results.nested(0) else {
        return Err(anyhow::anyhow!(
            "Failed to receive agent DAG execution argument"
        ));
    };
    let Some(ticket) = results.nested(1) else {
        return Err(anyhow::anyhow!(
            "Failed to receive agent DAG ticket argument"
        ));
    };
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        false,
    ));
    let tools_gas = tools_gas
        .iter()
        .map(|(address, version)| tx.input(sui::tx::Input::shared(*address, *version, true)))
        .collect();

    crate::transactions::gas::snapshot_dag_tool_costs(tx, objects, gas_service, execution, dag);
    lock_payment_state_for_tools(tx, objects, tools_gas, dag, execution, ticket);

    let leader_registry = tx.input(sui::tx::Input::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::REQUEST_NETWORK_TO_EXECUTE_WALKS.module,
            workflow::Dag::REQUEST_NETWORK_TO_EXECUTE_WALKS.name,
            vec![],
        ),
        vec![dag, execution, ticket, leader_registry, clock],
    );

    let execution_type =
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::DAG_EXECUTION);
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
            vec![execution_type],
        ),
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
                InterfaceRevision,
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
            let sui::types::Input::Pure { value } = self.input(argument) else {
                panic!("expected pure input, got {:?}", self.input(argument));
            };

            let actual: sui::types::Address =
                bcs::from_bytes(value).expect("address BCS should deserialize");
            assert_eq!(actual, expected);
        }

        fn expect_u64(&self, argument: &sui::types::Argument, expected: u64) {
            let sui::types::Input::Pure { value } = self.input(argument) else {
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
            let sui::types::Input::Shared {
                object_id,
                initial_shared_version,
                mutable,
            } = self.input(argument)
            else {
                panic!("expected shared input, got {:?}", self.input(argument));
            };
            assert_eq!(object_id, expected.object_id());
            assert_eq!(*initial_shared_version, expected.version());
            assert_eq!(*mutable, expected_mutable);
        }

        fn expect_string(&self, argument: &sui::types::Argument, expected: &str) {
            let sui::types::Input::Pure { value } = self.input(argument) else {
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

    fn mock_authenticated_offchain_verifier_evidence(
    ) -> crate::types::AuthenticatedOffchainVerifierEvidenceV1 {
        crate::types::AuthenticatedOffchainVerifierEvidenceV1 {
            submission_kind: crate::types::VerificationSubmissionKind::Success,
            payload_or_reason_hash: vec![1, 2, 3],
            transport_proof: vec![4, 5, 6],
            request: crate::types::AuthenticatedOffchainRequestEvidenceV1 {
                walk_index: 9,
                tool_fqn: "xyz.test.tool@1".to_string(),
                request_hash: vec![7, 8],
                request_signature: vec![9, 10],
            },
            response: crate::types::OffchainResponseEvidenceV1 {
                status_code: 200,
                response_hash: vec![11, 12],
                response_signature: vec![13, 14],
                normalized_err_eval_reason_hash: None,
            },
        }
    }

    fn mock_raw_offchain_verifier_evidence() -> crate::types::OffchainVerifierEvidenceV1 {
        crate::types::OffchainVerifierEvidenceV1 {
            submission_kind: crate::types::VerificationSubmissionKind::Success,
            payload_or_reason_hash: vec![1, 2, 3],
            transport_proof: vec![4, 5, 6],
            request: crate::types::OffchainRequestEvidenceV1 {
                execution: sui_mocks::mock_sui_address(),
                walk_index: 9,
                vertex: "vertex1".to_string(),
                tool_fqn: "xyz.test.tool@1".to_string(),
                leader_cap_id: sui_mocks::mock_sui_address(),
                request_hash: vec![7, 8],
                request_signature: vec![9, 10],
            },
            response: crate::types::OffchainResponseEvidenceV1 {
                status_code: 200,
                response_hash: vec![11, 12],
                response_signature: vec![13, 14],
                normalized_err_eval_reason_hash: None,
            },
        }
    }

    fn mock_external_verifier_runtime_call() -> crate::types::ExternalVerifierRuntimeCallV1 {
        let witness = sui_mocks::mock_sui_object_ref();
        let shared = sui_mocks::mock_sui_object_ref();
        crate::types::ExternalVerifierRuntimeCallV1 {
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
        crate::types::OffChainToolResultAuxiliaryV1::err_eval(
            crate::types::FailureEvidenceKind::LeaderEvidence,
        )
    }

    fn mock_objects_with_verifier_registry() -> NexusObjects {
        let mut objects = sui_mocks::mock_nexus_objects();
        objects.verifier_registry = sui_mocks::mock_sui_object_ref();
        objects
    }

    fn test_leader_registry_arg(
        tx: &mut sui::tx::TransactionBuilder,
        objects: &NexusObjects,
    ) -> sui::types::Argument {
        tx.input(sui::tx::Input::shared(
            *objects.leader_registry.object_id(),
            objects.leader_registry.version(),
            false,
        ))
    }

    #[test]
    fn test_prepared_tool_output_terminal_err_eval_shape() {
        let reason = DataStorage::try_from(serde_json::json!({
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
        assert_eq!(reason.as_json(), &expected_reason);
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

    fn mock_offchain_registered_key_proof() -> crate::types::OffChainVerifierProofV1 {
        crate::types::OffChainVerifierProofV1::RegisteredKey {
            verifier_credential: vec![1, 2, 3],
            communication_evidence: vec![4, 5, 6],
        }
    }

    fn mock_offchain_none_auxiliary() -> crate::types::OffChainToolResultAuxiliaryV1 {
        crate::types::OffChainToolResultAuxiliaryV1::success()
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_v1_with_registered_key_proof() {
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
            Some(&mock_offchain_none_auxiliary()),
            &mock_offchain_registered_key_proof(),
            sui::types::Argument::Result(6),
            sui::types::Argument::Result(7),
            sui::types::Argument::Result(8),
            sui::types::Argument::Result(9),
            sui::types::Argument::Result(10),
            0,
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
        assert_eq!(call.arguments.len(), 19);
        inspector.expect_shared_object(&call.arguments[13], &objects.priority_fee_vault, true);
        assert!(matches!(
            call.arguments[10],
            sui::types::Argument::Result(_)
        ));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == VERIFIER_V1_MODULE
                        && call.function == NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY_V1
            )
        }));
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_without_verifier_v1_routes_plain_submit() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

        submit_off_chain_tool_result_for_walk_without_verifier_v1(
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
            leader_registry,
            0,
            sui::types::Argument::Result(7),
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
        assert_eq!(call.arguments.len(), 14);
        inspector.expect_shared_object(&call.arguments[10], &objects.leader_registry, false);
        inspector.expect_shared_object(&call.arguments[11], &objects.priority_fee_vault, true);
    }

    #[test]
    fn test_submit_off_chain_tool_result_for_walk_without_verifier_v1_routes_failure_submit() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

        submit_off_chain_tool_result_for_walk_without_verifier_v1(
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
            leader_registry,
            0,
            sui::types::Argument::Result(7),
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
        assert_eq!(call.arguments.len(), 14);
        assert!(matches!(call.arguments[8], sui::types::Argument::Result(_)));
        assert!(matches!(call.arguments[9], sui::types::Argument::Input(_)));
        inspector.expect_shared_object(&call.arguments[10], &objects.leader_registry, false);
        inspector.expect_shared_object(&call.arguments[11], &objects.priority_fee_vault, true);
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
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

        submit_off_chain_tool_result_for_walk_without_verifier_v1(
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
            leader_registry,
            0,
            sui::types::Argument::Result(7),
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
    fn test_submit_off_chain_tool_result_for_walk_v1_with_external_verifier_proof_builds_typed_ptb()
    {
        let objects = sui_mocks::mock_nexus_objects();
        let runtime_call = mock_external_verifier_runtime_call();
        let mut tx = sui::tx::TransactionBuilder::new();

        submit_off_chain_tool_result_for_walk_with_external_verifier_proof_v1(
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
            &mock_authenticated_offchain_verifier_evidence(),
            &[21, 22, 23],
            &runtime_call,
            sui::types::Argument::Result(6),
            sui::types::Argument::Result(7),
            sui::types::Argument::Result(8),
            sui::types::Argument::Result(9),
            sui::types::Argument::Result(10),
            0,
            sui::types::Argument::Result(11),
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
        assert_eq!(verifier_call.arguments.len(), 3);
        assert!(matches!(
            verifier_call.arguments[0],
            sui::types::Argument::Input(_)
        ));
        assert!(matches!(
            verifier_call.arguments[1],
            sui::types::Argument::Input(_)
        ));
        assert!(matches!(
            verifier_call.arguments[2],
            sui::types::Argument::Result(_)
        ));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.workflow_pkg_id
                        && call.module
                            == workflow::Dag::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1.module
                        && call.function
                            == workflow::Dag::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1.name
            )
        }));
        assert!(!inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == VERIFIER_V1_MODULE
                        && call.function == NEW_OFFCHAIN_REQUEST_EVIDENCE_V1
            )
        }));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == VERIFIER_V1_MODULE
                        && call.function == NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE_V1
            )
        }));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == VERIFIER_V1_MODULE
                        && call.function == NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER_V1
            )
        }));

        let submit_call = inspector.move_call(inspector.commands().len() - 1);
        assert_eq!(submit_call.package, objects.workflow_pkg_id);
        assert_eq!(
            submit_call.module,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            submit_call.function,
            workflow::Dag::SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(submit_call.arguments.len(), 19);
        inspector.expect_shared_object(
            &submit_call.arguments[13],
            &objects.priority_fee_vault,
            true,
        );
        assert!(matches!(
            submit_call.arguments[10],
            sui::types::Argument::Result(_)
        ));
    }

    #[test]
    fn test_call_external_verifier_v1_keeps_raw_request_evidence_for_preflight() {
        let objects = sui_mocks::mock_nexus_objects();
        let runtime_call = mock_external_verifier_runtime_call();
        let mut tx = sui::tx::TransactionBuilder::new();

        call_external_verifier_v1(
            &mut tx,
            &objects,
            &mock_raw_offchain_verifier_evidence(),
            &runtime_call,
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert!(inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.interface_pkg_id
                        && call.module == VERIFIER_V1_MODULE
                        && call.function == NEW_OFFCHAIN_REQUEST_EVIDENCE_V1
            )
        }));
        assert!(!inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.package == objects.workflow_pkg_id
                        && call.module
                            == workflow::Dag::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1.module
                        && call.function
                            == workflow::Dag::NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1.name
            )
        }));
    }

    #[test]
    fn test_submit_on_chain_tool_result_for_walk_v1() {
        let objects = sui_mocks::mock_nexus_objects();
        let witness = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

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
            &mock_prepared_tool_output(),
            None,
            None,
            witness,
            leader_registry,
            0,
            sui::types::Argument::Result(7),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_on_chain_submit_call_uses_sdk_move_values(
            &inspector,
            call,
            &objects,
            objects.workflow_pkg_id,
            witness,
        );
    }

    #[test]
    fn test_submit_on_chain_tool_result_for_walk_v1_with_args_constructs_move_values() {
        let objects = sui_mocks::mock_nexus_objects();
        let witness = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

        submit_on_chain_tool_result_for_walk_v1_with_args(
            &mut tx,
            &objects,
            sui::types::Argument::Result(0),
            sui::types::Argument::Result(1),
            sui::types::Argument::Result(2),
            sui::types::Argument::Result(3),
            sui::types::Argument::Result(4),
            sui::types::Argument::Result(5),
            9,
            sui::types::Argument::Result(6),
            sui::types::Argument::Result(7),
            sui::types::Argument::Result(8),
            None,
            None,
            witness,
            leader_registry,
            0,
            sui::types::Argument::Result(10),
        )
        .unwrap();

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_on_chain_submit_call_uses_sdk_move_values(
            &inspector,
            call,
            &objects,
            objects.workflow_pkg_id,
            witness,
        );
    }

    #[test]
    fn test_mint_vertex_authorization_check_cap_for_onchain_walk_uses_active_walk_args() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = sui::types::Argument::Result(0);
        let execution = sui::types::Argument::Result(1);
        let leader_cap = sui::types::Argument::Result(2);

        mint_vertex_authorization_check_cap_for_onchain_walk(
            &mut tx, &objects, dag, execution, leader_cap, 17,
        )
        .expect("mint helper should build");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(inspector.commands().len() - 1);

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::MINT_VERTEX_AUTHORIZATION_CHECK_CAP_FOR_ONCHAIN_WALK.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::MINT_VERTEX_AUTHORIZATION_CHECK_CAP_FOR_ONCHAIN_WALK.name
        );
        assert_eq!(call.arguments.len(), 4);
        assert_eq!(call.arguments[0], dag);
        assert_eq!(call.arguments[1], execution);
        assert_eq!(call.arguments[2], leader_cap);
        inspector.expect_u64(&call.arguments[3], 17);
        assert_eq!(inspector.inputs().len(), 1);
    }

    fn assert_on_chain_submit_call_uses_sdk_move_values(
        inspector: &TxInspector,
        call: &sui::types::MoveCall,
        objects: &NexusObjects,
        workflow_pkg_id: sui::types::Address,
        expected_tool_witness_id: sui::types::Address,
    ) {
        assert_eq!(call.package, workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.module
        );
        assert_eq!(
            call.function,
            workflow::Dag::SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1.name
        );
        assert_eq!(call.arguments.len(), 17);
        assert!(matches!(
            call.arguments[10],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        assert!(matches!(
            call.arguments[12],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(call.arguments[15], sui::types::Argument::Input(_)));
        inspector.expect_shared_object(&call.arguments[13], &objects.leader_registry, false);
        inspector.expect_shared_object(&call.arguments[14], &objects.priority_fee_vault, true);

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
                workflow_pkg_id,
                workflow::Dag::FAILURE_EVIDENCE_KIND
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
    fn test_prepare_on_chain_tool_result_submission_v1_bytes_constructs_move_values() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_witness_id = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();

        let submission = prepare_on_chain_tool_result_submission_v1_bytes(
            &mut tx,
            &objects,
            sui::types::Argument::Result(0),
            sui::types::Argument::Result(1),
            None,
            &None,
            tool_witness_id,
        )
        .expect("submission bytes helper should build");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let submission_call = match submission {
            sui::types::Argument::Result(index) => inspector.move_call(index as usize),
            other => panic!("expected submission to be a command result, got {other:?}"),
        };

        assert_eq!(submission_call.package, objects.workflow_pkg_id);
        assert_eq!(
            submission_call.function,
            workflow::Dag::ON_CHAIN_TOOL_RESULT_SUBMISSION_V1_BYTES.name
        );

        let failure_option = match submission_call.arguments[2] {
            sui::types::Argument::Result(index) => inspector.move_call(index as usize),
            other => panic!("expected failure option to be a command result, got {other:?}"),
        };
        assert_eq!(failure_option.package, move_std::PACKAGE_ID);
        assert_eq!(failure_option.module, move_std::Option::NONE.module);
        assert_eq!(failure_option.function, move_std::Option::NONE.name);
        assert_eq!(
            failure_option.type_arguments,
            vec![workflow::into_type_tag(
                objects.workflow_pkg_id,
                workflow::Dag::FAILURE_EVIDENCE_KIND
            )]
        );

        let tool_witness_id_call = match submission_call.arguments[4] {
            sui::types::Argument::Result(index) => inspector.move_call(index as usize),
            other => panic!("expected tool witness ID to be a command result, got {other:?}"),
        };
        assert_eq!(tool_witness_id_call.package, sui_framework::PACKAGE_ID);
        assert_eq!(
            tool_witness_id_call.function,
            sui_framework::Object::ID_FROM_ADDRESS.name
        );
        inspector.expect_address(&tool_witness_id_call.arguments[0], tool_witness_id);
    }

    #[test]
    fn test_submit_on_chain_tool_result_for_walk_v1_with_failure_evidence() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_witness_id = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

        submit_on_chain_tool_result_for_walk_v1(
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
            Some(&FailureEvidenceKind::ToolEvidence),
            Some(b"tool failed".to_vec()),
            tool_witness_id,
            leader_registry,
            0,
            sui::types::Argument::Result(7),
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
        assert_eq!(call.arguments.len(), 17);
        assert!(matches!(
            call.arguments[10],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        assert!(matches!(
            call.arguments[12],
            sui::types::Argument::Result(_)
        ));
        inspector.expect_shared_object(&call.arguments[13], &objects.leader_registry, false);
        inspector.expect_shared_object(&call.arguments[14], &objects.priority_fee_vault, true);
        assert!(matches!(call.arguments[15], sui::types::Argument::Input(_)));
    }

    #[test]
    fn test_submit_on_chain_terminal_err_eval_for_walk_defaults_zero_tool_witness_id() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

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
            leader_registry,
            sui::types::Argument::Result(7),
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
        assert_eq!(call.arguments.len(), 17);
        assert!(matches!(
            call.arguments[10],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        assert!(matches!(
            call.arguments[12],
            sui::types::Argument::Result(_)
        ));
        inspector.expect_shared_object(&call.arguments[13], &objects.leader_registry, false);
        inspector.expect_shared_object(&call.arguments[14], &objects.priority_fee_vault, true);
        assert!(matches!(call.arguments[15], sui::types::Argument::Input(_)));
    }

    #[test]
    fn test_submit_on_chain_terminal_err_eval_for_walk_builds_terminal_output_with_tool_witness_id()
    {
        let objects = sui_mocks::mock_nexus_objects();
        let tool_witness_id = sui_mocks::mock_sui_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let leader_registry = test_leader_registry_arg(&mut tx, &objects);

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
            Some(tool_witness_id),
            leader_registry,
            sui::types::Argument::Result(7),
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
        assert_eq!(call.arguments.len(), 17);
        assert!(matches!(call.arguments[11], sui::types::Argument::Input(_)));
        let sui::types::Argument::Result(tool_witness_id_index) = &call.arguments[12] else {
            panic!("expected tool witness ID result argument");
        };
        inspector.expect_shared_object(&call.arguments[13], &objects.leader_registry, false);
        inspector.expect_shared_object(&call.arguments[14], &objects.priority_fee_vault, true);
        assert!(matches!(call.arguments[15], sui::types::Argument::Input(_)));
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
        assert_eq!(
            output_variant_call.module,
            workflow::Dag::OUTPUT_VARIANT_FROM_STRING.module
        );
        assert_eq!(
            output_variant_call.function,
            workflow::Dag::OUTPUT_VARIANT_FROM_STRING.name
        );
        inspector.expect_ascii_string_result(&output_variant_call.arguments[0], "_err_eval");

        let output_port_indices = inspector.move_call_indices_to(
            objects.workflow_pkg_id,
            &workflow::Dag::OUTPUT_PORT_FROM_STRING.module,
            &workflow::Dag::OUTPUT_PORT_FROM_STRING.name,
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
                    .expect("Failed to convert JSON to DataStorage"),
            )]),
        );

        let mut tx = sui::tx::TransactionBuilder::new();
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui_mocks::mock_sui_address(),
            skill_id: 11,
            payment_source: vec![1, 2],
            payment_coin: None,
            payment_coin_balance: None,
            payment_max_budget: 55,
            payment_total_budget: None,
            payment_refund_mode: 7,
            authorization_plan_commitment: Some(vec![9, 8]),
            authorization_plan: Vec::new(),
        };

        execute_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            &agent,
            None,
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
                    call.function == workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.name
                }
                _ => false,
            })
            .expect("agent begin call");
        let config_index = begin_index - 1;
        let request_index = inspector
            .commands()
            .iter()
            .position(|command| match command {
                sui::types::Command::MoveCall(call) => {
                    call.function == workflow::Dag::REQUEST_NETWORK_TO_EXECUTE_WALKS.name
                }
                _ => false,
            })
            .expect("request walk call");
        let config_call = inspector.move_call(config_index);
        assert_eq!(
            config_call.function,
            workflow::Dag::NEW_AGENT_EXECUTION_CONFIG.name
        );
        assert_eq!(config_call.arguments.len(), 9);
        let sui::types::Argument::Result(dag_id_index) = config_call.arguments[0] else {
            panic!("expected dag ID to come from object::id_from_address");
        };
        let dag_id_call = inspector.move_call(dag_id_index as usize);
        assert_eq!(dag_id_call.package, sui_framework::PACKAGE_ID);
        assert_eq!(
            dag_id_call.module,
            sui_framework::Object::ID_FROM_ADDRESS.module
        );
        assert_eq!(
            dag_id_call.function,
            sui_framework::Object::ID_FROM_ADDRESS.name
        );
        inspector.expect_address(&dag_id_call.arguments[0], *dag.object_id());
        assert_matches!(config_call.arguments[4], sui::types::Argument::Result(_));
        let sui::types::Argument::Result(agent_id_index) = config_call.arguments[5] else {
            panic!("expected agent ID to come from tap::agent_id_from_address");
        };
        let agent_id_call = inspector.move_call(agent_id_index as usize);
        assert_eq!(agent_id_call.package, nexus_objects.interface_pkg_id);
        assert_eq!(
            agent_id_call.module,
            crate::idents::tap::TapStandard::AGENT_ID_FROM_ADDRESS.module
        );
        assert_eq!(
            agent_id_call.function,
            crate::idents::tap::TapStandard::AGENT_ID_FROM_ADDRESS.name
        );
        inspector.expect_address(&agent_id_call.arguments[0], agent_execution.agent_id);

        inspector.expect_u64(&config_call.arguments[6], agent_execution.skill_id);

        let begin_call = inspector.move_call(begin_index);
        assert_eq!(
            begin_call.function,
            workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.name
        );
        assert_eq!(begin_call.arguments.len(), 10);
        inspector.expect_shared_object(&begin_call.arguments[2], &agent, true);
        assert_matches!(
            &begin_call.arguments[5],
            sui::types::Argument::NestedResult(_, 0)
        );
        let sui::types::Input::Pure { value } = inspector.input(&begin_call.arguments[6]) else {
            panic!("expected payment source input");
        };
        let payment_source: Vec<u8> = bcs::from_bytes(value).expect("payment source BCS");
        assert_eq!(payment_source, agent_execution.payment_source);

        let request_call = inspector.move_call(request_index);
        assert_eq!(
            request_call.function,
            workflow::Dag::REQUEST_NETWORK_TO_EXECUTE_WALKS.name
        );

        assert!(!inspector.commands().iter().any(|command| {
            matches!(
                command,
                sui::types::Command::MoveCall(call)
                    if call.function == sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name
                        && call.type_arguments.first()
                            == Some(&sui::types::TypeTag::Struct(Box::new(
                                sui::types::StructTag::new(
                                    nexus_objects.interface_pkg_id,
                                    crate::idents::tap::STANDARD_TAP_MODULE,
                                    sui::types::Identifier::from_static("ExecutionPayment"),
                                    vec![],
                                ),
                            )))
            )
        }));
    }

    #[test]
    fn execute_agent_dag_with_owned_payment_coin_and_authorization_plan_builds_move_values() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let payment_coin = sui_mocks::mock_sui_object_ref();
        let entry_group = "group1";
        let input_data = HashMap::new();
        let tools_gas = HashSet::new();
        let entry = TapVertexAuthorizationPlanEntry {
            vertex: RuntimeVertex::plain("entry"),
            grant_id: sui::types::Address::from_static("0x30"),
            tool_package: sui::types::Address::from_static("0x40"),
            tool_module: "tool".to_string(),
            tool_function: "run".to_string(),
            operation_commitment: vec![7],
            constraints_commitment: vec![8],
            endpoint_revision: Some(InterfaceRevision(2)),
            payment_id: Some(sui::types::Address::from_static("0x60")),
        };
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            payment_source: vec![1, 2],
            payment_coin: Some(payment_coin.clone()),
            payment_coin_balance: Some(1_000),
            payment_max_budget: 55,
            payment_total_budget: Some(66),
            payment_refund_mode: 7,
            authorization_plan_commitment: Some(vec![9, 8]),
            authorization_plan: vec![entry.clone()],
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        execute_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            &agent,
            None,
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
        let split = inspector
            .commands()
            .iter()
            .find_map(|command| match command {
                sui::types::Command::SplitCoins(split) => Some(split),
                _ => None,
            })
            .expect("split coins command");
        assert_eq!(split.amounts.len(), 1);
        inspector.expect_u64(&split.amounts[0], 66);

        let grant_ref_index = inspector
            .move_call_indices_to(
                nexus_objects.workflow_pkg_id,
                &workflow::Dag::TAP_AUTHORIZATION_GRANT_REF_CONSTRUCTOR.module,
                &workflow::Dag::TAP_AUTHORIZATION_GRANT_REF_CONSTRUCTOR.name,
            )
            .into_iter()
            .next()
            .expect("authorization grant ref constructor call");
        let grant_ref_call = inspector.move_call(grant_ref_index);
        assert_eq!(grant_ref_call.arguments.len(), 9);
        assert!(matches!(
            grant_ref_call.arguments[7],
            sui::types::Argument::Result(_)
        ));
        assert!(matches!(
            grant_ref_call.arguments[8],
            sui::types::Argument::Result(_)
        ));

        let begin_index = inspector
            .move_call_indices_to(
                nexus_objects.workflow_pkg_id,
                &workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.module,
                &workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.name,
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
            payment_source: vec![1],
            payment_coin: None,
            payment_coin_balance: None,
            payment_max_budget: 3,
            payment_total_budget: None,
            payment_refund_mode: 4,
            authorization_plan_commitment: None,
            authorization_plan: Vec::new(),
        };

        let mut tx = sui::tx::TransactionBuilder::new();
        execute_default_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            None,
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
            call.package == nexus_objects.workflow_pkg_id
                && call.module == workflow::Dag::NEW_DAG_EXECUTION_CONFIG.module
                && call.function == workflow::Dag::NEW_DAG_EXECUTION_CONFIG.name
        }));
        assert!(calls.iter().any(|call| {
            call.package == nexus_objects.workflow_pkg_id
                && call.module == workflow::Dag::BEGIN_DAG_EXECUTION_WITH_CONFIG.module
                && call.function == workflow::Dag::BEGIN_DAG_EXECUTION_WITH_CONFIG.name
        }));
        assert!(!calls.iter().any(|call| {
            call.package == nexus_objects.workflow_pkg_id
                && call.module == workflow::Dag::NEW_AGENT_EXECUTION_CONFIG.module
                && call.function == workflow::Dag::NEW_AGENT_EXECUTION_CONFIG.name
        }));
        let shared_inputs = inspector
            .inputs()
            .iter()
            .filter_map(|input| match input {
                sui::types::Input::Shared {
                    object_id, mutable, ..
                } => Some((*object_id, *mutable)),
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
                    .expect("Failed to convert JSON to DataStorage"),
            )]),
        );

        let mut tx = sui::tx::TransactionBuilder::new();
        let agent_execution = AgentDagExecuteInput {
            agent_id: sui_mocks::mock_sui_address(),
            skill_id: 11,
            payment_source: vec![1],
            payment_coin: None,
            payment_coin_balance: None,
            payment_max_budget: 3,
            payment_total_budget: None,
            payment_refund_mode: 4,
            authorization_plan_commitment: None,
            authorization_plan: Vec::new(),
        };
        execute_agent_dag(
            &mut tx,
            &nexus_objects,
            &dag,
            &agent,
            None,
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
                    && call.module == workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.module
                    && call.function == workflow::Dag::BEGIN_AGENT_EXECUTION_WITH_CONFIG.name
            }),
            "agent DAG execution must use the explicit agent entrypoint"
        );
        assert!(
            !calls.iter().any(|call| {
                call.package == nexus_objects.workflow_pkg_id
                    && call.module == workflow::Dag::LEADER_STAMP_WORKSHEET.module
                    && (call.function == workflow::Dag::LEADER_STAMP_WORKSHEET.name
                        || call.function == workflow::Dag::LEADER_STAMP_WORKSHEET_FOR_DRY_RUN.name)
            }),
            "agent DAG builders must not call legacy witness worksheet stamp helpers"
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
                leader_verifier: Some(VerifierConfig {
                    mode: VerifierMode::LeaderNautilusEnclave,
                    method: "nautilus_v1".to_string(),
                }),
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
            tool_verifier: Some(VerifierConfig {
                mode: VerifierMode::None,
                method: "none".to_string(),
            }),
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
            call.module == workflow::Dag::WITH_DEFAULT_TOOL_VERIFIER.module
                && call.function == workflow::Dag::WITH_DEFAULT_TOOL_VERIFIER.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.module == workflow::Dag::WITH_VERTEX_LEADER_VERIFIER.module
                && call.function == workflow::Dag::WITH_VERTEX_LEADER_VERIFIER.name
        }));
        assert!(move_calls.iter().any(|call| {
            call.module == workflow::Dag::WITH_VERTEX_TOOL_VERIFIER.module
                && call.function == workflow::Dag::WITH_VERTEX_TOOL_VERIFIER.name
        }));
        assert_eq!(
            move_calls
                .iter()
                .filter(|call| {
                    call.module == workflow::Dag::VERIFIER_CONFIG.module
                        && call.function == workflow::Dag::VERIFIER_CONFIG.name
                })
                .count(),
            4
        );
    }
}
