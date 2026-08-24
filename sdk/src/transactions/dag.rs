use {
    crate::{
        move_bindings::{
            interface::{
                dag as dag_binding,
                graph::{self as graph_binding, PostFailureAction, RuntimeVertex},
                meta_schema::MetaSchema,
                verifier::{self as verifier_binding, RegisteredKeyAuxiliary, ToolVerifierMode},
            },
            move_std::option::Option as MoveOption,
            primitives::tagged_output as tagged_output_binding,
            registry::registered_key_verifier as registered_key_verifier_binding,
            scheduler::{
                execution_settlement as execution_settlement_binding,
                execution_submission as execution_submission_binding,
            },
            tool::tool_registry as tool_registry_binding,
        },
        move_boundary,
        sui,
        transactions::{agent_input::AgentInput, scheduler},
        types::{
            DagDefaultValue,
            DagEdge,
            DagEntryPort,
            DagOutput,
            DagSpec,
            DagVertex,
            DagVertexKind,
            ExternalVerifierRuntimeCall,
            NexusContext,
            NexusData,
            OffchainToolOutput,
            DEFAULT_ENTRY_GROUP,
        },
    },
    std::collections::HashMap,
    sui::types::ProgrammableTransaction,
};

fn vertex_kind_arg(
    tx: &mut move_boundary::NexusPtbBuilder,
    kind: &DagVertexKind,
) -> anyhow::Result<sui::types::Argument> {
    match kind {
        DagVertexKind::OffChain { tool_fqn } => {
            tx.graph_vertex_kind_off_chain(tool_fqn.to_string())
        }
        DagVertexKind::OnChain { tool_fqn } => tx.graph_vertex_kind_on_chain(tool_fqn.to_string()),
    }
}

pub(crate) fn runtime_vertex_arg(
    tx: &mut move_boundary::NexusPtbBuilder,
    runtime_vertex: &RuntimeVertex,
) -> anyhow::Result<sui::types::Argument> {
    match runtime_vertex {
        RuntimeVertex::Plain { vertex } => {
            let vertex = tx.ascii_string(vertex.name.as_str())?;
            tx.call_target(
                graph_binding::runtime_vertex_plain_from_string_target,
                vec![vertex],
            )
        }
        RuntimeVertex::WithIterator {
            vertex,
            iteration,
            out_of,
        } => {
            let vertex = tx.ascii_string(vertex.name.as_str())?;
            let iteration = tx.arg(iteration)?;
            let out_of = tx.arg(out_of)?;
            tx.call_target(
                graph_binding::runtime_vertex_with_iterator_from_string_target,
                vec![vertex, iteration, out_of],
            )
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeToolResultWorksheet {
    pub worksheet: sui::types::Argument,
    pub stamp: sui::types::Argument,
    pub agent_registry: sui::types::Argument,
    pub dag: sui::types::Argument,
    pub execution: sui::types::Argument,
    pub clock: sui::types::Argument,
    pub tool_registry: sui::types::Argument,
    pub network_auth: sui::types::Argument,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeToolResultWorksheetInputs {
    pub dag: sui::types::ObjectReference,
    pub execution: (sui::types::Address, sui::types::Version),
    pub leader_registry: sui::types::Argument,
    pub leader_cap: sui::types::Argument,
    pub walk_index: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OffchainVerifierKeyBindings {
    pub leader_key_binding: sui::types::ObjectReference,
    pub tool_key_binding: sui::types::ObjectReference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedOffchainToolResultSubmission {
    NoVerifier {
        result: OffchainToolOutput,
        meta_schema: MetaSchema,
    },
    RegisteredKeyVerifier {
        tool_id: sui::types::Address,
        result: OffchainToolOutput,
        meta_schema: MetaSchema,
        auxiliary: RegisteredKeyAuxiliary,
        bindings: OffchainVerifierKeyBindings,
    },
    ExternalVerifier {
        result: OffchainToolOutput,
        meta_schema: MetaSchema,
        auxiliary: Vec<u8>,
        runtime_call: ExternalVerifierRuntimeCall,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnchainToolArgument {
    ObjectId(sui::types::Address),
    Pure(Vec<u8>),
    SharedObject {
        object_id: sui::types::Address,
        initial_shared_version: sui::types::Version,
        mutable: bool,
    },
    Object(sui::types::ObjectReference),
    Vector {
        type_tag: Option<sui::types::TypeTag>,
        elements: Vec<OnchainToolArgument>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedOnchainToolExecution {
    pub package: sui::types::Address,
    pub module: String,
    pub tool_witness_id: sui::types::Address,
    pub requires_authorization_cap: bool,
    pub arguments: Vec<OnchainToolArgument>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedOnchainToolResultSubmission {
    Execute(PreparedOnchainToolExecution),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokenOnchainToolResultCleanupInput {
    pub walk_index: u64,
    pub result_ref: sui::types::ObjectReference,
    pub tool_witness_id: sui::types::Address,
}

fn build_runtime_tool_result_worksheet(
    tx: &mut move_boundary::NexusPtbBuilder,
    inputs: RuntimeToolResultWorksheetInputs,
) -> anyhow::Result<RuntimeToolResultWorksheet> {
    let agent_registry_ref = tx.objects().agent_registry;
    let tool_registry_ref = tx.objects().tool_registry;
    let network_auth_ref = tx.objects().network_auth;
    let agent_registry = tx.shared_root(&agent_registry_ref, false)?;
    let tool_registry = tx.shared_root(&tool_registry_ref, false)?;
    let network_auth = tx.shared_root(&network_auth_ref, false)?;
    let dag = tx.immutable_object(&inputs.dag)?;
    let execution = tx.shared_object_by_id(inputs.execution.0, inputs.execution.1, true)?;
    let clock = tx.clock()?;
    let walk_index = tx.arg(&inputs.walk_index)?;
    let runtime_authority = tx.runtime_authority(false)?;
    let prepared = tx.call_target(
        execution_submission_binding::prepare_tool_result_submission_worksheet_target,
        vec![
            runtime_authority,
            dag,
            agent_registry,
            tool_registry,
            network_auth,
            inputs.leader_registry,
            execution,
            inputs.leader_cap,
            walk_index,
            clock,
        ],
    )?;

    Ok(RuntimeToolResultWorksheet {
        worksheet: tx.nested_result(prepared, 0)?,
        stamp: tx.nested_result(prepared, 1)?,
        agent_registry,
        dag,
        execution,
        clock,
        tool_registry,
        network_auth,
    })
}

#[derive(Clone, Copy)]
struct NewDagArguments {
    dag: sui::types::Argument,
    owner_cap: sui::types::Argument,
}

/// PTB template for creating a new empty DAG and its owner capability.
fn empty(tx: &mut move_boundary::NexusPtbBuilder) -> anyhow::Result<NewDagArguments> {
    let result = tx.call_target(dag_binding::new_target, vec![])?;
    Ok(NewDagArguments {
        dag: tx.nested_result(result, 0)?,
        owner_cap: tx.nested_result(result, 1)?,
    })
}

/// PTB template to publish a DAG.
pub(crate) fn publish(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    owner_cap: sui::types::Argument,
) -> anyhow::Result<()> {
    tx.call_target(dag_binding::finalize_target, vec![dag, owner_cap])?;
    Ok(())
}

/// PTB template to publish a full [`DagSpec`].
pub(crate) fn create(
    tx: &mut move_boundary::NexusPtbBuilder,
    mut dag_arg: sui::types::Argument,
    dag_owner: sui::types::Argument,
    dag: DagSpec,
) -> anyhow::Result<sui::types::Argument> {
    // Create all vertices.
    for vertex in &dag.vertices {
        create_registered_vertex(tx, dag_arg, dag_owner, vertex)?;

        if let Some(action) = &vertex.post_failure_action {
            dag_arg = create_vertex_post_failure_action(tx, dag_arg, &vertex.name, action)?;
        }

        if let Some(mode) = &vertex.verifier {
            if matches!(&vertex.kind, DagVertexKind::OnChain { .. }) {
                anyhow::bail!(
                    "on-chain vertex '{}' cannot configure an off-chain verifier",
                    vertex.name
                );
            }
            create_vertex_verifier_mode(tx, dag_arg, dag_owner, &vertex.name, mode)?;
        }
    }

    if let Some(action) = &dag.post_failure_action {
        dag_arg = create_post_failure_action(tx, dag_arg, action)?;
    }

    // Create all default values if present.
    for default_value in &dag.default_values {
        dag_arg = create_default_value(tx, dag_arg, default_value)?;
    }

    // Create all edges.
    for edge in &dag.edges {
        dag_arg = create_edge(tx, dag_arg, edge)?;
    }

    // Create all outputs.
    for output in &dag.outputs {
        dag_arg = create_output(tx, dag_arg, output)?;
    }

    // Create all entry ports and vertices. Or create a default entry group
    // with all specified entry ports if none is present.
    if !dag.entry_groups.is_empty() {
        for entry_group in &dag.entry_groups {
            for vertex in &entry_group.vertices {
                let entry_ports = dag
                    .vertices
                    .iter()
                    .find(|v| &v.name == vertex)
                    .map(|v| &v.entry_ports);

                if let Some(entry_ports) = entry_ports.filter(|ports| !ports.is_empty()) {
                    for entry_port in entry_ports {
                        dag_arg = mark_entry_input_port(
                            tx,
                            dag_arg,
                            vertex,
                            entry_port,
                            &entry_group.name,
                        )?;
                    }
                } else {
                    dag_arg = mark_entry_vertex(tx, dag_arg, vertex, &entry_group.name)?;
                }
            }
        }
    } else {
        for vertex in &dag.vertices {
            if vertex.entry_ports.is_empty() {
                continue;
            }

            for entry_port in &vertex.entry_ports {
                dag_arg = mark_entry_input_port(
                    tx,
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

/// Build a PTB that publishes a full [`DagSpec`].
pub(crate) fn publish_ptb(
    objects: &NexusContext,
    dag: DagSpec,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let new_dag = empty(tx)?;
        let mut dag_arg = new_dag.dag;
        dag_arg = create(tx, dag_arg, new_dag.owner_cap, dag)?;
        publish(tx, dag_arg, new_dag.owner_cap)?;
        Ok(())
    })
}

fn tool_registry_arg(
    tx: &mut move_boundary::NexusPtbBuilder,
) -> anyhow::Result<sui::types::Argument> {
    let tool_registry = tx.objects().tool_registry;
    Ok(tx.shared_root(&tool_registry, false)?)
}

/// PTB template for creating one DAG vertex from the current Tool Registry binding.
fn create_registered_vertex(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    dag_owner: sui::types::Argument,
    vertex: &DagVertex,
) -> anyhow::Result<()> {
    let tool_registry = tool_registry_arg(tx)?;

    // `name: Vertex`
    let name = tx.graph_vertex(&vertex.name)?;

    // `kind: VertexKind`
    let kind = vertex_kind_arg(tx, &vertex.kind)?;

    tx.call_target(
        tool_registry_binding::add_vertex_to_dag_target,
        vec![tool_registry, dag, dag_owner, name, kind],
    )?;
    Ok(())
}

/// PTB template for configuring a DAG-level default post-failure action.
fn create_post_failure_action(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    action: &PostFailureAction,
) -> anyhow::Result<sui::types::Argument> {
    let action = tx.graph_post_failure_action(action)?;

    tx.call_target(
        dag_binding::with_post_failure_action_target,
        vec![dag, action],
    )
}

/// PTB template for configuring a vertex-level post-failure action override.
fn create_vertex_post_failure_action(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    vertex: &str,
    action: &PostFailureAction,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(vertex)?;
    let action = tx.graph_post_failure_action(action)?;

    tx.call_target(
        dag_binding::with_vertex_post_failure_action_target,
        vec![dag, vertex, action],
    )
}

fn create_vertex_verifier_mode(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    dag_owner: sui::types::Argument,
    vertex: &str,
    mode: &ToolVerifierMode,
) -> anyhow::Result<()> {
    let vertex = tx.graph_vertex(vertex)?;
    let tool_registry = tool_registry_arg(tx)?;
    let mode = tx.tool_verifier_mode(mode)?;

    tx.call_target(
        tool_registry_binding::set_registered_vertex_verifier_mode_target,
        vec![tool_registry, dag, dag_owner, vertex, mode],
    )?;
    Ok(())
}

/// Builds a [`ProgrammableTransaction`] that refills TAP execution payment from
/// the sender's address balance.
pub(crate) fn refill_tap_execution_payment_for_self_ptb(
    objects: &NexusContext,
    execution: &sui::types::ObjectReference,
    amount: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let execution = tx.shared_object(execution, true)?;
        let coin = tx.withdraw_sui_coin(amount)?;
        let runtime_authority = tx.runtime_authority(false)?;
        tx.call_target(
            execution_settlement_binding::refill_tap_execution_payment_target,
            vec![runtime_authority, execution, coin],
        )?;
        Ok(())
    })
}

struct OffchainVerifierPtbObjects {
    network_auth: sui::types::Argument,
    leader_key_binding: sui::types::Argument,
    tool_key_binding: sui::types::Argument,
}

fn offchain_verifier_ptb_objects(
    tx: &mut move_boundary::NexusPtbBuilder,
    bindings: &OffchainVerifierKeyBindings,
) -> anyhow::Result<OffchainVerifierPtbObjects> {
    let network_auth_ref = tx.objects().network_auth;
    let network_auth = tx.shared_root(&network_auth_ref, false)?;
    let leader_key_binding = tx.shared_object(&bindings.leader_key_binding, false)?;
    let tool_key_binding = if bindings.tool_key_binding == bindings.leader_key_binding {
        leader_key_binding
    } else {
        tx.shared_object(&bindings.tool_key_binding, false)?
    };

    Ok(OffchainVerifierPtbObjects {
        network_auth,
        leader_key_binding,
        tool_key_binding,
    })
}

pub(crate) fn prepare_onchain_tool_argument(
    tx: &mut move_boundary::NexusPtbBuilder,
    argument: &OnchainToolArgument,
    pre_allocated: &HashMap<sui::types::Address, sui::types::Argument>,
) -> anyhow::Result<sui::types::Argument> {
    match argument {
        OnchainToolArgument::ObjectId(object_id) => Ok(tx.object_id(*object_id)?),
        OnchainToolArgument::Pure(bytes) => Ok(tx.pure_bcs(bytes.clone())?),
        OnchainToolArgument::SharedObject {
            object_id,
            initial_shared_version,
            mutable,
        } => {
            if let Some(existing_arg) = pre_allocated.get(object_id).copied() {
                if *mutable {
                    Ok(tx.shared_object_by_id(*object_id, *initial_shared_version, true)?)
                } else {
                    Ok(existing_arg)
                }
            } else {
                Ok(tx.shared_object_by_id(*object_id, *initial_shared_version, *mutable)?)
            }
        }
        OnchainToolArgument::Object(object) => Ok(tx.owned_object(object)?),
        OnchainToolArgument::Vector { type_tag, elements } => {
            let elements = elements
                .iter()
                .map(|element| prepare_onchain_tool_argument(tx, element, pre_allocated))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(tx.make_move_vector(type_tag.clone(), elements)?)
        }
    }
}

fn prepare_onchain_tool_arguments(
    tx: &mut move_boundary::NexusPtbBuilder,
    arguments: &[OnchainToolArgument],
    pre_allocated: &HashMap<sui::types::Address, sui::types::Argument>,
) -> anyhow::Result<Vec<sui::types::Argument>> {
    arguments
        .iter()
        .map(|argument| prepare_onchain_tool_argument(tx, argument, pre_allocated))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn runtime_pre_allocated_objects(
    objects: &NexusContext,
    dag_ref: &sui::types::ObjectReference,
    execution_ref: (sui::types::Address, sui::types::Version),
    agent_registry: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    clock: sui::types::Argument,
    tool_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
) -> HashMap<sui::types::Address, sui::types::Argument> {
    HashMap::from([
        (*dag_ref.object_id(), dag),
        (move_boundary::CLOCK_OBJECT_ID, clock),
        (execution_ref.0, execution),
        (objects.tool_registry.object_id(), tool_registry),
        (objects.leader_registry.object_id(), leader_registry),
        (objects.agent_registry.object_id(), agent_registry),
    ])
}

#[allow(clippy::too_many_arguments)]
fn commit_prepared_onchain_tool_execution(
    tx: &mut move_boundary::NexusPtbBuilder,
    execution_plan: &PreparedOnchainToolExecution,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    stamp: sui::types::Argument,
    leader_cap: sui::types::Argument,
    leader_registry: sui::types::Argument,
    walk_index: u64,
    expected_vertex: sui::types::Argument,
    pre_allocated: &HashMap<sui::types::Address, sui::types::Argument>,
) -> anyhow::Result<()> {
    let authorization = if execution_plan.requires_authorization_cap {
        Some(release_vertex_authorization_for_onchain_walk(
            tx, dag, execution, worksheet, stamp, leader_cap, walk_index,
        )?)
    } else {
        None
    };
    let (requirements, result) = create_on_chain_tool_result_for_walk(
        tx,
        dag,
        execution,
        tool_registry,
        worksheet,
        stamp,
        leader_cap,
        leader_registry,
        walk_index,
        expected_vertex,
    )?;
    let user_args = prepare_onchain_tool_arguments(tx, &execution_plan.arguments, pre_allocated)?;
    let mut tool_args = if let Some(authorization) = authorization {
        vec![authorization, requirements, result]
    } else {
        vec![requirements, result]
    };
    tool_args.extend(user_args);

    tx.call_function(
        execution_plan.package,
        execution_plan.module.as_str(),
        "execute",
        tool_args,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_off_chain_tool_result_for_walk_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
    submission: &PreparedOffchainToolResultSubmission,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let RuntimeToolResultWorksheet {
            worksheet,
            stamp,
            agent_registry: _,
            dag,
            execution,
            clock: _,
            tool_registry,
            network_auth,
        } = build_runtime_tool_result_worksheet(
            tx,
            RuntimeToolResultWorksheetInputs {
                dag: dag.clone(),
                execution,
                leader_registry,
                leader_cap,
                walk_index,
            },
        )?;

        let verdict = match submission {
            PreparedOffchainToolResultSubmission::NoVerifier {
                result,
                meta_schema,
            } => {
                let result = prepare_offchain_tool_response(tx, result, meta_schema)?;
                tx.call_target(verifier_binding::new_none_target, vec![result])?
            }
            PreparedOffchainToolResultSubmission::RegisteredKeyVerifier {
                tool_id,
                result,
                meta_schema,
                auxiliary,
                bindings,
            } => {
                let verifier_objects = offchain_verifier_ptb_objects(tx, bindings)?;
                let result = prepare_offchain_tool_response(tx, result, meta_schema)?;
                let auxiliary = prepare_registered_key_auxiliary(tx, auxiliary)?;
                let tool_id = tx.object_id(*tool_id)?;
                tx.call_target(
                    registered_key_verifier_binding::verify_target,
                    vec![
                        worksheet,
                        result,
                        auxiliary,
                        leader_registry,
                        leader_cap,
                        verifier_objects.network_auth,
                        verifier_objects.leader_key_binding,
                        verifier_objects.tool_key_binding,
                        tool_id,
                    ],
                )?
            }
            PreparedOffchainToolResultSubmission::ExternalVerifier {
                result,
                meta_schema,
                auxiliary,
                runtime_call,
            } => {
                call_external_verifier(tx, worksheet, result, meta_schema, auxiliary, runtime_call)?
            }
        };

        commit_off_chain_tool_result_for_walk(
            tx,
            dag,
            execution,
            tool_registry,
            network_auth,
            worksheet,
            stamp,
            verdict,
            leader_cap,
            leader_registry,
            walk_index,
            next_vertex,
        )?;

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn submit_on_chain_tool_result_for_walk_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
    submission: &PreparedOnchainToolResultSubmission,
) -> anyhow::Result<ProgrammableTransaction> {
    let dag_ref = dag;
    let execution_ref = execution;

    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let expected_vertex = runtime_vertex_arg(tx, next_vertex)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let RuntimeToolResultWorksheet {
            worksheet,
            stamp,
            agent_registry,
            dag: dag_arg,
            execution: execution_arg,
            clock,
            tool_registry,
            network_auth: _,
        } = build_runtime_tool_result_worksheet(
            tx,
            RuntimeToolResultWorksheetInputs {
                dag: dag_ref.clone(),
                execution: execution_ref,
                leader_registry,
                leader_cap,
                walk_index,
            },
        )?;

        let PreparedOnchainToolResultSubmission::Execute(execution_plan) = submission;
        let pre_allocated = runtime_pre_allocated_objects(
            objects,
            dag_ref,
            execution_ref,
            agent_registry,
            dag_arg,
            execution_arg,
            clock,
            tool_registry,
            leader_registry,
        );
        commit_prepared_onchain_tool_execution(
            tx,
            execution_plan,
            dag_arg,
            execution_arg,
            tool_registry,
            worksheet,
            stamp,
            leader_cap,
            leader_registry,
            walk_index,
            expected_vertex,
            &pre_allocated,
        )?;

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn consume_on_chain_tool_result_for_walk_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    invocation: &sui::types::ObjectReference,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
    result: (sui::types::Address, sui::types::Version),
    tool_witness_id: sui::types::Address,
    finalize_gas_charge: u64,
    settlement_gas_charge: u64,
    task_settlement: Option<&sui::types::ObjectReference>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let dag = tx.immutable_object(dag)?;
        let execution = tx.shared_object_by_id(execution.0, execution.1, true)?;
        let tool_registry = tx.shared_root(&objects.tool_registry, false)?;
        let result = tx.shared_object_by_id(result.0, result.1, true)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let priority_fee_vault = tx.shared_root(&objects.priority_fee_vault, false)?;
        let clock = tx.clock()?;

        consume_on_chain_tool_result_for_walk(
            tx,
            dag,
            execution,
            tool_registry,
            result,
            leader_cap,
            leader_registry,
            priority_fee_vault,
            walk_index,
            next_vertex,
            tool_witness_id,
            finalize_gas_charge,
            settlement_gas_charge,
            clock,
        )?;

        super::invocation::settle(tx, dag, execution, next_vertex, invocation)?;

        emit_payment_ready_walk_requests(tx, dag, execution, leader_registry, clock);

        if let Some(task) = task_settlement {
            scheduler::append_settle_occurrence(tx, task, execution, clock)?;
        }

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn dry_run_on_chain_tool_result_for_walk_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
    execution_plan: &PreparedOnchainToolExecution,
) -> anyhow::Result<ProgrammableTransaction> {
    let dag_ref = dag;
    let execution_ref = execution;

    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let RuntimeToolResultWorksheet {
            worksheet,
            stamp,
            agent_registry,
            dag: dag_arg,
            execution: execution_arg,
            clock,
            tool_registry,
            network_auth: _,
        } = build_runtime_tool_result_worksheet(
            tx,
            RuntimeToolResultWorksheetInputs {
                dag: dag_ref.clone(),
                execution: execution_ref,
                leader_registry,
                leader_cap,
                walk_index,
            },
        )?;
        let pre_allocated = runtime_pre_allocated_objects(
            objects,
            dag_ref,
            execution_ref,
            agent_registry,
            dag_arg,
            execution_arg,
            clock,
            tool_registry,
            leader_registry,
        );
        let expected_vertex = runtime_vertex_arg(tx, next_vertex)?;

        commit_prepared_onchain_tool_execution(
            tx,
            execution_plan,
            dag_arg,
            execution_arg,
            tool_registry,
            worksheet,
            stamp,
            leader_cap,
            leader_registry,
            walk_index,
            expected_vertex,
            &pre_allocated,
        )
    })
}

/// Build a PTB that refills TAP execution payment from an agent vault.
pub(crate) fn refill_tap_execution_payment_from_agent_vault_for_self_ptb(
    objects: &NexusContext,
    agent: AgentInput,
    execution: &sui::types::ObjectReference,
    amount: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let agent = agent.mutable_ptb_argument(tx)?;
        let execution = tx.shared_object(execution, true)?;
        let amount = tx.arg(&amount)?;
        let runtime_authority = tx.runtime_authority(false)?;
        tx.call_target(
            execution_settlement_binding::refill_tap_execution_payment_from_agent_vault_target,
            vec![runtime_authority, agent, execution, amount],
        )?;
        Ok(())
    })
}

fn prepare_offchain_tool_response(
    tx: &mut move_boundary::NexusPtbBuilder,
    result: &OffchainToolOutput,
    meta_schema: &MetaSchema,
) -> anyhow::Result<sui::types::Argument> {
    let ports = meta_schema.canonical_output_ports(result)?;
    prepare_tagged_output(
        tx,
        &result.tag,
        ports.iter().map(|(name, value)| (name.as_slice(), value)),
    )
}

fn prepare_registered_key_auxiliary(
    tx: &mut move_boundary::NexusPtbBuilder,
    auxiliary: &RegisteredKeyAuxiliary,
) -> anyhow::Result<sui::types::Argument> {
    crate::nexus::registered_key::validate_registered_key_auxiliary(auxiliary)?;
    let input_hash = tx.arg(&auxiliary.input_hash)?;
    let nonce = tx.arg(&auxiliary.nonce)?;
    let leader_signature = tx.arg(&auxiliary.leader_signature)?;
    let tool_signature = tx.arg(&auxiliary.tool_signature)?;
    tx.call_target(
        verifier_binding::registered_key_auxiliary_target,
        vec![input_hash, nonce, leader_signature, tool_signature],
    )
}

fn call_external_verifier(
    tx: &mut move_boundary::NexusPtbBuilder,
    worksheet: sui::types::Argument,
    result: &OffchainToolOutput,
    meta_schema: &MetaSchema,
    auxiliary: &[u8],
    runtime_call: &ExternalVerifierRuntimeCall,
) -> anyhow::Result<sui::types::Argument> {
    let result = prepare_offchain_tool_response(tx, result, meta_schema)?;
    let auxiliary = tx.arg(&auxiliary.to_vec())?;
    let verifier_objects = runtime_call
        .immutable_shared_objects
        .iter()
        .map(|object_ref| tx.shared_object(object_ref, false))
        .collect::<Result<Vec<_>, _>>()?;
    let mut args = Vec::with_capacity(verifier_objects.len() + 3);
    args.extend([worksheet, result, auxiliary]);
    args.extend(verifier_objects);
    tx.call_function(
        runtime_call.method_id.package_id.bytes,
        String::from(runtime_call.method_id.module_name.clone()).as_str(),
        String::from(runtime_call.method_id.function_name.clone()).as_str(),
        args,
    )
}

#[allow(clippy::too_many_arguments)]
fn commit_off_chain_tool_result_for_walk(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    network_auth: sui::types::Argument,
    worksheet: sui::types::Argument,
    stamp: sui::types::Argument,
    verdict: sui::types::Argument,
    leader_cap: sui::types::Argument,
    leader_registry: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let runtime_authority = tx.runtime_authority(false)?;

    tx.call_target(
        execution_submission_binding::commit_off_chain_tool_result_for_walk_target,
        vec![
            runtime_authority,
            dag,
            execution,
            tool_registry,
            network_auth,
            worksheet,
            stamp,
            verdict,
            leader_cap,
            leader_registry,
            walk_index,
            expected_vertex,
        ],
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn release_vertex_authorization_for_onchain_walk(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    worksheet: sui::types::Argument,
    stamp: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
) -> anyhow::Result<sui::types::Argument> {
    let walk_index = tx.arg(&walk_index)?;
    let runtime_authority = tx.runtime_authority(false)?;
    tx.call_target(
        execution_submission_binding::release_vertex_authorization_for_onchain_walk_target,
        vec![
            runtime_authority,
            dag,
            execution,
            worksheet,
            stamp,
            leader_cap,
            walk_index,
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_on_chain_tool_result_for_walk(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    stamp: sui::types::Argument,
    leader_cap: sui::types::Argument,
    leader_registry: sui::types::Argument,
    walk_index: u64,
    expected_vertex: sui::types::Argument,
) -> anyhow::Result<(sui::types::Argument, sui::types::Argument)> {
    let walk_index = tx.arg(&walk_index)?;
    let runtime_authority = tx.runtime_authority(false)?;

    let result = tx.call_target(
        execution_submission_binding::create_on_chain_tool_result_for_walk_target,
        vec![
            runtime_authority,
            dag,
            execution,
            tool_registry,
            worksheet,
            stamp,
            leader_cap,
            leader_registry,
            walk_index,
            expected_vertex,
        ],
    )?;
    Ok((tx.nested_result(result, 0)?, tx.nested_result(result, 1)?))
}

pub fn framework_random_object(
    tx: &mut move_boundary::NexusPtbBuilder,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.shared_object_by_id(move_boundary::RANDOM_OBJECT_ID, 1, false)?)
}

#[allow(clippy::too_many_arguments)]
pub fn consume_on_chain_tool_result_for_walk(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    result: sui::types::Argument,
    leader_cap: sui::types::Argument,
    leader_registry: sui::types::Argument,
    priority_fee_vault: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    tool_witness_id: sui::types::Address,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let tool_witness_id = tx.object_id(tool_witness_id)?;
    let commit_gas_charge = tx.arg(&commit_gas_charge)?;
    let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;
    let runtime_authority = tx.runtime_authority(false)?;

    tx.call_target(
        execution_submission_binding::consume_on_chain_tool_result_for_walk_target,
        vec![
            runtime_authority,
            dag,
            execution,
            tool_registry,
            result,
            leader_cap,
            leader_registry,
            priority_fee_vault,
            walk_index,
            expected_vertex,
            tool_witness_id,
            commit_gas_charge,
            settlement_gas_charge,
            clock,
        ],
    )?;

    Ok(())
}

fn prepare_tagged_output<'a>(
    tx: &mut move_boundary::NexusPtbBuilder,
    tag: &[u8],
    ports: impl IntoIterator<Item = (&'a [u8], &'a NexusData)>,
) -> anyhow::Result<sui::types::Argument> {
    let tag = tx.arg(&tag.to_vec())?;
    let mut output = tx.call_target(tagged_output_binding::new_target, vec![tag])?;
    for (name, data) in ports {
        let name = tx.arg(&name.to_vec())?;
        output = if data.is_one() {
            let values = data.values()?;
            let value = tx.nexus_value(
                values
                    .first()
                    .expect("well-formed One contains exactly one value"),
            )?;
            tx.call_target(
                tagged_output_binding::with_named_payload_target,
                vec![output, name, value],
            )?
        } else {
            let values = data
                .values()?
                .iter()
                .map(|value| tx.nexus_value(value))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let values =
                tx.move_vector::<crate::move_bindings::primitives::data::NexusValue>(values)?;
            tx.call_target(
                tagged_output_binding::with_named_payload_many_target,
                vec![output, name, values],
            )?
        };
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn record_committed_tool_result_gas_charge_by_leader(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    leader_registry: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    failed_onchain_tool_reason: Option<Vec<u8>>,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let failed_onchain_tool_reason =
        tx.arg(&MoveOption::from_option(failed_onchain_tool_reason))?;
    let commit_tx_digest = tx.arg(&commit_tx_digest)?;
    let commit_gas_charge = tx.arg(&commit_gas_charge)?;
    let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;
    let runtime_authority = tx.runtime_authority(false)?;

    tx.call_target(
        execution_settlement_binding::record_committed_tool_result_gas_charge_by_leader_target,
        vec![
            runtime_authority,
            dag,
            execution,
            leader_registry,
            leader_cap,
            walk_index,
            expected_vertex,
            failed_onchain_tool_reason,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
            clock,
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn record_committed_tool_result_gas_charge_by_leader_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    failed_onchain_tool_reason: Option<Vec<u8>>,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.immutable_object(dag)?;
        let execution = tx.shared_object_by_id(execution.0, execution.1, true)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let clock = tx.clock()?;

        record_committed_tool_result_gas_charge_by_leader(
            tx,
            dag,
            execution,
            leader_registry,
            leader_cap,
            walk_index,
            expected_vertex,
            failed_onchain_tool_reason,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
            clock,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn settle_committed_tool_result_for_walk_by_leader(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
    priority_fee_vault: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    failed_onchain_tool_reason: Option<Vec<u8>>,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let failed_onchain_tool_reason =
        tx.arg(&MoveOption::from_option(failed_onchain_tool_reason))?;
    let commit_tx_digest = tx.arg(&commit_tx_digest)?;
    let commit_gas_charge = tx.arg(&commit_gas_charge)?;
    let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;
    let runtime_authority = tx.runtime_authority(false)?;

    tx.call_target(
        execution_settlement_binding::settle_committed_tool_result_for_walk_by_leader_target,
        vec![
            runtime_authority,
            dag,
            execution,
            tool_registry,
            leader_registry,
            priority_fee_vault,
            leader_cap,
            walk_index,
            expected_vertex,
            failed_onchain_tool_reason,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
            clock,
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn settle_committed_tool_result_for_walk_by_leader_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    invocation: &sui::types::ObjectReference,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    failed_onchain_tool_reason: Option<Vec<u8>>,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
    task_settlement: Option<&sui::types::ObjectReference>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let dag = tx.immutable_object(dag)?;
        let execution = tx.shared_object_by_id(execution.0, execution.1, true)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let tool_registry = tx.shared_root(&objects.tool_registry, false)?;
        let priority_fee_vault = tx.shared_root(&objects.priority_fee_vault, false)?;
        let clock = tx.clock()?;

        settle_committed_tool_result_for_walk_by_leader(
            tx,
            dag,
            execution,
            tool_registry,
            leader_registry,
            priority_fee_vault,
            leader_cap,
            walk_index,
            expected_vertex,
            failed_onchain_tool_reason,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
            clock,
        )?;

        super::invocation::settle(tx, dag, execution, expected_vertex, invocation)?;
        emit_payment_ready_walk_requests(tx, dag, execution, leader_registry, clock);

        if let Some(task) = task_settlement {
            scheduler::append_settle_occurrence(tx, task, execution, clock)?;
        }

        Ok(())
    })
}

/// Builds a PTB that aborts an expired DAG execution.
///
/// When `task_settlement` is supplied, the owning Task is settled after the
/// execution transition has removed every accounting lock.
pub fn abort_expired_execution_for_self_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    broken_onchain_result_cleanups: &[BrokenOnchainToolResultCleanupInput],
    task_settlement: Option<&sui::types::ObjectReference>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.immutable_object(dag)?;
        let execution = tx.shared_object(execution, true)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let clock = tx.clock()?;

        if !broken_onchain_result_cleanups.is_empty() {
            let tool_registry = tx.shared_root(&objects.tool_registry, false)?;

            for cleanup in broken_onchain_result_cleanups {
                let result = tx.shared_object(&cleanup.result_ref, true)?;
                cleanup_broken_onchain_tool_result(
                    tx,
                    dag,
                    execution,
                    tool_registry,
                    result,
                    leader_registry,
                    cleanup.walk_index,
                    cleanup.tool_witness_id,
                    clock,
                )?;
            }
        }

        let runtime_authority = tx.runtime_authority(false)?;
        tx.call_target(
            execution_settlement_binding::abort_expired_execution_target,
            vec![runtime_authority, dag, execution, clock],
        )?;
        if let Some(task) = task_settlement {
            scheduler::append_settle_occurrence(tx, task, execution, clock)?;
        }
        Ok(())
    })
}

/// Builds a PTB that settles a committed Tool result for one [RuntimeVertex].
///
/// The exact Invocation is settled before the optional owning Task settlement.
pub fn settle_committed_tool_result_for_walk_for_self_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    invocation: &sui::types::ObjectReference,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    task_settlement: Option<&sui::types::ObjectReference>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.immutable_object(dag)?;
        let execution = tx.shared_object(execution, true)?;
        let tool_registry = tx.shared_root(&objects.tool_registry, false)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let priority_fee_vault = tx.shared_root(&objects.priority_fee_vault, false)?;
        let walk_index = tx.arg(&walk_index)?;
        let clock = tx.clock()?;
        let runtime_authority = tx.runtime_authority(false)?;

        tx.call_target(
            execution_settlement_binding::settle_committed_tool_result_for_walk_target,
            vec![
                runtime_authority,
                dag,
                execution,
                tool_registry,
                leader_registry,
                priority_fee_vault,
                walk_index,
                clock,
            ],
        )?;
        super::invocation::settle(tx, dag, execution, expected_vertex, invocation)?;
        if let Some(task) = task_settlement {
            scheduler::append_settle_occurrence(tx, task, execution, clock)?;
        }
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn settle_onchain_tool_result_for_walk(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    result: sui::types::Argument,
    leader_registry: sui::types::Argument,
    priority_fee_vault: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    tool_witness_id: sui::types::Address,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let tool_witness_id = tx.object_id(tool_witness_id)?;
    let runtime_authority = tx.runtime_authority(false)?;

    tx.call_target(
        execution_settlement_binding::settle_onchain_tool_result_for_walk_target,
        vec![
            runtime_authority,
            dag,
            execution,
            tool_registry,
            result,
            leader_registry,
            priority_fee_vault,
            walk_index,
            expected_vertex,
            tool_witness_id,
            clock,
        ],
    )?;

    Ok(())
}

/// Builds a PTB that settles a finalized on chain Tool result for one [RuntimeVertex].
///
/// The exact Invocation is settled before the optional owning Task settlement.
#[allow(clippy::too_many_arguments)]
pub fn settle_onchain_tool_result_for_walk_for_self_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    invocation: &sui::types::ObjectReference,
    result: &sui::types::ObjectReference,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    tool_witness_id: sui::types::Address,
    task_settlement: Option<&sui::types::ObjectReference>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.immutable_object(dag)?;
        let execution = tx.shared_object(execution, true)?;
        let tool_registry = tx.shared_root(&objects.tool_registry, false)?;
        let result = tx.shared_object(result, true)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let priority_fee_vault = tx.shared_root(&objects.priority_fee_vault, false)?;
        let clock = tx.clock()?;

        settle_onchain_tool_result_for_walk(
            tx,
            dag,
            execution,
            tool_registry,
            result,
            leader_registry,
            priority_fee_vault,
            walk_index,
            expected_vertex,
            tool_witness_id,
            clock,
        )?;

        super::invocation::settle(tx, dag, execution, expected_vertex, invocation)?;
        if let Some(task) = task_settlement {
            scheduler::append_settle_occurrence(tx, task, execution, clock)?;
        }

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn cleanup_broken_onchain_tool_result(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    result: sui::types::Argument,
    leader_registry: sui::types::Argument,
    walk_index: u64,
    tool_witness_id: sui::types::Address,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let tool_witness_id = tx.object_id(tool_witness_id)?;
    let runtime_authority = tx.runtime_authority(false)?;

    tx.call_target(
        execution_settlement_binding::cleanup_broken_onchain_tool_result_target,
        vec![
            runtime_authority,
            dag,
            execution,
            tool_registry,
            result,
            leader_registry,
            walk_index,
            tool_witness_id,
            clock,
        ],
    )?;

    Ok(())
}

/// Build a PTB that settles a committed tool result with leader gas accounting.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_committed_tool_result_for_walk_by_leader_for_self_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    execution_owner: &sui::types::Owner,
    leader_cap: &sui::types::ObjectReference,
    leader_cap_owner: &sui::types::Owner,
    invocation: &sui::types::ObjectReference,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    failed_onchain_tool_reason: Option<Vec<u8>>,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.immutable_object(dag)?;
        let execution = tx.object_from_owner(execution, execution_owner, true)?;
        let tool_registry = tx.shared_root(&objects.tool_registry, false)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let priority_fee_vault = tx.shared_root(&objects.priority_fee_vault, false)?;
        let leader_cap = tx.object_from_owner(leader_cap, leader_cap_owner, false)?;
        let walk_index = tx.arg(&walk_index)?;
        let expected_vertex_arg = runtime_vertex_arg(tx, expected_vertex)?;
        let failed_onchain_tool_reason =
            tx.arg(&MoveOption::from_option(failed_onchain_tool_reason))?;
        let commit_tx_digest = tx.arg(&commit_tx_digest)?;
        let commit_gas_charge = tx.arg(&commit_gas_charge)?;
        let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;
        let clock = tx.clock()?;
        let runtime_authority = tx.runtime_authority(false)?;

        tx.call_target(
            execution_settlement_binding::settle_committed_tool_result_for_walk_by_leader_target,
            vec![
                runtime_authority,
                dag,
                execution,
                tool_registry,
                leader_registry,
                priority_fee_vault,
                leader_cap,
                walk_index,
                expected_vertex_arg,
                failed_onchain_tool_reason,
                commit_tx_digest,
                commit_gas_charge,
                settlement_gas_charge,
                clock,
            ],
        )?;
        super::invocation::settle(tx, dag, execution, expected_vertex, invocation)?;
        Ok(())
    })
}

/// Build a PTB that records leader gas accounting for a committed tool result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_committed_tool_result_gas_charge_by_leader_for_self_ptb(
    objects: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    execution_owner: &sui::types::Owner,
    leader_cap: &sui::types::ObjectReference,
    leader_cap_owner: &sui::types::Owner,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    failed_onchain_tool_reason: Option<Vec<u8>>,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.immutable_object(dag)?;
        let execution = tx.object_from_owner(execution, execution_owner, true)?;
        let leader_registry = tx.shared_root(&objects.leader_registry, false)?;
        let leader_cap = tx.object_from_owner(leader_cap, leader_cap_owner, false)?;
        let walk_index = tx.arg(&walk_index)?;
        let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
        let failed_onchain_tool_reason =
            tx.arg(&MoveOption::from_option(failed_onchain_tool_reason))?;
        let commit_tx_digest = tx.arg(&commit_tx_digest)?;
        let commit_gas_charge = tx.arg(&commit_gas_charge)?;
        let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;
        let clock = tx.clock()?;
        let runtime_authority = tx.runtime_authority(false)?;

        tx.call_target(
            execution_settlement_binding::record_committed_tool_result_gas_charge_by_leader_target,
            vec![
                runtime_authority,
                dag,
                execution,
                leader_registry,
                leader_cap,
                walk_index,
                expected_vertex,
                failed_onchain_tool_reason,
                commit_tx_digest,
                commit_gas_charge,
                settlement_gas_charge,
                clock,
            ],
        )?;
        Ok(())
    })
}

fn emit_payment_ready_walk_requests(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    leader_registry: sui::types::Argument,
    clock: sui::types::Argument,
) {
    let runtime_authority = tx
        .runtime_authority(false)
        .expect("the configured RuntimeAuthority root is a valid shared input");
    tx.call_target(
        execution_settlement_binding::emit_payment_ready_walk_requests_target,
        vec![runtime_authority, dag, execution, leader_registry, clock],
    )
    .expect("generated execution settlement target is valid");
}

/// PTB template for creating a new DAG default value.
fn create_default_value(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    default_value: &DagDefaultValue,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(&default_value.vertex)?;
    let port = tx.graph_input_port(&default_value.input_port)?;
    let value = default_value_nexus_data(default_value)?;
    let value = tx.nexus_data(&value)?;

    tx.call_target(
        dag_binding::with_default_value_target,
        vec![dag, vertex, port, value],
    )
}

fn default_value_nexus_data(default_value: &DagDefaultValue) -> anyhow::Result<NexusData> {
    NexusData::from_json_value(&default_value.value)
}

/// PTB template for creating a new DAG edge.
fn create_edge(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    edge: &DagEdge,
) -> anyhow::Result<sui::types::Argument> {
    let from_vertex = tx.graph_vertex(&edge.from.vertex)?;
    let from_variant = tx.graph_output_variant(&edge.from.output_variant)?;
    let from_port = tx.graph_output_port(&edge.from.output_port)?;
    let to_vertex = tx.graph_vertex(&edge.to.vertex)?;
    let to_port = tx.graph_input_port(&edge.to.input_port)?;
    let kind = tx.graph_edge_kind(&edge.kind)?;

    tx.call_target(
        dag_binding::with_edge_target,
        vec![
            dag,
            from_vertex,
            from_variant,
            from_port,
            to_vertex,
            to_port,
            kind,
        ],
    )
}

/// PTB template for creating a new DAG edge.
fn create_output(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    output: &DagOutput,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(&output.vertex)?;
    let variant = tx.graph_output_variant(&output.output_variant)?;
    let port = tx.graph_output_port(&output.output_port)?;

    tx.call_target(
        dag_binding::with_output_target,
        vec![dag, vertex, variant, port],
    )
}

/// PTB template for marking a vertex as an entry vertex.
fn mark_entry_vertex(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    vertex: &str,
    entry_group: &str,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(vertex)?;
    let entry_group = tx.graph_entry_group(entry_group)?;

    tx.call_target(
        dag_binding::with_entry_in_group_target,
        vec![dag, vertex, entry_group],
    )
}

/// PTB template for marking an input port as an input port.
fn mark_entry_input_port(
    tx: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    vertex: &str,
    entry_port: &DagEntryPort,
    entry_group: &str,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(vertex)?;
    let entry_port = tx.graph_input_port(&entry_port.name)?;
    let entry_group = tx.graph_entry_group(entry_group)?;

    tx.call_target(
        dag_binding::with_entry_port_in_group_target,
        vec![dag, vertex, entry_port, entry_group],
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                interface::{
                    meta_schema::{OutputVariantSchema, PortSchema, ValueKind},
                    verifier::VerifierMethodId,
                },
                move_std::ascii,
                sui_framework::object::ID,
            },
            test_utils::sui_mocks,
            types::{OffchainToolOutputPort, PackageRole, SharedRoot},
        },
        std::sync::Arc,
        sui::types::{Argument, Command, Input},
    };

    fn addr(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn object_ref(value: &'static str, version: u64, digest: u8) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(
            addr(value),
            version,
            sui::types::Digest::from([digest; 32]),
        )
    }

    fn nexus_objects() -> NexusContext {
        sui_mocks::mock_nexus_context()
    }

    fn runtime_package(objects: &NexusContext) -> sui::types::Address {
        objects
            .require_package(PackageRole::Scheduler)
            .unwrap()
            .storage_id
    }

    fn move_call_index(
        ptb: &ProgrammableTransaction,
        package: Option<sui::types::Address>,
        module: &str,
        function: &str,
    ) -> usize {
        ptb.commands
            .iter()
            .position(|command| {
                let Command::MoveCall(call) = command else {
                    return false;
                };
                package.is_none_or(|package| call.package == package)
                    && call.module.as_str() == module
                    && call.function.as_str() == function
            })
            .unwrap_or_else(|| panic!("missing move call {module}::{function}"))
    }

    fn input_for_argument<'a>(
        ptb: &'a ProgrammableTransaction,
        argument: &sui::types::Argument,
    ) -> &'a sui::types::Input {
        let sui::types::Argument::Input(index) = argument else {
            panic!("expected input argument, got {argument:?}");
        };
        ptb.inputs
            .get(*index as usize)
            .unwrap_or_else(|| panic!("missing input at index {index}"))
    }

    fn expect_shared_object_arg(
        ptb: &ProgrammableTransaction,
        argument: &sui::types::Argument,
        expected: &sui::types::ObjectReference,
        expected_mutable: bool,
    ) {
        let sui::types::Input::Shared(shared) = input_for_argument(ptb, argument) else {
            panic!("expected shared object input, got {argument:?}");
        };
        assert_eq!(shared.object_id(), *expected.object_id());
        assert_eq!(shared.version(), expected.version());
        assert_eq!(shared.mutability().is_mutable(), expected_mutable);
    }

    fn expect_shared_root_arg(
        ptb: &ProgrammableTransaction,
        argument: &sui::types::Argument,
        expected: &SharedRoot,
        expected_mutable: bool,
    ) {
        let sui::types::Input::Shared(shared) = input_for_argument(ptb, argument) else {
            panic!("expected shared root input, got {argument:?}");
        };
        assert_eq!(shared.object_id(), expected.object_id());
        assert_eq!(shared.version(), expected.initial_shared_version);
        assert_eq!(shared.mutability().is_mutable(), expected_mutable);
    }

    fn expect_task_settlement_uses_clock(ptb: &ProgrammableTransaction) {
        let settle = move_call_index(ptb, None, "scheduler", "settle");
        let Command::MoveCall(call) = &ptb.commands[settle] else {
            panic!("expected task settlement call");
        };
        let Input::Shared(clock) = input_for_argument(ptb, &call.arguments[3]) else {
            panic!("expected Clock shared object argument");
        };
        assert_eq!(clock.object_id(), move_boundary::CLOCK_OBJECT_ID);
        assert!(!clock.mutability().is_mutable());
    }

    fn expect_u64_arg(
        ptb: &ProgrammableTransaction,
        argument: &sui::types::Argument,
        expected: u64,
    ) {
        let sui::types::Input::Pure(bytes) = input_for_argument(ptb, argument) else {
            panic!("expected pure u64 input, got {argument:?}");
        };
        let actual = bcs::from_bytes::<u64>(bytes).expect("u64 pure argument should decode");
        assert_eq!(actual, expected);
    }
    fn canonical_response() -> OffchainToolOutput {
        OffchainToolOutput {
            tag: b"ok".to_vec(),
            ports: vec![OffchainToolOutputPort {
                port_name: b"result".to_vec(),
                values: NexusData::inline_data(b"result")
                    .expect("fixture is bounded")
                    .into_values()
                    .expect("fixture should decode"),
            }],
        }
    }

    #[test]
    fn many_default_serializes_independent_json_values_at_ptb_boundary() {
        let default_value = DagDefaultValue {
            vertex: "root".to_owned(),
            input_port: "items".to_owned(),
            value: serde_json::json!({
                "many": [
                    { "kind": "data", "data": 1 },
                    { "kind": "data", "data": { "ordered": 2 } },
                ],
            }),
        };

        let value = default_value_nexus_data(&default_value).unwrap();
        assert!(value.is_many(), "default should retain Many cardinality");
        let values = value.values().expect("default should decode");
        assert!(matches!(
            values.as_slice(),
            [
                crate::move_bindings::primitives::data::NexusValue::InlineData { bytes: first },
                crate::move_bindings::primitives::data::NexusValue::InlineData { bytes: second },
            ] if first == b"1" && second == br#"{"ordered":2}"#
        ));
    }

    fn offchain_meta_schema() -> MetaSchema {
        MetaSchema::new(
            vec![],
            vec![OutputVariantSchema::new(
                b"ok".to_vec(),
                vec![PortSchema::new(b"result".to_vec(), false, ValueKind::Data)],
            )],
        )
    }

    fn offchain_ptb(
        objects: &NexusContext,
        submission: &PreparedOffchainToolResultSubmission,
    ) -> ProgrammableTransaction {
        submit_off_chain_tool_result_for_walk_ptb(
            objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            0,
            &RuntimeVertex::plain("offchain"),
            submission,
        )
        .unwrap()
    }

    fn move_calls(ptb: &ProgrammableTransaction) -> Vec<&sui::types::MoveCall> {
        ptb.commands
            .iter()
            .filter_map(|command| match command {
                Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect()
    }

    fn input_shared_object_id(
        ptb: &ProgrammableTransaction,
        argument: Argument,
    ) -> sui::types::Address {
        let Argument::Input(index) = argument else {
            panic!("expected input argument")
        };
        let Input::Shared(shared) = &ptb.inputs[usize::from(index)] else {
            panic!("expected shared-object input")
        };
        shared.object_id()
    }

    #[test]
    fn malformed_named_output_is_rejected_before_ptb_projection() {
        let mut result = canonical_response();
        result.ports[0].port_name = b"renamed".to_vec();
        let mut tx = move_boundary::NexusPtbBuilder::new(Arc::new(nexus_objects()));

        let error = prepare_offchain_tool_response(&mut tx, &result, &offchain_meta_schema())
            .expect_err("renamed producer port must fail before projection");
        let ptb = tx.finish();

        assert!(error.to_string().contains("does not conform to MetaSchema"));
        assert!(ptb.inputs.is_empty());
        assert!(ptb.commands.is_empty());
    }

    #[test]
    fn offchain_none_creates_verdict_before_unified_submission() {
        let objects = nexus_objects();
        let ptb = offchain_ptb(
            &objects,
            &PreparedOffchainToolResultSubmission::NoVerifier {
                result: canonical_response(),
                meta_schema: offchain_meta_schema(),
            },
        );
        let typed_output = move_call_index(&ptb, None, "tagged_output", "with_named_payload");
        let worksheet = move_call_index(
            &ptb,
            None,
            "execution_submission",
            "prepare_tool_result_submission_worksheet",
        );
        let verify = move_call_index(&ptb, None, "verifier", "new_none");
        let submit = move_call_index(
            &ptb,
            None,
            "execution_submission",
            "commit_off_chain_tool_result_for_walk",
        );
        assert!(worksheet < typed_output && typed_output < verify && verify < submit);
        let Command::MoveCall(worksheet_call) = &ptb.commands[worksheet] else {
            unreachable!()
        };
        assert_eq!(worksheet_call.arguments.len(), 10);
        expect_shared_root_arg(
            &ptb,
            &worksheet_call.arguments[0],
            &objects.runtime_authority,
            false,
        );
        let Command::MoveCall(submit_call) = &ptb.commands[submit] else {
            unreachable!()
        };
        assert_eq!(submit_call.arguments.len(), 12);
        assert_eq!(
            submit_call.arguments[5],
            Argument::NestedResult(worksheet as u16, 0),
        );
        assert_eq!(
            submit_call.arguments[6],
            Argument::NestedResult(worksheet as u16, 1),
        );
    }

    #[test]
    fn offchain_registered_key_uses_current_auxiliary_and_unified_submission() {
        let objects = nexus_objects();
        let leader_cap = object_ref("0x20", 1, 20);
        let leader_key_binding = object_ref("0x70", 2, 70);
        let tool_key_binding = object_ref("0x71", 3, 71);
        let ptb = offchain_ptb(
            &objects,
            &PreparedOffchainToolResultSubmission::RegisteredKeyVerifier {
                tool_id: addr("0x42"),
                result: canonical_response(),
                meta_schema: offchain_meta_schema(),
                auxiliary: RegisteredKeyAuxiliary {
                    input_hash: vec![1; 32],
                    nonce: vec![4; 32],
                    leader_signature: vec![2; 64],
                    tool_signature: vec![3; 64],
                },
                bindings: OffchainVerifierKeyBindings {
                    leader_key_binding: leader_key_binding.clone(),
                    tool_key_binding: tool_key_binding.clone(),
                },
            },
        );
        let auxiliary = move_call_index(&ptb, None, "verifier", "registered_key_auxiliary");
        let verify = move_call_index(&ptb, None, "registered_key_verifier", "verify");
        let submit = move_call_index(
            &ptb,
            None,
            "execution_submission",
            "commit_off_chain_tool_result_for_walk",
        );
        assert!(auxiliary < verify && verify < submit);
        let Command::MoveCall(auxiliary_call) = &ptb.commands[auxiliary] else {
            unreachable!()
        };
        assert_eq!(auxiliary_call.arguments.len(), 4);
        let Command::MoveCall(verify_call) = &ptb.commands[verify] else {
            unreachable!()
        };
        assert_eq!(verify_call.arguments.len(), 9);
        expect_shared_root_arg(
            &ptb,
            &verify_call.arguments[3],
            &objects.leader_registry,
            false,
        );
        expect_shared_object_arg(&ptb, &verify_call.arguments[4], &leader_cap, false);
        expect_shared_root_arg(
            &ptb,
            &verify_call.arguments[5],
            &objects.network_auth,
            false,
        );
        expect_shared_object_arg(&ptb, &verify_call.arguments[6], &leader_key_binding, false);
        expect_shared_object_arg(&ptb, &verify_call.arguments[7], &tool_key_binding, false);
    }

    #[test]
    fn offchain_external_appends_immutable_objects_after_fixed_arguments() {
        let objects = nexus_objects();
        let verifier_package = addr("0x40");
        let witness = object_ref("0x70", 2, 70);
        let config = object_ref("0x71", 3, 71);
        let ptb = offchain_ptb(
            &objects,
            &PreparedOffchainToolResultSubmission::ExternalVerifier {
                result: canonical_response(),
                meta_schema: offchain_meta_schema(),
                auxiliary: vec![9],
                runtime_call: ExternalVerifierRuntimeCall {
                    method_id: VerifierMethodId {
                        tool_id: ID::new(addr("0x42")),
                        package_id: ID::new(verifier_package),
                        module_name: ascii::String::from("verifier"),
                        function_name: ascii::String::from("verify"),
                    },
                    witness_id: *witness.object_id(),
                    immutable_shared_objects: vec![witness, config],
                },
            },
        );
        let calls = move_calls(&ptb);
        let verify = calls
            .iter()
            .find(|call| call.package == verifier_package)
            .expect("external verifier call");
        assert_eq!(verify.module.as_str(), "verifier");
        assert_eq!(verify.function.as_str(), "verify");
        assert_eq!(verify.arguments.len(), 5);
        assert_eq!(
            input_shared_object_id(&ptb, verify.arguments[3]),
            addr("0x70")
        );
        assert_eq!(
            input_shared_object_id(&ptb, verify.arguments[4]),
            addr("0x71")
        );
        let submit = move_call_index(
            &ptb,
            None,
            "execution_submission",
            "commit_off_chain_tool_result_for_walk",
        );
        let external = ptb
            .commands
            .iter()
            .position(|command| {
                matches!(command, Command::MoveCall(call) if call.package == verifier_package)
            })
            .unwrap();
        assert!(external < submit);
    }

    #[test]
    fn onchain_tool_execution_keeps_invocation_locked_until_outcome() {
        let objects = nexus_objects();
        let next_vertex = RuntimeVertex::plain("counter_increment");
        let tool_package = addr("0x40");
        let submission =
            PreparedOnchainToolResultSubmission::Execute(PreparedOnchainToolExecution {
                package: tool_package,
                module: "tool".to_string(),
                tool_witness_id: addr("0x41"),
                requires_authorization_cap: false,
                arguments: vec![],
            });

        let ptb = submit_on_chain_tool_result_for_walk_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            0,
            &next_vertex,
            &submission,
        )
        .unwrap();

        let execute = move_call_index(&ptb, Some(tool_package), "tool", "execute");

        assert!(!ptb.commands.iter().any(|command| {
            matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == "invocation_adapter"
                        && call.function.as_str() == "settle"
            )
        }));
        assert_eq!(execute, ptb.commands.len() - 1);
    }

    #[test]
    fn onchain_tool_execution_promotes_preallocated_shared_input_to_mutable() {
        let objects = nexus_objects();
        let next_vertex = RuntimeVertex::plain("mutate_agent_registry");
        let tool_package = addr("0x40");
        let submission =
            PreparedOnchainToolResultSubmission::Execute(PreparedOnchainToolExecution {
                package: tool_package,
                module: "tool".to_string(),
                tool_witness_id: addr("0x41"),
                requires_authorization_cap: false,
                arguments: vec![OnchainToolArgument::SharedObject {
                    object_id: objects.agent_registry.object_id(),
                    initial_shared_version: objects.agent_registry.initial_shared_version,
                    mutable: true,
                }],
            });

        let ptb = submit_on_chain_tool_result_for_walk_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            0,
            &next_vertex,
            &submission,
        )
        .unwrap();

        let worksheet = &move_calls(&ptb)
            .into_iter()
            .find(|call| call.function.as_str() == "prepare_tool_result_submission_worksheet")
            .expect("worksheet call")
            .arguments;
        let execute = &move_calls(&ptb)
            .into_iter()
            .find(|call| call.package == tool_package)
            .expect("dynamic Tool call")
            .arguments;

        assert_eq!(execute[2], worksheet[2]);
        expect_shared_root_arg(&ptb, &execute[2], &objects.agent_registry, true);
        assert_eq!(
            ptb.inputs
                .iter()
                .filter(|input| {
                    matches!(
                        input,
                        Input::Shared(shared)
                            if shared.object_id() == objects.agent_registry.object_id()
                    )
                })
                .count(),
            1
        );
    }

    #[test]
    fn onchain_submission_threads_prepared_stamp_through_release_and_result_creation() {
        let objects = nexus_objects();
        let next_vertex = RuntimeVertex::plain("counter_increment");
        let submission =
            PreparedOnchainToolResultSubmission::Execute(PreparedOnchainToolExecution {
                package: addr("0x40"),
                module: "tool".to_string(),
                tool_witness_id: addr("0x41"),
                requires_authorization_cap: true,
                arguments: vec![],
            });

        let ptb = submit_on_chain_tool_result_for_walk_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            0,
            &next_vertex,
            &submission,
        )
        .unwrap();

        let worksheet = move_call_index(
            &ptb,
            None,
            "execution_submission",
            "prepare_tool_result_submission_worksheet",
        );
        let release = move_call_index(
            &ptb,
            None,
            "execution_submission",
            "release_vertex_authorization_for_onchain_walk",
        );
        let create = move_call_index(
            &ptb,
            None,
            "execution_submission",
            "create_on_chain_tool_result_for_walk",
        );
        let Command::MoveCall(release_call) = &ptb.commands[release] else {
            panic!("expected authorization release call");
        };
        let Command::MoveCall(create_call) = &ptb.commands[create] else {
            panic!("expected on-chain result creation call");
        };

        assert_eq!(
            release_call.arguments[3],
            Argument::NestedResult(worksheet as u16, 0),
        );
        assert_eq!(
            release_call.arguments[4],
            Argument::NestedResult(worksheet as u16, 1),
        );
        assert_eq!(
            create_call.arguments[4],
            Argument::NestedResult(worksheet as u16, 0),
        );
        assert_eq!(
            create_call.arguments[5],
            Argument::NestedResult(worksheet as u16, 1),
        );
    }

    #[test]
    fn consume_on_chain_tool_result_uses_priority_fee_vault_argument() {
        let objects = nexus_objects();
        let next_vertex = RuntimeVertex::plain("counter_increment");

        let ptb = consume_on_chain_tool_result_for_walk_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            &object_ref("0x30", 9, 30),
            9,
            &next_vertex,
            (addr("0x70"), 10),
            addr("0x71"),
            123,
            45,
            None,
        )
        .expect("ptb should build");

        let call_index = move_call_index(
            &ptb,
            Some(runtime_package(&objects)),
            "execution_submission",
            "consume_on_chain_tool_result_for_walk",
        );
        let Command::MoveCall(call) = &ptb.commands[call_index] else {
            panic!("expected consume call");
        };

        assert_eq!(call.arguments.len(), 14);
        expect_shared_root_arg(&ptb, &call.arguments[0], &objects.runtime_authority, false);
        expect_shared_root_arg(&ptb, &call.arguments[6], &objects.leader_registry, false);
        expect_shared_root_arg(&ptb, &call.arguments[7], &objects.priority_fee_vault, false);
        expect_u64_arg(&ptb, &call.arguments[8], 9);
        expect_u64_arg(&ptb, &call.arguments[11], 123);
        expect_u64_arg(&ptb, &call.arguments[12], 45);
        assert!(move_call_index(&ptb, None, "invocation_adapter", "settle") > call_index);
    }

    #[test]
    fn settle_committed_tool_result_by_leader_uses_priority_fee_vault_argument() {
        let objects = nexus_objects();

        let ptb = settle_committed_tool_result_for_walk_by_leader_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            &object_ref("0x30", 9, 30),
            11,
            &RuntimeVertex::plain(""),
            None,
            vec![9, 8, 7],
            123,
            45,
            None,
        )
        .expect("ptb should build");

        let call_index = move_call_index(
            &ptb,
            Some(runtime_package(&objects)),
            "execution_settlement",
            "settle_committed_tool_result_for_walk_by_leader",
        );
        let Command::MoveCall(call) = &ptb.commands[call_index] else {
            panic!("expected settlement call");
        };

        assert_eq!(call.arguments.len(), 14);
        expect_shared_root_arg(&ptb, &call.arguments[0], &objects.runtime_authority, false);
        expect_shared_root_arg(&ptb, &call.arguments[4], &objects.leader_registry, false);
        expect_shared_root_arg(&ptb, &call.arguments[5], &objects.priority_fee_vault, false);
        expect_u64_arg(&ptb, &call.arguments[7], 11);
        expect_u64_arg(&ptb, &call.arguments[11], 123);
        expect_u64_arg(&ptb, &call.arguments[12], 45);
        assert!(move_call_index(&ptb, None, "invocation_adapter", "settle") > call_index);
    }

    #[test]
    fn failed_onchain_tool_payment_record_does_not_settle_invocation() {
        let objects = nexus_objects();
        let expected_vertex = RuntimeVertex::with_iterator("counter_increment", 2, 3);
        let ptb = record_committed_tool_result_gas_charge_by_leader_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            11,
            &expected_vertex,
            Some(b"tool failed".to_vec()),
            vec![9, 8, 7],
            123,
            45,
        )
        .expect("failed on-chain result record PTB should build");

        let call_index = move_call_index(
            &ptb,
            Some(runtime_package(&objects)),
            "execution_settlement",
            "record_committed_tool_result_gas_charge_by_leader",
        );
        let Command::MoveCall(call) = &ptb.commands[call_index] else {
            panic!("expected failed on-chain result record call");
        };

        assert_eq!(call.arguments.len(), 12);
        expect_shared_root_arg(&ptb, &call.arguments[0], &objects.runtime_authority, false);
        expect_shared_root_arg(&ptb, &call.arguments[3], &objects.leader_registry, false);
        expect_u64_arg(&ptb, &call.arguments[5], 11);
        expect_u64_arg(&ptb, &call.arguments[9], 123);
        expect_u64_arg(&ptb, &call.arguments[10], 45);
        assert!(!ptb.commands.iter().any(|command| {
            matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == "invocation_adapter"
                        && call.function.as_str() == "settle"
            )
        }));
        assert!(!ptb.commands.iter().any(|command| {
            matches!(
                command,
                Command::MoveCall(call)
                    if call.module.as_str() == "execution_settlement"
                        && call.function.as_str() == "emit_payment_ready_walk_requests"
            )
        }));
    }

    #[test]
    fn failed_onchain_tool_secondary_settles_exact_invocation() {
        let objects = nexus_objects();
        let expected_vertex = RuntimeVertex::with_iterator("counter_increment", 2, 3);
        let ptb = settle_committed_tool_result_for_walk_by_leader_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            (addr("0x60"), 8),
            &object_ref("0x20", 1, 20),
            &object_ref("0x30", 9, 30),
            11,
            &expected_vertex,
            Some(b"tool failed".to_vec()),
            vec![9, 8, 7],
            123,
            45,
            None,
        )
        .expect("failed on-chain secondary settlement PTB should build");

        let call_index = move_call_index(
            &ptb,
            Some(runtime_package(&objects)),
            "execution_settlement",
            "settle_committed_tool_result_for_walk_by_leader",
        );
        let Command::MoveCall(call) = &ptb.commands[call_index] else {
            panic!("expected failed on-chain settlement call");
        };

        assert_eq!(call.arguments.len(), 14);
        expect_shared_root_arg(&ptb, &call.arguments[0], &objects.runtime_authority, false);
        expect_u64_arg(&ptb, &call.arguments[7], 11);
        expect_u64_arg(&ptb, &call.arguments[11], 123);
        expect_u64_arg(&ptb, &call.arguments[12], 45);
        assert!(move_call_index(&ptb, None, "invocation_adapter", "settle") > call_index);
        assert!(
            move_call_index(
                &ptb,
                Some(runtime_package(&objects)),
                "execution_settlement",
                "emit_payment_ready_walk_requests",
            ) > call_index
        );
    }

    #[test]
    fn permissionless_settle_committed_tool_result_uses_priority_fee_vault_argument() {
        let objects = nexus_objects();
        let task = object_ref("0x80", 10, 80);

        let ptb = settle_committed_tool_result_for_walk_for_self_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            &object_ref("0x60", 8, 60),
            &object_ref("0x30", 9, 30),
            13,
            &RuntimeVertex::plain("counter_increment"),
            Some(&task),
        )
        .expect("ptb should build");

        let call_index = move_call_index(
            &ptb,
            Some(runtime_package(&objects)),
            "execution_settlement",
            "settle_committed_tool_result_for_walk",
        );
        let Command::MoveCall(call) = &ptb.commands[call_index] else {
            panic!("expected permissionless settlement call");
        };

        assert_eq!(call.arguments.len(), 8);
        expect_shared_root_arg(&ptb, &call.arguments[0], &objects.runtime_authority, false);
        expect_shared_root_arg(&ptb, &call.arguments[4], &objects.leader_registry, false);
        expect_shared_root_arg(&ptb, &call.arguments[5], &objects.priority_fee_vault, false);
        expect_u64_arg(&ptb, &call.arguments[6], 13);
        let invocation = move_call_index(&ptb, None, "invocation_adapter", "settle");
        let task = move_call_index(&ptb, None, "scheduler", "settle");
        assert!(call_index < invocation && invocation < task);
        expect_task_settlement_uses_clock(&ptb);
    }

    #[test]
    fn settle_onchain_tool_result_uses_priority_fee_vault_argument() {
        let objects = nexus_objects();
        let next_vertex = RuntimeVertex::plain("counter_increment");
        let task = object_ref("0x80", 10, 80);

        let ptb = settle_onchain_tool_result_for_walk_for_self_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            &object_ref("0x60", 8, 60),
            &object_ref("0x30", 9, 30),
            &object_ref("0x70", 9, 70),
            15,
            &next_vertex,
            addr("0x71"),
            Some(&task),
        )
        .expect("ptb should build");

        let call_index = move_call_index(
            &ptb,
            Some(runtime_package(&objects)),
            "execution_settlement",
            "settle_onchain_tool_result_for_walk",
        );
        let Command::MoveCall(call) = &ptb.commands[call_index] else {
            panic!("expected on-chain settlement call");
        };
        let invocation = move_call_index(&ptb, None, "invocation_adapter", "settle");
        let task = move_call_index(&ptb, None, "scheduler", "settle");
        assert!(call_index < invocation && invocation < task);
        expect_task_settlement_uses_clock(&ptb);

        assert_eq!(call.arguments.len(), 11);
        expect_shared_root_arg(&ptb, &call.arguments[0], &objects.runtime_authority, false);
        expect_shared_root_arg(&ptb, &call.arguments[5], &objects.leader_registry, false);
        expect_shared_root_arg(&ptb, &call.arguments[6], &objects.priority_fee_vault, false);
        expect_u64_arg(&ptb, &call.arguments[7], 15);
    }

    #[test]
    fn permissionless_abort_settles_the_task_after_the_execution() {
        let objects = nexus_objects();
        let task = object_ref("0x80", 10, 80);
        let ptb = abort_expired_execution_for_self_ptb(
            &objects,
            &object_ref("0x50", 7, 50),
            &object_ref("0x60", 8, 60),
            &[],
            Some(&task),
        )
        .expect("ptb should build");

        let abort = move_call_index(
            &ptb,
            None,
            "execution_settlement",
            "abort_expired_execution",
        );
        let task = move_call_index(&ptb, None, "scheduler", "settle");
        assert!(abort < task);
        expect_task_settlement_uses_clock(&ptb);
    }

    #[test]
    fn publish_dispatches_edges_to_direct_move_api() {
        let edge = DagEdge {
            from: DagOutput {
                vertex: "producer".to_string(),
                output_variant: "ok".to_string(),
                output_port: "result".to_string(),
            },
            to: crate::types::DagInput {
                vertex: "consumer".to_string(),
                input_port: "items".to_string(),
            },
            kind: graph_binding::EdgeKind::Normal,
        };
        let dag = DagSpec {
            edges: vec![edge],
            ..Default::default()
        };

        let ptb = publish_ptb(&nexus_objects(), dag).unwrap();
        move_call_index(&ptb, None, "dag", "with_edge");
    }

    #[test]
    fn publish_offchain_vertex_binds_registry_and_selected_verifier_mode() {
        let dag = DagSpec {
            vertices: vec![DagVertex {
                kind: DagVertexKind::OffChain {
                    tool_fqn: crate::fqn!("xyz.taluslabs.demo@1"),
                },
                name: "demo".to_string(),
                entry_ports: vec![],
                post_failure_action: None,
                verifier: Some(ToolVerifierMode::RegisteredKey),
            }],
            ..Default::default()
        };

        let ptb = publish_ptb(&nexus_objects(), dag).unwrap();
        let new_dag = move_call_index(&ptb, None, "dag", "new");
        let add_vertex = move_call_index(&ptb, None, "tool_registry", "add_vertex_to_dag");
        let mode = move_call_index(&ptb, None, "verifier", "verifier_mode_registered_key");
        let configure = move_call_index(
            &ptb,
            None,
            "tool_registry",
            "set_registered_vertex_verifier_mode",
        );
        let finalize_dag = move_call_index(&ptb, None, "dag", "finalize");
        let Command::MoveCall(add_vertex_call) = &ptb.commands[add_vertex] else {
            panic!("expected add vertex call");
        };
        let Command::MoveCall(new_dag_call) = &ptb.commands[new_dag] else {
            panic!("expected DAG constructor call");
        };
        assert_eq!(new_dag_call.arguments.len(), 0);
        assert_eq!(add_vertex_call.arguments.len(), 5);
        assert_eq!(
            add_vertex_call.arguments[1],
            Argument::NestedResult(new_dag as u16, 0)
        );
        assert_eq!(
            add_vertex_call.arguments[2],
            Argument::NestedResult(new_dag as u16, 1)
        );
        let Command::MoveCall(configure_call) = &ptb.commands[configure] else {
            panic!("expected verifier configuration call");
        };
        assert_eq!(
            configure_call.arguments[1],
            Argument::NestedResult(new_dag as u16, 0)
        );
        assert_eq!(
            configure_call.arguments[2],
            Argument::NestedResult(new_dag as u16, 1)
        );
        let Command::MoveCall(finalize_dag_call) = &ptb.commands[finalize_dag] else {
            panic!("expected DAG finalization call");
        };
        assert_eq!(
            finalize_dag_call.arguments[0],
            Argument::NestedResult(new_dag as u16, 0)
        );
        assert_eq!(
            finalize_dag_call.arguments[1],
            Argument::NestedResult(new_dag as u16, 1)
        );
        assert!(!ptb.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "dag" && call.function.as_str() == "with_vertex"
        )));
        assert!(!ptb.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "tool_registry"
                    && call.function.as_str() == "with_registered_vertices"
        )));
        assert!(!ptb.commands.iter().any(|command| matches!(
            command,
            Command::MoveCall(call)
                if call.module.as_str() == "tool_registry"
                    && call.function.as_str() == "with_registered_vertex"
        )));
        assert!(add_vertex < mode);
        assert!(mode < configure);
    }

    #[test]
    fn publish_rejects_verifier_mode_on_onchain_vertex() {
        let dag = DagSpec {
            vertices: vec![DagVertex {
                kind: DagVertexKind::OnChain {
                    tool_fqn: crate::fqn!("xyz.taluslabs.onchain@1"),
                },
                name: "onchain".to_string(),
                entry_ports: vec![],
                post_failure_action: None,
                verifier: Some(ToolVerifierMode::External),
            }],
            ..Default::default()
        };

        assert!(publish_ptb(&nexus_objects(), dag)
            .unwrap_err()
            .to_string()
            .contains("cannot configure an off-chain verifier"));
    }

    #[test]
    fn active_submission_binding_ir_uses_typed_stamp_without_raw_commitment() {
        let ir: serde_json::Value =
            serde_json::from_str(include_str!("../move_bindings/ir/workflow.json")).unwrap();
        let execution_functions = ir["modules"]["execution"]["functions"].as_array().unwrap();
        assert!(execution_functions
            .iter()
            .all(|function| function["name"] != "prove_vertex_authorization_for_recipient"));
        let functions = ir["modules"]["execution_submission"]["functions"]
            .as_array()
            .unwrap();

        for function_name in [
            "commit_off_chain_tool_result_for_walk",
            "release_vertex_authorization_for_onchain_walk",
            "create_on_chain_tool_result_for_walk",
        ] {
            let function = functions
                .iter()
                .find(|function| function["name"] == function_name)
                .unwrap();
            let parameters = function["parameters"].as_array().unwrap();
            assert!(parameters
                .iter()
                .all(|parameter| parameter["name"] != "input_commitment"));
            let stamp = parameters
                .iter()
                .find(|parameter| parameter["name"] == "stamp")
                .expect("active submission target must carry the typed stamp");
            assert!(stamp["ty"]
                .to_string()
                .contains("AgentVertexAuthorizationStamp"));
        }
    }

    #[test]
    fn onchain_result_ir_reuses_published_layout_without_persisted_witnesses() {
        let interface: serde_json::Value =
            serde_json::from_str(include_str!("../move_bindings/ir/interface.json")).unwrap();
        let datatypes = interface["modules"]["onchain_tool_result"]["datatypes"]
            .as_array()
            .unwrap();
        let result = datatypes
            .iter()
            .find(|datatype| datatype["name"] == "OnchainToolResult")
            .unwrap();
        let fields = result["kind"]["Struct"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(fields, ["id"]);
        let inner = datatypes
            .iter()
            .find(|datatype| datatype["name"] == "OnchainToolResultInnerV1")
            .unwrap();
        let inner_fields = inner["kind"]["Struct"]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            inner_fields,
            [
                "execution_id",
                "finalized",
                "stamps",
                "tag",
                "named_payload",
                "finalize_tx_digest",
                "finalize_recipient",
            ]
        );

        let workflow: serde_json::Value =
            serde_json::from_str(include_str!("../move_bindings/ir/workflow.json")).unwrap();
        let consume = workflow["modules"]["execution_submission"]["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|function| function["name"] == "consume_on_chain_tool_result_for_walk")
            .unwrap();
        let parameter_names = consume["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|parameter| parameter["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            parameter_names,
            [
                "_permit",
                "dag",
                "execution",
                "tool_registry",
                "result",
                "leader_cap",
                "leader_registry",
                "priority_fee_vault",
                "walk_index",
                "expected_vertex",
                "tool_witness_id",
                "commit_gas_charge",
                "settlement_gas_charge",
                "clock",
                "ctx",
            ]
        );
        assert_eq!(consume["type_parameters"].as_array().unwrap().len(), 1);

        let scheduler: serde_json::Value =
            serde_json::from_str(include_str!("../move_bindings/ir/scheduler.json")).unwrap();
        let facade = scheduler["modules"]["execution_submission"]["functions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|function| function["name"] == "consume_on_chain_tool_result_for_walk")
            .unwrap();
        let facade_parameter_names = facade["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|parameter| parameter["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(facade_parameter_names.first(), Some(&"authority"));
        assert!(facade["type_parameters"].as_array().unwrap().is_empty());
    }
}
