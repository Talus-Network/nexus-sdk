use {
    crate::{
        move_bindings::{
            interface::{
                authorization::AgentVertexAuthorizationTemplate,
                dag as dag_binding,
                graph::{
                    self as graph_binding,
                    PostFailureAction,
                    RuntimeVertex,
                    Vertex as GraphVertex,
                },
                verifier::{
                    self as verifier_binding,
                    ExternalVerifierSubmitEvidence,
                    FailureEvidenceKind,
                    OffChainToolResultAuxiliary,
                    OffChainVerifierProof,
                    OffchainResponseEvidence,
                    PreparedToolOutput,
                    PreparedToolOutputPort,
                    VerificationSubmissionKind,
                    VerifierConfig,
                    VerifierContractResult,
                    VerifierDecision,
                },
            },
            primitives::{
                data::{self as data_binding, NexusData},
                onchain_tool_result as onchain_tool_result_binding,
                tagged_output as tagged_output_binding,
            },
            registry::verifier_registry as verifier_registry_binding,
            sui_framework::transfer as transfer_binding,
            workflow::{
                execution as execution_binding,
                execution_entries as execution_entries_binding,
                execution_settlement as execution_settlement_binding,
                execution_submission as execution_submission_binding,
                gas as gas_binding,
            },
        },
        move_boundary,
        sui,
        transactions::{agent_input::AgentInput, scheduler, tap},
        types::{
            AgentId,
            AuthenticatedOffchainRequestEvidence,
            AuthenticatedOffchainVerifierEvidence,
            DagDefaultValue,
            DagEdge,
            DagEntryPort,
            DagOutput,
            DagSpec,
            DagVertex,
            DagVertexKind,
            ExternalVerifierRuntimeCall,
            NexusObjects,
            SkillId,
            DEFAULT_ENTRY_GROUP,
        },
    },
    std::collections::{HashMap, HashSet},
    sui::types::ProgrammableTransaction,
};

const TERMINAL_ERR_EVAL_VARIANT: &str = "_err_eval";
const TERMINAL_ERR_EVAL_REASON_PORT: &str = "reason";

fn vertex_kind_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    kind: &DagVertexKind,
) -> anyhow::Result<sui::types::Argument> {
    match kind {
        DagVertexKind::OffChain { tool_fqn } => {
            tx.graph_vertex_kind_off_chain(tool_fqn.to_string())
        }
        DagVertexKind::OnChain { tool_fqn } => tx.graph_vertex_kind_on_chain(tool_fqn.to_string()),
    }
}

fn runtime_vertex_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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

fn begin_execution_inputs_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
) -> anyhow::Result<sui::types::Argument> {
    let mut vertices = Vec::new();
    let mut ports = Vec::new();
    let mut values = Vec::new();

    for (vertex_name, data) in input_data {
        for (port_name, value) in data {
            vertices.push(tx.graph_vertex(vertex_name)?);
            ports.push(tx.graph_input_port(port_name)?);
            values.push(tx.nexus_data(value)?);
        }
    }

    let vertices = tx.move_vector::<crate::move_bindings::interface::graph::Vertex>(vertices)?;
    let ports = tx.move_vector::<crate::move_bindings::interface::graph::InputPort>(ports)?;
    let values = tx.move_vector::<NexusData>(values)?;

    tx.call_target(
        graph_binding::inputs_to_begin_execution_target,
        vec![vertices, ports, values],
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedOnchainToolOutput {
    pub output_variant: String,
    pub output_ports_data: HashMap<String, NexusData>,
}

impl PreparedOnchainToolOutput {
    pub fn terminal_err_eval(reason: NexusData) -> Self {
        Self {
            output_variant: TERMINAL_ERR_EVAL_VARIANT.to_string(),
            output_ports_data: HashMap::from([(TERMINAL_ERR_EVAL_REASON_PORT.to_string(), reason)]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeToolResultWorksheet {
    pub worksheet: sui::types::Argument,
    pub agent_registry: sui::types::Argument,
    pub dag: sui::types::Argument,
    pub execution: sui::types::Argument,
    pub clock: sui::types::Argument,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeToolResultWorksheetInputs {
    pub dag: (sui::types::Address, sui::types::Version),
    pub execution: (sui::types::Address, sui::types::Version),
    pub leader_registry: sui::types::Argument,
    pub leader_cap: sui::types::Argument,
    pub walk_index: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeToolGasRef {
    pub vertex: GraphVertex,
    pub object_id: sui::types::Address,
    pub version: sui::types::Version,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OffchainVerifierKeyBindings {
    pub leader_key_binding: sui::types::ObjectReference,
    pub tool_key_binding: sui::types::ObjectReference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreparedOffchainToolResultSubmission {
    NoVerifier {
        result: PreparedToolOutput,
        auxiliary: Option<OffChainToolResultAuxiliary>,
    },
    RegisteredKeyVerifier {
        result: PreparedToolOutput,
        auxiliary: Option<OffChainToolResultAuxiliary>,
        verifier_credential: Vec<u8>,
        communication_evidence: Vec<u8>,
        bindings: OffchainVerifierKeyBindings,
    },
    ExternalVerifier {
        result: PreparedToolOutput,
        verifier_evidence: AuthenticatedOffchainVerifierEvidence,
        runtime_call: ExternalVerifierRuntimeCall,
        communication_evidence: Vec<u8>,
        bindings: OffchainVerifierKeyBindings,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnchainToolArgument {
    PreallocatedObject {
        object_id: sui::types::Address,
    },
    ObjectId(sui::types::Address),
    Pure(Vec<u8>),
    SharedObject {
        object_id: sui::types::Address,
        initial_shared_version: sui::types::Version,
        mutable: bool,
    },
    ImmutableObject(sui::types::ObjectReference),
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
    TerminalErrEval {
        output: PreparedOnchainToolOutput,
        failure_evidence_kind: FailureEvidenceKind,
        submitted_failure_reason: Option<Vec<u8>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokenOnchainToolResultCleanupInput {
    pub walk_index: u64,
    pub result_ref: sui::types::ObjectReference,
    pub tool_witness_id: sui::types::Address,
}

fn build_runtime_tool_result_worksheet(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    agent_registry_ref: &sui::types::ObjectReference,
    inputs: RuntimeToolResultWorksheetInputs,
) -> anyhow::Result<RuntimeToolResultWorksheet> {
    let agent_registry = tx.shared_object(agent_registry_ref, false)?;
    let dag = tx.shared_object_by_id(inputs.dag.0, inputs.dag.1, false)?;
    let execution = tx.shared_object_by_id(inputs.execution.0, inputs.execution.1, true)?;
    let clock = tx.clock()?;
    let walk_index = tx.arg(&inputs.walk_index)?;
    let worksheet = tx.call_target(
        execution_submission_binding::prepare_tool_result_submission_worksheet_target,
        vec![
            dag,
            agent_registry,
            inputs.leader_registry,
            execution,
            inputs.leader_cap,
            walk_index,
            clock,
        ],
    )?;

    Ok(RuntimeToolResultWorksheet {
        worksheet,
        agent_registry,
        dag,
        execution,
        clock,
    })
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
pub(crate) fn empty(tx: &mut move_boundary::NexusPtbBuilder<'_>) -> sui::types::Argument {
    tx.call_target(dag_binding::new_target, vec![])
        .expect("generated dag::new target is valid")
}

/// PTB template to publish a DAG.
pub(crate) fn publish(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
) -> sui::types::Argument {
    tx.call_target(
        transfer_binding::public_share_object_target::<crate::move_bindings::interface::dag::DAG>,
        vec![dag],
    )
    .expect("generated transfer::public_share_object<DAG> target is valid")
}

/// PTB template to publish a full [`DagSpec`].
pub(crate) fn create(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    mut dag_arg: sui::types::Argument,
    dag: DagSpec,
) -> anyhow::Result<sui::types::Argument> {
    // Create all vertices.
    for vertex in &dag.vertices {
        dag_arg = create_vertex(tx, dag_arg, vertex)?;

        if let Some(action) = &vertex.post_failure_action {
            dag_arg = create_vertex_post_failure_action(tx, dag_arg, &vertex.name, action)?;
        }

        if let Some(verifier) = &vertex.leader_verifier {
            dag_arg = create_vertex_leader_verifier(tx, dag_arg, &vertex.name, verifier)?;
        }

        if let Some(verifier) = &vertex.tool_verifier {
            dag_arg = create_vertex_tool_verifier(tx, dag_arg, &vertex.name, verifier)?;
        }
    }

    if let Some(action) = &dag.post_failure_action {
        dag_arg = create_post_failure_action(tx, dag_arg, action)?;
    }

    if let Some(verifier) = &dag.leader_verifier {
        dag_arg = create_default_leader_verifier(tx, dag_arg, verifier)?;
    }

    if let Some(verifier) = &dag.tool_verifier {
        dag_arg = create_default_tool_verifier(tx, dag_arg, verifier)?;
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
    objects: &NexusObjects,
    dag: DagSpec,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let mut dag_arg = empty(tx);
        dag_arg = create(tx, dag_arg, dag)?;
        publish(tx, dag_arg);
        Ok(())
    })
}

fn verifier_registry_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    Ok(tx.shared_object(&objects.verifier_registry, false)?)
}

/// PTB template for creating a new DAG vertex.
fn create_vertex(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    vertex: &DagVertex,
) -> anyhow::Result<sui::types::Argument> {
    // `name: Vertex`
    let name = tx.graph_vertex(&vertex.name)?;

    // `kind: VertexKind`
    let kind = vertex_kind_arg(tx, &vertex.kind)?;

    // `dag.with_vertex(name, kind)`
    tx.call_target(dag_binding::with_vertex_target, vec![dag, name, kind])
}

/// PTB template for configuring a DAG-level default post-failure action.
fn create_post_failure_action(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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

fn create_default_leader_verifier(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let verifier_registry = verifier_registry_arg(tx)?;
    let verifier = tx.verifier_config(verifier)?;

    tx.call_target(
        verifier_registry_binding::with_default_leader_verifier_target,
        vec![verifier_registry, dag, verifier],
    )
}

fn create_default_tool_verifier(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let verifier_registry = verifier_registry_arg(tx)?;
    let verifier = tx.verifier_config(verifier)?;

    tx.call_target(
        verifier_registry_binding::with_default_tool_verifier_target,
        vec![verifier_registry, dag, verifier],
    )
}

fn create_vertex_leader_verifier(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    vertex: &str,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(vertex)?;
    let verifier_registry = verifier_registry_arg(tx)?;
    let verifier = tx.verifier_config(verifier)?;

    tx.call_target(
        verifier_registry_binding::with_vertex_leader_verifier_target,
        vec![verifier_registry, dag, vertex, verifier],
    )
}

fn create_vertex_tool_verifier(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    vertex: &str,
    verifier: &VerifierConfig,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(vertex)?;
    let verifier_registry = verifier_registry_arg(tx)?;
    let verifier = tx.verifier_config(verifier)?;

    tx.call_target(
        verifier_registry_binding::with_vertex_tool_verifier_target,
        vec![verifier_registry, dag, vertex, verifier],
    )
}

/// Build a PTB that accomplishes TAP execution payment.
pub(crate) fn accomplish_tap_execution_payment_for_self_ptb(
    objects: &NexusObjects,
    execution: &sui::types::ObjectReference,
    agent: Option<AgentInput>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let execution = tx.shared_object(execution, true)?;
        match agent {
            Some(agent) => {
                let agent = agent.mutable_ptb_argument(tx)?;
                tx.call_target(execution_settlement_binding::accomplish_tap_execution_payment_from_agent_vault_target, vec![agent, execution])?;
            }
            None => {
                tx.call_target(
                    execution_settlement_binding::accomplish_tap_execution_payment_target,
                    vec![execution],
                )?;
            }
        }
        Ok(())
    })
}

/// Build a PTB that refills TAP execution payment from the transaction gas coin.
pub(crate) fn refill_tap_execution_payment_for_self_ptb(
    objects: &NexusObjects,
    execution: &sui::types::ObjectReference,
    amount: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let execution = tx.shared_object(execution, true)?;
        let amount = tx.arg(&amount)?;
        let gas = tx.gas();
        let coin = tx.split_coins(gas, vec![amount])?;
        tx.call_target(
            execution_settlement_binding::refill_tap_execution_payment_target,
            vec![execution, coin],
        )?;
        Ok(())
    })
}

struct OffchainVerifierPtbObjects {
    verifier_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
    network_auth: sui::types::Argument,
    leader_key_binding: sui::types::Argument,
    tool_key_binding: sui::types::Argument,
}

fn offchain_verifier_ptb_objects(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    leader_registry: sui::types::Argument,
    bindings: &OffchainVerifierKeyBindings,
) -> anyhow::Result<OffchainVerifierPtbObjects> {
    let objects = tx.objects();
    let verifier_registry = tx.shared_object(&objects.verifier_registry, false)?;
    let network_auth = tx.shared_object(&objects.network_auth, false)?;
    let leader_key_binding = tx.shared_object(&bindings.leader_key_binding, false)?;
    let tool_key_binding = if bindings.tool_key_binding == bindings.leader_key_binding {
        leader_key_binding
    } else {
        tx.shared_object(&bindings.tool_key_binding, false)?
    };

    Ok(OffchainVerifierPtbObjects {
        verifier_registry,
        leader_registry,
        network_auth,
        leader_key_binding,
        tool_key_binding,
    })
}

fn ordered_runtime_tool_gas_refs(tools_gas: &[RuntimeToolGasRef]) -> Vec<&RuntimeToolGasRef> {
    let mut ordered = tools_gas.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.object_id
            .to_string()
            .cmp(&right.object_id.to_string())
            .then_with(|| left.version.cmp(&right.version))
            .then_with(|| left.vertex.name.as_str().cmp(right.vertex.name.as_str()))
    });
    ordered
}

fn runtime_tool_gas_args(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tools_gas: &[RuntimeToolGasRef],
    next_vertex: &RuntimeVertex,
    settle_current_vertex_gas: bool,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    expected_vertex_arg: Option<sui::types::Argument>,
) -> anyhow::Result<Vec<sui::types::Argument>> {
    let mut tool_gas_args = HashMap::new();
    let mut lock_tool_gas_args = Vec::new();
    let mut current_tool_gas = None;

    for gas_ref in ordered_runtime_tool_gas_refs(tools_gas) {
        let key = (gas_ref.object_id, gas_ref.version);
        if !tool_gas_args.contains_key(&key) {
            let arg = tx.shared_object_by_id(gas_ref.object_id, gas_ref.version, true)?;
            tool_gas_args.insert(key, arg);
            lock_tool_gas_args.push(arg);
        }

        let tool_gas = *tool_gas_args
            .get(&key)
            .expect("tool gas argument was just inserted");

        if &gas_ref.vertex == next_vertex.vertex() {
            current_tool_gas = Some(tool_gas);
        }
    }

    if settle_current_vertex_gas {
        if let Some(tool_gas) = current_tool_gas {
            let expected_vertex = match expected_vertex_arg {
                Some(expected_vertex) => expected_vertex,
                None => runtime_vertex_arg(tx, next_vertex)?,
            };
            crate::transactions::gas::settle_payment_state_for_vertex(
                tx,
                tool_gas,
                dag,
                execution,
                expected_vertex,
            )?;
        }
    }

    Ok(lock_tool_gas_args)
}

fn prepare_onchain_tool_argument(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    argument: &OnchainToolArgument,
    pre_allocated: &HashMap<sui::types::Address, sui::types::Argument>,
) -> anyhow::Result<sui::types::Argument> {
    match argument {
        OnchainToolArgument::PreallocatedObject { object_id } => {
            pre_allocated.get(object_id).copied().ok_or_else(|| {
                anyhow::anyhow!(
                    "Required pre-allocated object '{object_id}' is not available in this PTB"
                )
            })
        }
        OnchainToolArgument::ObjectId(object_id) => Ok(tx.object_id(*object_id)?),
        OnchainToolArgument::Pure(bytes) => Ok(tx.pure_bcs(bytes.clone())?),
        OnchainToolArgument::SharedObject {
            object_id,
            initial_shared_version,
            mutable,
        } => {
            if let Some(existing_arg) = pre_allocated.get(object_id).copied() {
                Ok(existing_arg)
            } else {
                Ok(tx.shared_object_by_id(*object_id, *initial_shared_version, *mutable)?)
            }
        }
        OnchainToolArgument::ImmutableObject(object) => Ok(tx.owned_object(object)?),
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
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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
    objects: &NexusObjects,
    dag_ref: (sui::types::Address, sui::types::Version),
    execution_ref: (sui::types::Address, sui::types::Version),
    agent_registry: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    clock: sui::types::Argument,
    tool_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
) -> HashMap<sui::types::Address, sui::types::Argument> {
    HashMap::from([
        (dag_ref.0, dag),
        (move_boundary::CLOCK_OBJECT_ID, clock),
        (execution_ref.0, execution),
        (*objects.tool_registry.object_id(), tool_registry),
        (*objects.leader_registry.object_id(), leader_registry),
        (*objects.agent_registry.object_id(), agent_registry),
    ])
}

#[allow(clippy::too_many_arguments)]
fn commit_prepared_onchain_tool_execution(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    execution_plan: &PreparedOnchainToolExecution,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    leader_registry: sui::types::Argument,
    walk_index: u64,
    expected_vertex: sui::types::Argument,
    pre_allocated: &HashMap<sui::types::Address, sui::types::Argument>,
) -> anyhow::Result<()> {
    let result = create_on_chain_tool_result_for_walk(
        tx,
        dag,
        execution,
        worksheet,
        leader_cap,
        leader_registry,
        walk_index,
        expected_vertex,
    )?;
    let user_args = prepare_onchain_tool_arguments(tx, &execution_plan.arguments, pre_allocated)?;
    let mut tool_args = if execution_plan.requires_authorization_cap {
        let authorization = release_vertex_authorization_for_onchain_walk(
            tx, dag, execution, worksheet, leader_cap, walk_index,
        )?;
        vec![authorization, worksheet, result]
    } else {
        vec![worksheet, result]
    };
    tool_args.extend(user_args);

    tx.call_function(
        execution_plan.package,
        execution_plan.module.as_str(),
        "execute",
        tool_args,
    )?;
    let _ = tool_registry;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn submit_off_chain_tool_result_for_walk_ptb(
    objects: &NexusObjects,
    dag: (sui::types::Address, sui::types::Version),
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    tools_gas: &[RuntimeToolGasRef],
    walk_index: u64,
    next_vertex: &RuntimeVertex,
    settle_current_vertex_gas: bool,
    submission: &PreparedOffchainToolResultSubmission,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let RuntimeToolResultWorksheet {
            worksheet,
            agent_registry: _,
            dag,
            execution,
            clock: _,
        } = build_runtime_tool_result_worksheet(
            tx,
            &objects.agent_registry,
            RuntimeToolResultWorksheetInputs {
                dag,
                execution,
                leader_registry,
                leader_cap,
                walk_index,
            },
        )?;

        let lock_tool_gas_args = runtime_tool_gas_args(
            tx,
            tools_gas,
            next_vertex,
            settle_current_vertex_gas,
            dag,
            execution,
            None,
        )?;

        match submission {
            PreparedOffchainToolResultSubmission::NoVerifier { result, auxiliary } => {
                commit_off_chain_tool_result_for_walk_without_verifier_v1(
                    tx,
                    dag,
                    execution,
                    worksheet,
                    leader_cap,
                    walk_index,
                    next_vertex,
                    result,
                    auxiliary.as_ref(),
                    leader_registry,
                )?;
            }
            PreparedOffchainToolResultSubmission::RegisteredKeyVerifier {
                result,
                auxiliary,
                verifier_credential,
                communication_evidence,
                bindings,
            } => {
                let verifier_objects =
                    offchain_verifier_ptb_objects(tx, leader_registry, bindings)?;
                let proof = OffChainVerifierProof::RegisteredKey {
                    verifier_credential: verifier_credential.clone(),
                    communication_evidence: communication_evidence.clone(),
                };

                commit_off_chain_tool_result_for_walk_v1(
                    tx,
                    dag,
                    execution,
                    tool_registry,
                    worksheet,
                    leader_cap,
                    walk_index,
                    next_vertex,
                    result,
                    auxiliary.as_ref(),
                    &proof,
                    verifier_objects.verifier_registry,
                    verifier_objects.leader_registry,
                    verifier_objects.network_auth,
                    verifier_objects.leader_key_binding,
                    verifier_objects.tool_key_binding,
                )?;
            }
            PreparedOffchainToolResultSubmission::ExternalVerifier {
                result,
                verifier_evidence,
                runtime_call,
                communication_evidence,
                bindings,
            } => {
                let verifier_objects =
                    offchain_verifier_ptb_objects(tx, leader_registry, bindings)?;

                commit_off_chain_tool_result_for_walk_with_external_verifier_proof_v1(
                    tx,
                    dag,
                    execution,
                    tool_registry,
                    worksheet,
                    leader_cap,
                    walk_index,
                    next_vertex,
                    result,
                    verifier_evidence,
                    communication_evidence,
                    runtime_call,
                    verifier_objects.verifier_registry,
                    verifier_objects.leader_registry,
                    verifier_objects.network_auth,
                    verifier_objects.leader_key_binding,
                    verifier_objects.tool_key_binding,
                )?;
            }
        }

        lock_payment_state_for_tools(tx, lock_tool_gas_args, dag, execution)?;

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn submit_on_chain_tool_result_for_walk_ptb(
    objects: &NexusObjects,
    dag: (sui::types::Address, sui::types::Version),
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    tools_gas: &[RuntimeToolGasRef],
    walk_index: u64,
    next_vertex: &RuntimeVertex,
    settle_current_vertex_gas: bool,
    submission: &PreparedOnchainToolResultSubmission,
) -> anyhow::Result<ProgrammableTransaction> {
    let dag_ref = dag;
    let execution_ref = execution;

    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let expected_vertex = runtime_vertex_arg(tx, next_vertex)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let RuntimeToolResultWorksheet {
            worksheet,
            agent_registry,
            dag: dag_arg,
            execution: execution_arg,
            clock,
        } = build_runtime_tool_result_worksheet(
            tx,
            &objects.agent_registry,
            RuntimeToolResultWorksheetInputs {
                dag: dag_ref,
                execution: execution_ref,
                leader_registry,
                leader_cap,
                walk_index,
            },
        )?;

        let lock_tool_gas_args = runtime_tool_gas_args(
            tx,
            tools_gas,
            next_vertex,
            settle_current_vertex_gas,
            dag_arg,
            execution_arg,
            Some(expected_vertex),
        )?;

        match submission {
            PreparedOnchainToolResultSubmission::Execute(execution_plan) => {
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
                    leader_cap,
                    leader_registry,
                    walk_index,
                    expected_vertex,
                    &pre_allocated,
                )?;
            }
            PreparedOnchainToolResultSubmission::TerminalErrEval {
                output,
                failure_evidence_kind: _,
                submitted_failure_reason: _,
            } => {
                let result = create_on_chain_tool_result_for_walk(
                    tx,
                    dag_arg,
                    execution_arg,
                    worksheet,
                    leader_cap,
                    leader_registry,
                    walk_index,
                    expected_vertex,
                )?;
                finalize_onchain_tool_result_output(tx, result, worksheet, output)?;
            }
        }

        lock_payment_state_for_tools(tx, lock_tool_gas_args, dag_arg, execution_arg)?;

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn consume_on_chain_tool_result_for_walk_ptb(
    objects: &NexusObjects,
    dag: (sui::types::Address, sui::types::Version),
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    tools_gas: &[RuntimeToolGasRef],
    walk_index: u64,
    next_vertex: &RuntimeVertex,
    result: (sui::types::Address, sui::types::Version),
    tool_witness_id: sui::types::Address,
    finalize_gas_charge: u64,
    settlement_gas_charge: u64,
    scheduled_payment_settlement: Option<(
        &sui::types::ObjectReference,
        &sui::types::ObjectReference,
    )>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let dag = tx.shared_object_by_id(dag.0, dag.1, false)?;
        let execution = tx.shared_object_by_id(execution.0, execution.1, true)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let result = tx.shared_object_by_id(result.0, result.1, true)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;

        consume_on_chain_tool_result_for_walk(
            tx,
            dag,
            execution,
            tool_registry,
            result,
            leader_cap,
            leader_registry,
            walk_index,
            next_vertex,
            tool_witness_id,
            finalize_gas_charge,
            settlement_gas_charge,
            clock,
        )?;

        let tools_gas =
            runtime_tool_gas_args(tx, tools_gas, next_vertex, false, dag, execution, None)?;
        lock_payment_state_for_tools(tx, tools_gas, dag, execution)?;
        emit_payment_ready_walk_requests(tx, dag, execution, leader_registry, clock);

        if let Some((task, scheduled_execution)) = scheduled_payment_settlement {
            scheduler::settle_finished_scheduled_execution_payment_if_ready(
                tx,
                task,
                scheduled_execution,
            )?;
        }

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn dry_run_on_chain_tool_result_for_walk_ptb(
    objects: &NexusObjects,
    dag: (sui::types::Address, sui::types::Version),
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
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let RuntimeToolResultWorksheet {
            worksheet,
            agent_registry,
            dag: dag_arg,
            execution: execution_arg,
            clock,
        } = build_runtime_tool_result_worksheet(
            tx,
            &objects.agent_registry,
            RuntimeToolResultWorksheetInputs {
                dag: dag_ref,
                execution: execution_ref,
                leader_registry,
                leader_cap,
                walk_index,
            },
        )?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
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
    objects: &NexusObjects,
    agent: AgentInput,
    execution: &sui::types::ObjectReference,
    amount: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let agent = agent.mutable_ptb_argument(tx)?;
        let execution = tx.shared_object(execution, true)?;
        let amount = tx.arg(&amount)?;
        tx.call_target(
            execution_settlement_binding::refill_tap_execution_payment_from_agent_vault_target,
            vec![agent, execution, amount],
        )?;
        Ok(())
    })
}

fn prepare_offchain_tool_result_bytes(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    result: &PreparedToolOutput,
) -> anyhow::Result<sui::types::Argument> {
    let output_variant = tx.ascii_string(result.output_variant.as_str())?;
    let output_ports_data = result
        .output_ports_data
        .iter()
        .map(|output_port| {
            let port = tx.ascii_string(output_port.port.as_str())?;
            let data = tx.nexus_data(&output_port.data)?;
            tx.call_target(
                verifier_binding::new_prepared_tool_output_port_target,
                vec![port, data],
            )
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let output_ports_data = tx.move_vector::<PreparedToolOutputPort>(output_ports_data)?;

    let prepared_tool_output = tx.call_target(
        verifier_binding::new_prepared_tool_output_target,
        vec![output_variant, output_ports_data],
    )?;

    tx.call_target(
        verifier_binding::prepared_tool_output_into_bcs_bytes_target,
        vec![prepared_tool_output],
    )
}

fn prepare_move_option_vec_u8(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    value: &Option<Vec<u8>>,
) -> anyhow::Result<sui::types::Argument> {
    let value = match value {
        Some(value) => Some(tx.arg(value)?),
        None => None,
    };
    Ok(tx.option::<Vec<u8>>(value)?)
}

fn prepare_submission_kind(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    submission_kind: VerificationSubmissionKind,
) -> anyhow::Result<sui::types::Argument> {
    tx.verification_submission_kind(&submission_kind)
}

fn prepare_verifier_evidence_kind(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    failure_evidence_kind: FailureEvidenceKind,
) -> anyhow::Result<sui::types::Argument> {
    tx.failure_evidence_kind(&failure_evidence_kind)
}

fn prepare_verifier_decision(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    decision: VerifierDecision,
) -> anyhow::Result<sui::types::Argument> {
    tx.verifier_decision(&decision)
}

fn prepare_verifier_contract_result(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    result: &VerifierContractResult,
) -> anyhow::Result<sui::types::Argument> {
    let method = tx.ascii_string(&result.method)?;
    let decision = prepare_verifier_decision(tx, result.decision.clone())?;
    let submission_kind = prepare_submission_kind(tx, result.submission_kind.clone())?;
    let failure_evidence_kind =
        prepare_verifier_evidence_kind(tx, result.failure_evidence_kind.clone())?;
    let payload_or_reason_hash = tx.arg(&result.payload_or_reason_hash)?;
    let credential = tx.arg(&result.credential)?;
    let detail = tx.arg(&result.detail)?;

    tx.call_target(
        verifier_binding::new_verifier_contract_result_target,
        vec![
            method,
            decision,
            submission_kind,
            failure_evidence_kind,
            payload_or_reason_hash,
            credential,
            detail,
        ],
    )
}

fn prepare_external_verifier_submit_evidence(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    evidence: &ExternalVerifierSubmitEvidence,
) -> anyhow::Result<sui::types::Argument> {
    let result = prepare_verifier_contract_result(tx, &evidence.result)?;
    let communication_evidence = tx.arg(&evidence.communication_evidence)?;

    tx.call_target(
        verifier_binding::new_external_verifier_submit_evidence_target,
        vec![result, communication_evidence],
    )
}

fn prepare_offchain_verifier_proof(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    proof: &OffChainVerifierProof,
) -> anyhow::Result<sui::types::Argument> {
    match proof {
        OffChainVerifierProof::RegisteredKey {
            verifier_credential,
            communication_evidence,
        } => {
            let verifier_credential = tx.arg(verifier_credential)?;
            let communication_evidence = tx.arg(communication_evidence)?;
            tx.call_target(
                verifier_binding::new_off_chain_verifier_proof_registered_key_target,
                vec![verifier_credential, communication_evidence],
            )
        }
        OffChainVerifierProof::ExternalVerifier { evidence } => {
            let evidence = prepare_external_verifier_submit_evidence(tx, evidence)?;
            tx.call_target(
                verifier_binding::new_off_chain_verifier_proof_external_verifier_target,
                vec![evidence],
            )
        }
    }
}

fn prepare_authenticated_offchain_request_evidence(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    expected_vertex: sui::types::Argument,
    request: &AuthenticatedOffchainRequestEvidence,
) -> anyhow::Result<sui::types::Argument> {
    let walk_index = tx.arg(&request.walk_index)?;
    let tool_fqn = tx.ascii_string(&request.tool_fqn)?;
    let request_hash = tx.arg(&request.request_hash)?;
    let request_signature = tx.arg(&request.request_signature)?;

    tx.call_target(
        execution_submission_binding::new_authenticated_offchain_request_evidence_target,
        vec![
            execution,
            leader_cap,
            walk_index,
            expected_vertex,
            tool_fqn,
            request_hash,
            request_signature,
        ],
    )
}

fn prepare_offchain_response_evidence(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    response: &OffchainResponseEvidence,
) -> anyhow::Result<sui::types::Argument> {
    let status_code = tx.arg(&response.status_code)?;
    let response_hash = tx.arg(&response.response_hash)?;
    let response_signature = tx.arg(&response.response_signature)?;
    let normalized_err_eval_reason_hash_value =
        response.normalized_err_eval_reason_hash.cloned_option();
    let normalized_err_eval_reason_hash =
        prepare_move_option_vec_u8(tx, &normalized_err_eval_reason_hash_value)?;

    tx.call_target(
        verifier_binding::new_offchain_response_evidence_target,
        vec![
            status_code,
            response_hash,
            response_signature,
            normalized_err_eval_reason_hash,
        ],
    )
}

fn prepare_offchain_verifier_evidence(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    expected_vertex: sui::types::Argument,
    evidence: &AuthenticatedOffchainVerifierEvidence,
) -> anyhow::Result<sui::types::Argument> {
    let submission_kind = prepare_submission_kind(tx, evidence.submission_kind.clone())?;
    let payload_or_reason_hash = tx.arg(&evidence.payload_or_reason_hash)?;
    let transport_proof = tx.arg(&evidence.transport_proof)?;
    let request = prepare_authenticated_offchain_request_evidence(
        tx,
        execution,
        leader_cap,
        expected_vertex,
        &evidence.request,
    )?;
    let response = prepare_offchain_response_evidence(tx, &evidence.response)?;

    tx.call_target(
        verifier_binding::new_offchain_verifier_evidence_target,
        vec![
            submission_kind,
            payload_or_reason_hash,
            transport_proof,
            request,
            response,
        ],
    )
}

struct ExternalVerifierCallResult {
    pub worksheet: sui::types::Argument,
    pub result: sui::types::Argument,
}

fn external_verifier_call_results(
    tx: &move_boundary::NexusPtbBuilder<'_>,
    call: sui::types::Argument,
) -> anyhow::Result<ExternalVerifierCallResult> {
    Ok(ExternalVerifierCallResult {
        worksheet: tx.nested_result(call, 0)?,
        result: tx.nested_result(call, 1)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn call_external_verifier_v1_with_authenticated_request(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    worksheet: sui::types::Argument,
    expected_vertex: sui::types::Argument,
    verifier_evidence: &AuthenticatedOffchainVerifierEvidence,
    runtime_call: &ExternalVerifierRuntimeCall,
) -> anyhow::Result<ExternalVerifierCallResult> {
    let witness = tx.shared_object(&runtime_call.witness, true)?;
    let shared_objects = runtime_call
        .shared_objects
        .iter()
        .map(|(shared, object_ref)| tx.shared_object(object_ref, shared.ref_mut))
        .collect::<Result<Vec<_>, _>>()?;
    let verifier_evidence = prepare_offchain_verifier_evidence(
        tx,
        execution,
        leader_cap,
        expected_vertex,
        verifier_evidence,
    )?;
    let call = tx.call_function(
        runtime_call.package_address,
        &runtime_call.module_name,
        &runtime_call.function_name,
        {
            let mut args = Vec::with_capacity(shared_objects.len() + 3);
            args.push(witness);
            args.extend(shared_objects);
            args.push(worksheet);
            args.push(verifier_evidence);
            args
        },
    )?;
    external_verifier_call_results(tx, call)
}

#[allow(clippy::too_many_arguments)]
fn commit_off_chain_tool_result_for_walk_v1(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    result: &PreparedToolOutput,
    auxiliary: Option<&OffChainToolResultAuxiliary>,
    proof: &OffChainVerifierProof,
    verifier_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
    network_auth: sui::types::Argument,
    leader_key_binding: sui::types::Argument,
    tool_key_binding: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, result)?;
    let auxiliary_bytes =
        prepare_move_option_vec_u8(tx, &auxiliary.map(bcs::to_bytes).transpose()?)?;
    let proof = prepare_offchain_verifier_proof(tx, proof)?;

    tx.call_target(
        execution_submission_binding::commit_off_chain_tool_result_for_walk_v1_target,
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
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_off_chain_tool_result_for_walk_without_verifier_v1(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    result: &PreparedToolOutput,
    auxiliary: Option<&OffChainToolResultAuxiliary>,
    leader_registry: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, result)?;
    let auxiliary_bytes =
        prepare_move_option_vec_u8(tx, &auxiliary.map(bcs::to_bytes).transpose()?)?;

    tx.call_target(execution_submission_binding::commit_off_chain_tool_result_for_walk_without_verifier_v1_target, vec![
            dag,
            execution,
            worksheet,
            leader_cap,
            walk_index,
            expected_vertex,
            result_bytes,
            auxiliary_bytes,
            leader_registry,
        ])?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_off_chain_tool_result_for_walk_with_external_verifier_proof_v1(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    result: &PreparedToolOutput,
    verifier_evidence: &AuthenticatedOffchainVerifierEvidence,
    communication_evidence: &[u8],
    runtime_call: &ExternalVerifierRuntimeCall,
    verifier_registry: sui::types::Argument,
    leader_registry: sui::types::Argument,
    network_auth: sui::types::Argument,
    leader_key_binding: sui::types::Argument,
    tool_key_binding: sui::types::Argument,
) -> anyhow::Result<()> {
    let expected_vertex_arg = runtime_vertex_arg(tx, expected_vertex)?;
    let verifier_call = call_external_verifier_v1_with_authenticated_request(
        tx,
        execution,
        leader_cap,
        worksheet,
        expected_vertex_arg,
        verifier_evidence,
        runtime_call,
    )?;
    let worksheet = verifier_call.worksheet;
    let communication_evidence = tx.arg(&communication_evidence.to_vec())?;
    let external_verifier_evidence = tx.call_target(
        verifier_binding::new_external_verifier_submit_evidence_target,
        vec![verifier_call.result, communication_evidence],
    )?;
    let proof = tx.call_target(
        verifier_binding::new_off_chain_verifier_proof_external_verifier_target,
        vec![external_verifier_evidence],
    )?;
    let auxiliary = prepare_move_option_vec_u8(tx, &Option::<Vec<u8>>::None)?;
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let result_bytes = prepare_offchain_tool_result_bytes(tx, result)?;

    tx.call_target(
        execution_submission_binding::commit_off_chain_tool_result_for_walk_v1_target,
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
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn release_vertex_authorization_for_onchain_walk(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
) -> anyhow::Result<sui::types::Argument> {
    let walk_index = tx.arg(&walk_index)?;
    tx.call_target(
        execution_submission_binding::release_vertex_authorization_for_onchain_walk_target,
        vec![dag, execution, worksheet, leader_cap, walk_index],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn create_on_chain_tool_result_for_walk(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    worksheet: sui::types::Argument,
    leader_cap: sui::types::Argument,
    leader_registry: sui::types::Argument,
    walk_index: u64,
    expected_vertex: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let walk_index = tx.arg(&walk_index)?;

    tx.call_target(
        execution_submission_binding::create_on_chain_tool_result_for_walk_target,
        vec![
            dag,
            execution,
            worksheet,
            leader_cap,
            leader_registry,
            walk_index,
            expected_vertex,
        ],
    )
}

pub fn framework_random_object(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.shared_object_by_id(move_boundary::RANDOM_OBJECT_ID, 1, false)?)
}

#[allow(clippy::too_many_arguments)]
pub fn consume_on_chain_tool_result_for_walk(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    result: sui::types::Argument,
    leader_cap: sui::types::Argument,
    leader_registry: sui::types::Argument,
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

    tx.call_target(
        execution_submission_binding::consume_on_chain_tool_result_for_walk_target,
        vec![
            dag,
            execution,
            tool_registry,
            result,
            leader_cap,
            leader_registry,
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

fn finalize_onchain_tool_result_output(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    result: sui::types::Argument,
    worksheet: sui::types::Argument,
    output: &PreparedOnchainToolOutput,
) -> anyhow::Result<()> {
    let output = prepare_tagged_tool_output(tx, output)?;
    tx.call_target(
        onchain_tool_result_binding::finalize_and_share_target,
        vec![result, worksheet, output],
    )?;
    Ok(())
}

fn prepare_tagged_tool_output(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    prepared: &PreparedOnchainToolOutput,
) -> anyhow::Result<sui::types::Argument> {
    let variant = tx.arg(&prepared.output_variant.as_bytes().to_vec())?;
    let mut tagged_output = tx.call_target(tagged_output_binding::new_target, vec![variant])?;

    for (output_port, dag_data) in &prepared.output_ports_data {
        let port = tx.arg(&output_port.as_bytes().to_vec())?;
        let value = tx.nexus_data(dag_data)?;
        let typed_value = tx.call_target(data_binding::as_raw_target, vec![value])?;
        tagged_output = tx.call_target(
            tagged_output_binding::with_named_payload_target,
            vec![tagged_output, port, typed_value],
        )?;
    }

    Ok(tagged_output)
}

#[allow(clippy::too_many_arguments)]
fn record_committed_tool_result_gas_charge_by_leader(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let commit_tx_digest = tx.arg(&commit_tx_digest)?;
    let commit_gas_charge = tx.arg(&commit_gas_charge)?;
    let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;

    tx.call_target(
        execution_settlement_binding::record_committed_tool_result_gas_charge_by_leader_target,
        vec![
            execution,
            leader_cap,
            walk_index,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn record_committed_tool_result_gas_charge_by_leader_ptb(
    objects: &NexusObjects,
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let execution = tx.shared_object_by_id(execution.0, execution.1, true)?;
        let leader_cap = tx.shared_object(leader_cap, false)?;

        record_committed_tool_result_gas_charge_by_leader(
            tx,
            execution,
            leader_cap,
            walk_index,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn settle_committed_tool_result_for_walk_by_leader(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let commit_tx_digest = tx.arg(&commit_tx_digest)?;
    let commit_gas_charge = tx.arg(&commit_gas_charge)?;
    let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;

    tx.call_target(
        execution_settlement_binding::settle_committed_tool_result_for_walk_by_leader_target,
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
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn settle_committed_tool_result_for_walk_by_leader_ptb(
    objects: &NexusObjects,
    dag: (sui::types::Address, sui::types::Version),
    execution: (sui::types::Address, sui::types::Version),
    leader_cap: &sui::types::ObjectReference,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
    scheduled_payment_settlement: Option<(
        &sui::types::ObjectReference,
        &sui::types::ObjectReference,
    )>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let dag = tx.shared_object_by_id(dag.0, dag.1, false)?;
        let execution = tx.shared_object_by_id(execution.0, execution.1, true)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let clock = tx.clock()?;

        settle_committed_tool_result_for_walk_by_leader(
            tx,
            dag,
            execution,
            tool_registry,
            leader_cap,
            walk_index,
            commit_tx_digest,
            commit_gas_charge,
            settlement_gas_charge,
            clock,
        )?;

        let mut tools_gas_args = Vec::with_capacity(tools_gas.len());
        for (addr, ver) in tools_gas {
            tools_gas_args.push(tx.shared_object_by_id(*addr, *ver, true)?);
        }
        lock_payment_state_for_tools(tx, tools_gas_args, dag, execution)?;
        emit_payment_ready_walk_requests(tx, dag, execution, leader_registry, clock);

        if let Some((task, scheduled_execution)) = scheduled_payment_settlement {
            scheduler::settle_finished_scheduled_execution_payment_if_ready(
                tx,
                task,
                scheduled_execution,
            )?;
        }

        Ok(())
    })
}

/// Build a PTB that aborts an expired DAG execution.
pub fn abort_expired_execution_for_self_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    broken_onchain_result_cleanups: &[BrokenOnchainToolResultCleanupInput],
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.shared_object(dag, false)?;
        let execution = tx.shared_object(execution, true)?;
        let clock = tx.clock()?;

        if !broken_onchain_result_cleanups.is_empty() {
            let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
            let leader_registry = tx.shared_object(&objects.leader_registry, false)?;

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

        tx.call_target(
            execution_settlement_binding::abort_expired_execution_target,
            vec![dag, execution, clock],
        )?;
        Ok(())
    })
}

/// Build a PTB that settles a committed tool result for one walk.
pub fn settle_committed_tool_result_for_walk_for_self_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    walk_index: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.shared_object(dag, false)?;
        let execution = tx.shared_object(execution, true)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let walk_index = tx.arg(&walk_index)?;
        let clock = tx.clock()?;

        tx.call_target(
            execution_settlement_binding::settle_committed_tool_result_for_walk_target,
            vec![dag, execution, tool_registry, walk_index, clock],
        )?;
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn settle_onchain_tool_result_for_walk(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    tool_registry: sui::types::Argument,
    result: sui::types::Argument,
    leader_registry: sui::types::Argument,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    tool_witness_id: sui::types::Address,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let walk_index = tx.arg(&walk_index)?;
    let expected_vertex = runtime_vertex_arg(tx, expected_vertex)?;
    let tool_witness_id = tx.object_id(tool_witness_id)?;

    tx.call_target(
        execution_settlement_binding::settle_onchain_tool_result_for_walk_target,
        vec![
            dag,
            execution,
            tool_registry,
            result,
            leader_registry,
            walk_index,
            expected_vertex,
            tool_witness_id,
            clock,
        ],
    )?;

    Ok(())
}

/// Build a PTB that settles a finalized on chain tool result for one walk.
pub fn settle_onchain_tool_result_for_walk_for_self_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    result: &sui::types::ObjectReference,
    walk_index: u64,
    expected_vertex: &RuntimeVertex,
    tool_witness_id: sui::types::Address,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.shared_object(dag, false)?;
        let execution = tx.shared_object(execution, true)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let result = tx.shared_object(result, true)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;

        settle_onchain_tool_result_for_walk(
            tx,
            dag,
            execution,
            tool_registry,
            result,
            leader_registry,
            walk_index,
            expected_vertex,
            tool_witness_id,
            clock,
        )?;

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn cleanup_broken_onchain_tool_result(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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

    tx.call_target(
        execution_settlement_binding::cleanup_broken_onchain_tool_result_target,
        vec![
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
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    execution_owner: &sui::types::Owner,
    leader_cap: &sui::types::ObjectReference,
    leader_cap_owner: &sui::types::Owner,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let dag = tx.shared_object(dag, false)?;
        let execution = tx.object_from_owner(execution, execution_owner, true)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let leader_cap = tx.object_from_owner(leader_cap, leader_cap_owner, false)?;
        let walk_index = tx.arg(&walk_index)?;
        let commit_tx_digest = tx.arg(&commit_tx_digest)?;
        let commit_gas_charge = tx.arg(&commit_gas_charge)?;
        let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;
        let clock = tx.clock()?;

        tx.call_target(
            execution_settlement_binding::settle_committed_tool_result_for_walk_by_leader_target,
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
        )?;
        Ok(())
    })
}

/// Build a PTB that records leader gas accounting for a committed tool result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_committed_tool_result_gas_charge_by_leader_for_self_ptb(
    objects: &NexusObjects,
    execution: &sui::types::ObjectReference,
    execution_owner: &sui::types::Owner,
    leader_cap: &sui::types::ObjectReference,
    leader_cap_owner: &sui::types::Owner,
    walk_index: u64,
    commit_tx_digest: Vec<u8>,
    commit_gas_charge: u64,
    settlement_gas_charge: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let execution = tx.object_from_owner(execution, execution_owner, true)?;
        let leader_cap = tx.object_from_owner(leader_cap, leader_cap_owner, false)?;
        let walk_index = tx.arg(&walk_index)?;
        let commit_tx_digest = tx.arg(&commit_tx_digest)?;
        let commit_gas_charge = tx.arg(&commit_gas_charge)?;
        let settlement_gas_charge = tx.arg(&settlement_gas_charge)?;

        tx.call_target(
            execution_settlement_binding::record_committed_tool_result_gas_charge_by_leader_target,
            vec![
                execution,
                leader_cap,
                walk_index,
                commit_tx_digest,
                commit_gas_charge,
                settlement_gas_charge,
            ],
        )?;
        Ok(())
    })
}

fn emit_payment_ready_walk_requests(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    leader_registry: sui::types::Argument,
    clock: sui::types::Argument,
) {
    tx.call_target(
        execution_settlement_binding::emit_payment_ready_walk_requests_target,
        vec![dag, execution, leader_registry, clock],
    )
    .expect("generated execution_settlement::emit_payment_ready_walk_requests target is valid");
}

/// PTB template for creating a new DAG default value.
fn create_default_value(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    default_value: &DagDefaultValue,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = tx.graph_vertex(&default_value.vertex)?;
    let port = tx.graph_input_port(&default_value.input_port)?;
    let value = tx.nexus_data(&default_value.value)?;

    tx.call_target(
        dag_binding::with_default_value_target,
        vec![dag, vertex, port, value],
    )
}

/// PTB template for creating a new DAG edge.
fn create_edge(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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

#[allow(clippy::too_many_arguments)]
fn begin_user_funded_agent_execution(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool_registry: sui::types::Argument,
    agent_registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag: sui::types::Argument,
    _dag_id: sui::types::Argument,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    payment_coin: sui::types::Argument,
    clock: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let network = tx.object_id(objects.network_id)?;
    let entry_group = tx.graph_entry_group(entry_group)?;
    let with_vertex_inputs = begin_execution_inputs_arg(tx, input_data)?;

    let priority_fee_per_gas_unit = tx.arg(&priority_fee_per_gas_unit)?;

    let agent_id = tx.object_id(agent_execution.agent_id)?;
    let agent_config = tap::agent_execution_config_arg(
        tx,
        agent_id,
        network,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
        agent_execution.skill_id,
        agent_execution.selected_dag,
        &agent_execution.authorization_templates,
    )?;

    let payment_max_budget = tx.arg(&agent_execution.payment_max_budget)?;

    tx.call_target(
        execution_entries_binding::begin_user_funded_agent_execution_target,
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
    )
}

#[allow(clippy::too_many_arguments)]
fn begin_default_dag_execution(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool_registry: sui::types::Argument,
    agent_registry: sui::types::Argument,
    dag: sui::types::Argument,
    dag_id: sui::types::Argument,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    payment_coin: sui::types::Argument,
    clock: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let network = tx.object_id(objects.network_id)?;
    let entry_group = tx.graph_entry_group(entry_group)?;
    let with_vertex_inputs = begin_execution_inputs_arg(tx, input_data)?;

    let priority_fee_per_gas_unit = tx.arg(&priority_fee_per_gas_unit)?;
    let config = tap::default_agent_execution_config_arg(
        tx,
        dag_id,
        network,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
    )?;

    let payment_max_budget = tx.arg(&agent_execution.payment_max_budget)?;

    tx.call_target(
        execution_entries_binding::begin_default_dag_execution_target,
        vec![
            dag,
            agent_registry,
            tool_registry,
            config,
            payment_coin,
            payment_max_budget,
            clock,
        ],
    )
}

/// PTB template to lock execution payment state for the given tools.
#[allow(clippy::too_many_arguments)]
fn lock_payment_state_for_tools(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tools_gas: Vec<sui::types::Argument>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
) -> anyhow::Result<()> {
    for tool_gas in tools_gas {
        tx.call_target(
            gas_binding::lock_payment_state_for_tool_target,
            vec![tool_gas, dag, execution],
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn append_execute_agent_dag(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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
pub(crate) fn execute_agent_dag_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    agent: AgentInput,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        append_execute_agent_dag(
            tx,
            dag,
            agent,
            priority_fee_per_gas_unit,
            entry_group,
            input_data,
            agent_execution,
            tools_gas,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub fn execute_default_agent_dag_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        append_execute_default_agent_dag(
            tx,
            dag,
            priority_fee_per_gas_unit,
            entry_group,
            input_data,
            agent_execution,
            tools_gas,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn append_execute_default_agent_dag(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: &sui::types::ObjectReference,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    execute_agent_dag_internal(
        tx,
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
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: &sui::types::ObjectReference,
    agent: Option<AgentInput>,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_execution: &AgentDagExecuteInput,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
    default_executor: bool,
) -> anyhow::Result<()> {
    let objects = tx.objects();
    let dag_id = tx.object_id(*dag.object_id())?;
    let dag = tx.shared_object(dag, false)?;

    let agent = match agent {
        Some(agent) => Some(agent.mutable_ptb_argument(tx)?),
        None => None,
    };

    let tool_registry = tx.shared_object(&objects.tool_registry, false)?;

    let agent_registry = tx.shared_object(&objects.agent_registry, false)?;

    let clock = tx.clock()?;

    let payment_coin = if let Some(payment_coin_ref) = agent_execution.payment_coin.as_ref() {
        let owned_payment_coin = tx.owned_object(payment_coin_ref)?;
        match agent_execution.payment_coin_balance {
            Some(balance) if balance > agent_execution.payment_max_budget => {
                let payment_amount = tx.arg(&agent_execution.payment_max_budget)?;
                tx.split_coins(owned_payment_coin, vec![payment_amount])?
            }
            _ => owned_payment_coin,
        }
    } else {
        let payment_amount = tx.arg(&agent_execution.payment_max_budget)?;
        let gas = tx.gas();
        tx.split_coins(gas, vec![payment_amount])?
    };

    let results = if default_executor {
        begin_default_dag_execution(
            tx,
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
    let gas_service = tx.shared_object(&objects.gas_service, false)?;
    let tools_gas = tools_gas
        .iter()
        .map(|(address, version)| tx.shared_object_by_id(*address, *version, true))
        .collect::<Result<Vec<_>, _>>()?;

    crate::transactions::gas::snapshot_dag_tool_costs(tx, gas_service, execution, dag)?;
    lock_payment_state_for_tools(tx, tools_gas, dag, execution)?;

    let leader_registry = tx.shared_object(&objects.leader_registry, false)?;

    tx.call_target(
        execution_entries_binding::start_execution_target,
        vec![dag, execution, leader_registry, clock],
    )?;

    tx.call_target(
        transfer_binding::public_share_object_target::<execution_binding::DAGExecution>,
        vec![execution],
    )?;

    Ok(())
}
