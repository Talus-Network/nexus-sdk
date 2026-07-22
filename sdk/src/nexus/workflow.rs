//! Commands related to workflow management in Nexus.
//!
//! - [`WorkflowActions::publish`] to publish a [`DagSpec`] instance to Nexus.
//! - [`WorkflowActions::execute`] to execute a published DAG.
//! - [`WorkflowActions::inspect_execution`] to monitor the execution of a DAG.

#[cfg(feature = "walrus")]
use crate::{
    move_bindings::interface::{agent::SkillDagBinding, graph::InputPort},
    types::{
        payment_source_from_address,
        resolve_active_tap_skill_execution_target,
        resolve_default_tap_dag_executor,
        validate_execution_payment_options,
        AgentId,
        AgentRegistrySnapshot,
        DefaultDagExecutorRecord,
        SkillId,
        SkillRevisionLookupKey,
        DEFAULT_ENTRY_GROUP,
    },
    walrus::StorageConf,
};
use {
    crate::{
        events::{EventPage, NexusEvent, NexusEventKind},
        move_bindings::{
            interface::{
                dag as dag_move,
                graph::{self as graph_move, RuntimeVertex},
                payment::{ExecutionPayment, ExecutionPaymentVertexLock},
                verifier::{FailureEvidenceKind, VerifierConfig, VerifierMode},
            },
            move_std::type_name::TypeName,
            primitives::{data::NexusData, onchain_tool_result::OnchainToolResult},
            sui_framework::{clock::Clock as SuiClock, linked_table, object::ID, vec_map::VecMap},
            workflow::{
                execution::{self as execution_move, DAGExecution, DAGWalk},
                execution_events::{
                    EndStateReachedEvent,
                    ExecutionFinishedEvent,
                    TerminalErrEvalRecordedEvent,
                },
                execution_failure::WorkflowFailureClass,
            },
        },
        move_boundary,
        nexus::{client::NexusClient, crawler::Crawler, error::NexusError, tap},
        sui,
        transactions::{dag, gas},
        types::{DagSpec, NexusObjects, Tool, ToolRef},
    },
    anyhow::anyhow,
    sha2::{Digest as _, Sha256},
    std::collections::HashMap,
    tokio::{
        sync::mpsc::{unbounded_channel, UnboundedReceiver},
        task::JoinHandle,
        time::Duration,
    },
};

const COMMITTED_TOOL_RESULT_VALUE_TYPE_SUFFIX: &str = "::execution::CommittedToolResult";
const EXECUTION_PAYMENT_INSUFFICIENT_SETTLEMENT_VALUE_TYPE_SUFFIX: &str =
    "::execution::ExecutionPaymentInsufficientSettlement";
const ONCHAIN_TOOL_RESULT_ID_VALUE_TYPE_SUFFIX: &str = "::object::ID";
pub const EXPIRED_WALK_NOT_DOUBLE_TIMEOUT_EXPIRED_REASON: &str =
    "walk is not double timeout expired";
pub const EXPIRED_WALK_ALREADY_TERMINAL_REASON: &str = "walk is already terminal";

#[derive(Clone, Debug)]
pub struct PublishResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_object_id: sui::types::Address,
}

#[cfg(feature = "walrus")]
pub struct ExecuteResult {
    pub tx_digest: sui::types::Digest,
    pub execution_object_id: sui::types::Address,
    pub tx_checkpoint: u64,
    pub tap_execution: Option<TapExecutionSubmitMetadata>,
}

#[cfg(feature = "walrus")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapExecutionSubmitMetadata {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub dag_id: sui::types::Address,
    pub skill_revision_key: SkillRevisionLookupKey,
    pub payment_max_budget: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ToolGasAbortCandidate {
    pub tool_fqn: crate::ToolFqn,
    pub tool_gas_ref: sui::types::ObjectReference,
    pub matching_walks: Vec<ToolGasAbortCandidateWalk>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ToolGasAbortCandidateWalk {
    pub walk_index: usize,
    pub vertex: RuntimeVertex,
    pub payment_vertex_key: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbortExpiredExecutionResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_id: sui::types::Address,
    pub dag_execution_id: sui::types::Address,
    pub selected_candidate: ToolGasAbortCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbortExecutionResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_id: sui::types::Address,
    pub dag_execution_id: sui::types::Address,
    pub cleaned_broken_onchain_results: Vec<BrokenOnchainToolResultCleanup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettleCommittedToolResultParams {
    pub dag_execution_id: sui::types::Address,
    pub walk_index: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveExpiredWalkParams {
    pub dag_execution_id: sui::types::Address,
    pub walk_index: u64,
    pub tool_gas_id: Option<sui::types::Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpiredWalkResolutionKind {
    Settled,
    SettledOnchainResult {
        result_ref: sui::types::ObjectReference,
        expected_vertex: RuntimeVertex,
        tool_witness_id: sui::types::Address,
        finalize_tx_digest: Vec<u8>,
    },
    Aborted,
    AbortedWithToolGas {
        selected_candidate: ToolGasAbortCandidate,
    },
    Skipped {
        reason: String,
    },
}

impl ExpiredWalkResolutionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::SettledOnchainResult { .. } => "settled_onchain_result",
            Self::Aborted => "aborted",
            Self::AbortedWithToolGas { .. } => "aborted_with_tool_gas",
            Self::Skipped { .. } => "skipped",
        }
    }

    pub fn selected_candidate(&self) -> Option<&ToolGasAbortCandidate> {
        match self {
            Self::AbortedWithToolGas { selected_candidate } => Some(selected_candidate),
            _ => None,
        }
    }

    pub fn skip_reason(&self) -> Option<&str> {
        match self {
            Self::Skipped { reason } => Some(reason),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpiredWalkResolutionPlan {
    pub dag_id: sui::types::Address,
    pub dag_execution_id: sui::types::Address,
    pub walk_index: u64,
    pub kind: ExpiredWalkResolutionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpiredWalkResolutionResult {
    pub tx_digest: Option<sui::types::Digest>,
    pub tx_checkpoint: Option<u64>,
    pub dag_id: sui::types::Address,
    pub dag_execution_id: sui::types::Address,
    pub walk_index: u64,
    pub resolution_kind: ExpiredWalkResolutionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettleCommittedToolResultByLeaderParams {
    pub dag_execution_id: sui::types::Address,
    pub leader_cap_id: sui::types::Address,
    pub walk_index: u64,
    pub commit_tx_digest: Vec<u8>,
    pub commit_gas_charge: u64,
    pub settlement_gas_charge: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordCommittedToolResultGasChargeParams {
    pub dag_execution_id: sui::types::Address,
    pub leader_cap_id: sui::types::Address,
    pub walk_index: u64,
    pub commit_tx_digest: Vec<u8>,
    pub commit_gas_charge: u64,
    pub settlement_gas_charge: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommittedToolResultSettlementResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_id: sui::types::Address,
    pub dag_execution_id: sui::types::Address,
    pub walk_index: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokenOnchainToolResultCleanup {
    pub walk_index: u64,
    pub result_ref: sui::types::ObjectReference,
    pub expected_vertex: RuntimeVertex,
    pub tool_witness_id: sui::types::Address,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordCommittedToolResultGasChargeResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_execution_id: sui::types::Address,
    pub leader_cap_id: sui::types::Address,
    pub walk_index: u64,
}

/// Narrowed committed result view for off chain freshness checks.
///
/// This is separate from [`DAGExecution`] because callers only need committed result state and
/// should not read or decode the full execution object for early wake decisions.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CommittedToolResultView {
    pub expected_vertex: RuntimeVertex,
    pub primary_failure_evidence_kind: Option<FailureEvidenceKind>,
    pub secondary_failure_evidence_kind: Option<FailureEvidenceKind>,
    pub current_leader_cap_id: sui::types::Address,
    pub has_finalized_onchain_payload: bool,
    pub leader_records: Vec<CommittedToolResultLeaderRecordView>,
}

impl CommittedToolResultView {
    pub fn leader_record(
        &self,
        leader_cap_id: sui::types::Address,
    ) -> Option<&CommittedToolResultLeaderRecordView> {
        self.leader_records
            .iter()
            .find(|record| record.leader_cap_id == leader_cap_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CommittedToolResultLeaderRecordView {
    pub leader_cap_id: sui::types::Address,
    pub commit_tx_digest: Vec<u8>,
    pub recipient: sui::types::Address,
    pub commit_gas_charge: Option<u64>,
    pub settlement_gas_charge: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct InsufficientSettlementMarkerView {
    pub walks: Vec<u64>,
}

pub enum OnchainToolResultState {
    NoResult,
    Finalized {
        result: OnchainToolResult,
        object_ref: sui::types::ObjectReference,
    },
    Committed {
        committed_result: CommittedToolResultView,
    },
    InsufficientSettlement {
        committed_result: Option<CommittedToolResultView>,
        marker: InsufficientSettlementMarkerView,
    },
    InvalidEmpty {
        result_id: sui::types::Address,
        object_ref: sui::types::ObjectReference,
    },
}

impl OnchainToolResultState {
    /// Return the fetched generated result and object metadata that let a
    /// retrying leader skip execution and build the consume+settle transaction.
    pub fn consume_ready_result(
        &self,
    ) -> Option<(&OnchainToolResult, &sui::types::ObjectReference)> {
        match self {
            Self::Finalized { result, object_ref } => Some((result, object_ref)),
            Self::NoResult
            | Self::Committed { .. }
            | Self::InsufficientSettlement { .. }
            | Self::InvalidEmpty { .. } => None,
        }
    }

    /// Build the mutable shared-object input expected by
    /// `consume_on_chain_tool_result_for_walk`.
    pub fn consume_object_input(&self) -> Option<sui_move_call::CallArg> {
        self.consume_ready_result().map(|(_, object_ref)| {
            sui_move_call::CallArg::Shared(sui::types::SharedInput::new(
                *object_ref.object_id(),
                object_ref.version(),
                true,
            ))
        })
    }
}

fn onchain_tool_result_is_finalized(result: &OnchainToolResult) -> bool {
    result.finalized
        && result.tag.as_option().is_some()
        && result.named_payload.as_option().is_some()
        && result.finalize_tx_digest.as_option().is_some()
        && result.finalize_recipient.as_option().is_some()
}

impl From<execution_move::CommittedToolResult> for CommittedToolResultView {
    fn from(value: execution_move::CommittedToolResult) -> Self {
        Self {
            expected_vertex: value.expected_vertex,
            primary_failure_evidence_kind: value.primary_failure_evidence_kind.into_option(),
            secondary_failure_evidence_kind: value.secondary_failure_evidence_kind.into_option(),
            current_leader_cap_id: value.current_leader_cap_id.bytes,
            has_finalized_onchain_payload: value.has_finalized_onchain_payload,
            leader_records: value
                .leader_records
                .contents
                .into_iter()
                .map(|entry| CommittedToolResultLeaderRecordView {
                    leader_cap_id: entry.key.bytes,
                    commit_tx_digest: entry.value.commit_tx_digest,
                    recipient: entry.value.recipient,
                    commit_gas_charge: entry.value.commit_gas_charge.into_option(),
                    settlement_gas_charge: entry.value.settlement_gas_charge.into_option(),
                })
                .collect(),
        }
    }
}

#[cfg(feature = "walrus")]
#[derive(Clone, Debug, Default)]
pub struct AgentDagExecuteOptions {
    pub payment_source: Vec<u8>,
    pub payment_coin: Option<sui::types::ObjectReference>,
    pub payment_coin_balance: Option<u64>,
    pub payment_max_budget: u64,
}

#[cfg(feature = "walrus")]
fn resolve_default_agent_dag_executor(
    objects: &crate::types::NexusObjects,
    registry: &AgentRegistrySnapshot,
) -> anyhow::Result<DefaultDagExecutorRecord> {
    let configured = objects.default_dag_executor;
    if let Ok(target) = resolve_active_tap_skill_execution_target(
        registry,
        configured.agent_id,
        configured.skill_id,
    ) {
        if target.skill.dag_binding() == &SkillDagBinding::RuntimeSelected {
            return Ok(DefaultDagExecutorRecord {
                target: configured,
                skill: target.skill,
                skill_revision: target.skill_revision,
            });
        }
    }

    resolve_default_tap_dag_executor(registry)
}

pub struct InspectExecutionResult {
    pub next_event: UnboundedReceiver<NexusEvent>,
    pub poller: JoinHandle<Result<(), NexusError>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowExecutionTerminalState {
    Succeeded,
    Failed,
    Aborted,
    NoWalkOutcome,
}

#[derive(Clone, Debug)]
pub struct ResolvedEndState {
    pub event: EndStateReachedEvent,
    pub resolved_ports_to_data: HashMap<String, NexusData>,
}

#[derive(Clone, Debug)]
pub struct InspectExecutionCompletionResult {
    pub terminal_state: WorkflowExecutionTerminalState,
    pub execution_finished: ExecutionFinishedEvent,
    pub end_states: Vec<ResolvedEndState>,
    pub terminal_err_eval_recordings: Vec<TerminalErrEvalRecordedEvent>,
    pub events: Vec<NexusEvent>,
}

pub struct ExecutionCostResult {
    pub payment_id: sui::types::Address,
    pub max_budget: u64,
    pub locked_budget: u64,
    pub consumed: u64,
    pub outstanding_locks: u64,
    pub accomplished: bool,
    pub refunded: bool,
}

pub struct WorkflowActions {
    pub(super) client: NexusClient,
}

fn event_execution_id(event: &NexusEventKind) -> Option<sui::types::Address> {
    match event {
        NexusEventKind::WalkAdvanced(e) => Some(e.execution.clone().into()),
        NexusEventKind::WalkFailed(e) => Some(e.execution.clone().into()),
        NexusEventKind::TerminalErrEvalRecorded(e) => Some(e.execution.clone().into()),
        NexusEventKind::WalkAborted(e) => Some(e.execution.clone().into()),
        NexusEventKind::WalkCancelled(e) => Some(e.execution.clone().into()),
        NexusEventKind::EndStateReached(e) => Some(e.execution.clone().into()),
        NexusEventKind::ExecutionFinished(e) => Some(e.execution.clone().into()),
        _ => None,
    }
}

#[cfg(feature = "walrus")]
fn terminal_state_from_execution_finished(
    execution_finished: &ExecutionFinishedEvent,
) -> WorkflowExecutionTerminalState {
    if execution_finished.was_aborted {
        WorkflowExecutionTerminalState::Aborted
    } else if execution_finished.has_any_walk_failed {
        WorkflowExecutionTerminalState::Failed
    } else if execution_finished.has_any_walk_succeeded {
        WorkflowExecutionTerminalState::Succeeded
    } else {
        WorkflowExecutionTerminalState::NoWalkOutcome
    }
}

#[cfg(feature = "walrus")]
async fn build_execution_completion_result(
    events: Vec<NexusEvent>,
    dag_execution_id: sui::types::Address,
    storage_conf: &StorageConf,
) -> Result<InspectExecutionCompletionResult, NexusError> {
    let mut end_states = Vec::new();
    let mut terminal_err_eval_recordings = Vec::new();
    let mut execution_finished = None;

    for event in &events {
        match &event.data {
            NexusEventKind::EndStateReached(end_state) => {
                let resolved_ports_to_data = end_state
                    .variant_ports_to_data
                    .clone()
                    .fetch_all(storage_conf)
                    .await
                    .map_err(|e| {
                        NexusError::Storage(anyhow!(
                            "Failed to fetch output data for execution '{dag_execution_id}': {e}"
                        ))
                    })?
                    .into_map();

                end_states.push(ResolvedEndState {
                    event: end_state.clone(),
                    resolved_ports_to_data,
                });
            }
            NexusEventKind::TerminalErrEvalRecorded(recorded) => {
                terminal_err_eval_recordings.push(recorded.clone());
            }
            NexusEventKind::ExecutionFinished(finished) => {
                execution_finished = Some(finished.clone());
            }
            _ => {}
        }
    }

    let execution_finished = execution_finished.ok_or_else(|| {
        NexusError::Channel(anyhow!(
            "ExecutionFinished event not found while inspecting DAG execution '{dag_execution_id}'"
        ))
    })?;

    Ok(InspectExecutionCompletionResult {
        terminal_state: terminal_state_from_execution_finished(&execution_finished),
        execution_finished,
        end_states,
        terminal_err_eval_recordings,
        events,
    })
}

pub fn verifier_mode_requires_proof(mode: VerifierMode) -> bool {
    matches!(
        mode,
        VerifierMode::LeaderRegisteredKey | VerifierMode::ToolVerifierContract
    )
}

pub fn effective_verifier_config(
    dag_default: &VerifierConfig,
    vertex_override: Option<&VerifierConfig>,
) -> VerifierConfig {
    vertex_override
        .cloned()
        .unwrap_or_else(|| dag_default.clone())
}

pub fn dag_vertex_requires_verifier_proof(
    dag: &dag_move::DAG,
    vertex: &graph_move::VertexInfo,
) -> bool {
    verifier_mode_requires_proof(
        effective_verifier_config(&dag.leader_verifier, vertex.leader_verifier.as_option()).mode,
    ) || verifier_mode_requires_proof(
        effective_verifier_config(&dag.tool_verifier, vertex.tool_verifier.as_option()).mode,
    )
}

pub async fn fetch_dag_vertices_bcs(
    crawler: &Crawler,
    dag: &dag_move::DAG,
) -> anyhow::Result<HashMap<graph_move::Vertex, graph_move::VertexInfo>> {
    Ok(crawler
        .get_dynamic_fields::<
            graph_move::Vertex,
            linked_table::Node<graph_move::Vertex, graph_move::VertexInfo>,
        >(dag.vertices.id(), dag.vertices.size())
        .await?
        .into_iter()
        .map(|(vertex, node)| (vertex, node.value))
        .collect())
}

/// Fetch the committed result for one walk from `DAGExecution` dynamic fields.
///
/// Returns `Ok(None)` when `CommittedToolResultKey { walk_index }` is absent.
pub async fn fetch_committed_tool_result_for_walk(
    crawler: &Crawler,
    execution_id: sui::types::Address,
    walk_index: u64,
) -> anyhow::Result<Option<CommittedToolResultView>> {
    crawler
        .get_optional_dynamic_field_matching_value_type::<
            execution_move::CommittedToolResultKey,
            execution_move::CommittedToolResult,
        >(
            execution_id,
            execution_move::CommittedToolResultKey { walk_index },
            Some(COMMITTED_TOOL_RESULT_VALUE_TYPE_SUFFIX),
        )
        .await
        .map(|value| value.map(CommittedToolResultView::from))
}

pub async fn inspect_expired_walk_resolution(
    crawler: &Crawler,
    objects: &NexusObjects,
    params: ResolveExpiredWalkParams,
) -> anyhow::Result<ExpiredWalkResolutionPlan> {
    let clock = crawler
        .get_object::<SuiClock>(move_boundary::CLOCK_OBJECT_ID)
        .await?
        .data;
    inspect_expired_walk_resolution_at(crawler, objects, params, clock.timestamp_ms).await
}

pub async fn inspect_expired_walk_resolution_at(
    crawler: &Crawler,
    objects: &NexusObjects,
    params: ResolveExpiredWalkParams,
    clock_ms: u64,
) -> anyhow::Result<ExpiredWalkResolutionPlan> {
    let execution = crawler
        .get_object::<DAGExecution>(params.dag_execution_id)
        .await?
        .data;
    let Some(walk) = usize::try_from(params.walk_index)
        .ok()
        .and_then(|index| execution.walks.get(index))
    else {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id(),
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Skipped {
                reason: "walk index no longer exists in execution".to_string(),
            },
        });
    };

    let Some(timeout_vertex) = walk.timeout_expired_vertex(clock_ms) else {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id(),
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Skipped {
                reason: unresolved_timeout_skip_reason(walk).to_string(),
            },
        });
    };

    match fetch_onchain_tool_result_state_for_walk(
        crawler,
        params.dag_execution_id,
        params.walk_index,
    )
    .await?
    {
        OnchainToolResultState::Committed { .. } => {
            return Ok(ExpiredWalkResolutionPlan {
                dag_id: execution.dag_id(),
                dag_execution_id: params.dag_execution_id,
                walk_index: params.walk_index,
                kind: ExpiredWalkResolutionKind::Settled,
            });
        }
        OnchainToolResultState::Finalized { result, object_ref } => {
            let dag = crawler
                .get_object::<dag_move::DAG>(execution.dag_id())
                .await?;
            let vertices = fetch_dag_vertices_bcs(crawler, &dag.data).await?;
            let kind = finalized_onchain_result_resolution_kind(
                crawler,
                objects,
                &vertices,
                timeout_vertex.clone(),
                &result,
                object_ref,
            )
            .await?;
            return Ok(ExpiredWalkResolutionPlan {
                dag_id: execution.dag_id(),
                dag_execution_id: params.dag_execution_id,
                walk_index: params.walk_index,
                kind,
            });
        }
        OnchainToolResultState::InsufficientSettlement { .. } => {
            return Ok(ExpiredWalkResolutionPlan {
                dag_id: execution.dag_id(),
                dag_execution_id: params.dag_execution_id,
                walk_index: params.walk_index,
                kind: ExpiredWalkResolutionKind::Skipped {
                    reason: "walk has committed result with insufficient settlement".to_string(),
                },
            });
        }
        OnchainToolResultState::InvalidEmpty {
            result_id,
            object_ref,
        } => {
            return Ok(ExpiredWalkResolutionPlan {
                dag_id: execution.dag_id(),
                dag_execution_id: params.dag_execution_id,
                walk_index: params.walk_index,
                kind: ExpiredWalkResolutionKind::Skipped {
                    reason: format!(
                        "stored on-chain tool result {result_id}@{} is not finalized",
                        object_ref.version()
                    ),
                },
            });
        }
        OnchainToolResultState::NoResult => {}
    }

    let Some(abort_vertex) = walk.abortable_timeout_expired_vertex(clock_ms) else {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id(),
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Skipped {
                reason: format!(
                    "timeout-expired walk at vertex '{}' is not abortable without a committed result",
                    timeout_vertex.vertex_name()
                ),
            },
        });
    };

    let payment = tap::fetch_execution_payment_for_execution(crawler, params.dag_execution_id)
        .await?
        .data;
    let dag = crawler
        .get_object::<dag_move::DAG>(execution.dag_id())
        .await?;
    let vertices = fetch_dag_vertices_bcs(crawler, &dag.data).await?;
    let vertex_info = vertices.get(abort_vertex.vertex()).ok_or_else(|| {
        anyhow!(
            "DAG vertex '{}' missing from fetched DAG",
            abort_vertex.vertex_name()
        )
    })?;
    let tool_fqn = vertex_info.kind.tool_fqn()?;
    let vertex_key = payment_vertex_key(params.dag_execution_id, abort_vertex, &tool_fqn)?;
    let tool_fqn_bytes = tool_fqn.to_string().into_bytes();
    let locked = payment
        .locked_vertices
        .iter()
        .any(|lock| lock.vertex_key == vertex_key && lock.tool_fqn == tool_fqn_bytes);

    if !locked {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id(),
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Aborted,
        });
    }

    let candidate_walk = ToolGasAbortCandidateWalk {
        walk_index: usize::try_from(params.walk_index)?,
        vertex: abort_vertex.clone(),
        payment_vertex_key: vertex_key,
    };
    let candidates = fetch_tool_gas_refs_for_abort_candidates(
        crawler,
        *objects.gas_service.object_id(),
        HashMap::from([(tool_fqn, vec![candidate_walk])]),
    )
    .await?;
    let selected_candidate = select_tool_gas_abort_candidate(candidates, params.tool_gas_id)?;

    Ok(ExpiredWalkResolutionPlan {
        dag_id: execution.dag_id(),
        dag_execution_id: params.dag_execution_id,
        walk_index: params.walk_index,
        kind: ExpiredWalkResolutionKind::AbortedWithToolGas { selected_candidate },
    })
}

fn unresolved_timeout_skip_reason(walk: &DAGWalk) -> &'static str {
    match walk {
        DAGWalk::Active { .. } | DAGWalk::PendingSettlement { .. } => {
            EXPIRED_WALK_NOT_DOUBLE_TIMEOUT_EXPIRED_REASON
        }
        _ => EXPIRED_WALK_ALREADY_TERMINAL_REASON,
    }
}

async fn finalized_onchain_result_resolution_kind(
    crawler: &Crawler,
    objects: &NexusObjects,
    vertices: &HashMap<graph_move::Vertex, graph_move::VertexInfo>,
    timeout_vertex: RuntimeVertex,
    result: &OnchainToolResult,
    object_ref: sui::types::ObjectReference,
) -> anyhow::Result<ExpiredWalkResolutionKind> {
    let vertex_info = vertices.get(timeout_vertex.vertex()).ok_or_else(|| {
        anyhow!(
            "DAG vertex '{}' missing from fetched DAG",
            timeout_vertex.vertex_name()
        )
    })?;
    let tool_fqn = vertex_info.kind.tool_fqn()?;
    let tool_id =
        crate::move_bindings::derive_tool_id(*objects.tool_registry.object_id(), &tool_fqn)?;
    let tool = crawler.get_object::<Tool>(tool_id).await?.data;
    let ToolRef::Sui {
        tool_witness_id, ..
    } = tool.reference()
    else {
        return Ok(ExpiredWalkResolutionKind::Skipped {
            reason: format!(
                "finalized on-chain result exists for non-Sui tool vertex '{}'",
                timeout_vertex.vertex_name()
            ),
        });
    };
    let finalize_tx_digest = result
        .finalize_tx_digest
        .as_option()
        .cloned()
        .ok_or_else(|| anyhow!("finalized on-chain result is missing finalize_tx_digest"))?;

    Ok(ExpiredWalkResolutionKind::SettledOnchainResult {
        result_ref: object_ref,
        expected_vertex: timeout_vertex,
        tool_witness_id: tool_witness_id.bytes,
        finalize_tx_digest,
    })
}

async fn broken_onchain_result_cleanups_for_abort(
    crawler: &Crawler,
    objects: &NexusObjects,
    execution_id: sui::types::Address,
    execution: &DAGExecution,
    clock_ms: u64,
) -> anyhow::Result<Vec<BrokenOnchainToolResultCleanup>> {
    let mut vertices = None;
    let mut cleanups = Vec::new();

    for (walk_index, walk) in execution.walks.iter().enumerate() {
        let Some(timeout_vertex) = walk.abortable_timeout_expired_vertex(clock_ms) else {
            continue;
        };
        let walk_index = u64::try_from(walk_index)?;
        match fetch_onchain_tool_result_state_for_walk(crawler, execution_id, walk_index).await? {
            OnchainToolResultState::Finalized { result, object_ref } => {
                if vertices.is_none() {
                    let dag = crawler
                        .get_object::<dag_move::DAG>(execution.dag_id())
                        .await?;
                    vertices = Some(fetch_dag_vertices_bcs(crawler, &dag.data).await?);
                }
                let vertices = vertices.as_ref().expect("vertices were just fetched");
                let kind = finalized_onchain_result_resolution_kind(
                    crawler,
                    objects,
                    vertices,
                    timeout_vertex.clone(),
                    &result,
                    object_ref.clone(),
                )
                .await?;
                let ExpiredWalkResolutionKind::SettledOnchainResult {
                    expected_vertex,
                    tool_witness_id,
                    ..
                } = kind
                else {
                    return Err(anyhow!(
                        "expired abort is blocked by finalized on-chain result for walk {walk_index} that cannot be cleaned by the Sui-tool cleanup path"
                    ));
                };
                if onchain_tool_result_has_required_stamps(
                    &result,
                    execution_id,
                    *objects.leader_registry.object_id(),
                    tool_witness_id,
                ) {
                    return Err(anyhow!(
                        "expired abort is blocked by consumable on-chain result for walk {walk_index}; settle the on-chain result before aborting"
                    ));
                }
                cleanups.push(BrokenOnchainToolResultCleanup {
                    walk_index,
                    result_ref: object_ref,
                    expected_vertex,
                    tool_witness_id,
                });
            }
            OnchainToolResultState::InvalidEmpty { result_id, .. } => {
                return Err(anyhow!(
                    "expired abort is blocked by unfinalized on-chain result {result_id} for walk {walk_index}"
                ));
            }
            OnchainToolResultState::NoResult
            | OnchainToolResultState::Committed { .. }
            | OnchainToolResultState::InsufficientSettlement { .. } => {}
        }
    }

    Ok(cleanups)
}

fn onchain_tool_result_has_required_stamps(
    result: &OnchainToolResult,
    execution_id: sui::types::Address,
    leader_registry_id: sui::types::Address,
    tool_witness_id: sui::types::Address,
) -> bool {
    let Some(stamps) = result.stamps.as_option() else {
        return false;
    };
    let execution_id = ID::new(execution_id);
    let leader_registry_id = ID::new(leader_registry_id);
    let tool_witness_id = ID::new(tool_witness_id);

    stamps
        .contents
        .iter()
        .any(|entry| entry.key == execution_id)
        && stamps
            .contents
            .iter()
            .any(|entry| entry.key == leader_registry_id)
        && stamps
            .contents
            .iter()
            .any(|entry| entry.key == tool_witness_id)
}

pub async fn fetch_onchain_tool_result_state_for_walk(
    crawler: &Crawler,
    execution_id: sui::types::Address,
    walk_index: u64,
) -> anyhow::Result<OnchainToolResultState> {
    let committed_result =
        fetch_committed_tool_result_for_walk(crawler, execution_id, walk_index).await?;
    let insufficient_settlement = crawler
        .get_optional_dynamic_field_matching_value_type::<
            execution_move::ExecutionPaymentInsufficientSettlementFieldKey,
            execution_move::ExecutionPaymentInsufficientSettlement,
        >(
            execution_id,
            insufficient_settlement_field_key(),
            Some(EXECUTION_PAYMENT_INSUFFICIENT_SETTLEMENT_VALUE_TYPE_SUFFIX),
        )
        .await?;

    if let Some(marker) = insufficient_settlement {
        if marker.walks.contains(&walk_index) {
            return Ok(OnchainToolResultState::InsufficientSettlement {
                committed_result,
                marker: InsufficientSettlementMarkerView {
                    walks: marker.walks,
                },
            });
        }
    }

    if let Some(committed_result) = committed_result {
        return Ok(OnchainToolResultState::Committed { committed_result });
    }

    let result_id = crawler
        .get_optional_dynamic_field_matching_value_type::<execution_move::OnchainToolResultKey, ID>(
            execution_id,
            execution_move::OnchainToolResultKey { walk_index },
            Some(ONCHAIN_TOOL_RESULT_ID_VALUE_TYPE_SUFFIX),
        )
        .await?;
    let Some(result_id) = result_id else {
        return Ok(OnchainToolResultState::NoResult);
    };

    let result_id = result_id.bytes;
    let result_response = crawler.get_object::<OnchainToolResult>(result_id).await?;
    let object_ref = result_response.object_ref();
    let result = result_response.data;

    if onchain_tool_result_is_finalized(&result) {
        Ok(OnchainToolResultState::Finalized { result, object_ref })
    } else {
        Ok(OnchainToolResultState::InvalidEmpty {
            result_id,
            object_ref,
        })
    }
}

fn insufficient_settlement_field_key(
) -> execution_move::ExecutionPaymentInsufficientSettlementFieldKey {
    execution_move::ExecutionPaymentInsufficientSettlementFieldKey { dummy_field: false }
}

pub async fn fetch_dag_default_values_bcs<T>(
    crawler: &Crawler,
    dag: &dag_move::DAG,
) -> anyhow::Result<HashMap<graph_move::VertexInputPort, T>>
where
    T: serde::de::DeserializeOwned,
{
    crawler
        .get_dynamic_fields::<graph_move::VertexInputPort, T>(
            dag.defaults_to_input_ports.id(),
            dag.defaults_to_input_ports.size(),
        )
        .await
}

pub async fn fetch_dag_edges_bcs(
    crawler: &Crawler,
    dag: &dag_move::DAG,
) -> anyhow::Result<HashMap<graph_move::Vertex, Vec<graph_move::Edge>>> {
    crawler
        .get_dynamic_fields::<graph_move::Vertex, Vec<graph_move::Edge>>(
            dag.edges.id(),
            dag.edges.size(),
        )
        .await
}

pub async fn fetch_dag_outputs_bcs(
    crawler: &Crawler,
    dag: &dag_move::DAG,
) -> anyhow::Result<HashMap<graph_move::Vertex, Vec<graph_move::OutputVariantPort>>> {
    crawler
        .get_dynamic_fields::<graph_move::Vertex, Vec<graph_move::OutputVariantPort>>(
            dag.outputs.id(),
            dag.outputs.size(),
        )
        .await
}

pub async fn offchain_success_requires_verifier_proof(
    crawler: &Crawler,
    dag_object_id: sui::types::Address,
    next_vertex: &RuntimeVertex,
) -> anyhow::Result<bool> {
    let dag = crawler.get_object::<dag_move::DAG>(dag_object_id).await?;
    let mut vertices = fetch_dag_vertices_bcs(crawler, &dag.data).await?;
    let vertex_name = next_vertex.vertex();
    let vertex = vertices.remove(&vertex_name).ok_or_else(|| {
        anyhow!(
            "Vertex '{}' not found in DAG verifier config",
            vertex_name.name
        )
    })?;

    Ok(dag_vertex_requires_verifier_proof(&dag.data, &vertex))
}

pub async fn fetch_vertex_input_port_names(
    crawler: &Crawler,
    dag: &dag_move::DAG,
    vertex_name: &TypeName,
) -> anyhow::Result<Vec<String>> {
    let mut vertices = fetch_dag_vertices_bcs(crawler, dag).await?;
    let vertex_key = graph_move::Vertex::from(vertex_name);
    let vertex = vertices.remove(&vertex_key).ok_or_else(|| {
        anyhow!("Vertex '{vertex_name}' not found in DAG vertices dynamic fields")
    })?;

    Ok(vertex.declared_input_port_names())
}

pub fn execution_terminal_record_matches_retryable_vertex(
    terminal_records: &VecMap<u64, execution_move::TerminalErrEvalRecord>,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
) -> bool {
    terminal_records.get(&walk_index).is_some_and(|record| {
        &record.vertex == next_vertex && record.failure_class == WorkflowFailureClass::Retryable
    })
}

pub async fn should_settle_tool_err_eval_gas(
    crawler: &Crawler,
    execution: sui::types::Address,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
) -> anyhow::Result<bool> {
    let execution = crawler.get_object::<DAGExecution>(execution).await?;
    Ok(execution_terminal_record_matches_retryable_vertex(
        &execution.data.terminal_records,
        walk_index,
        next_vertex,
    ))
}

impl WorkflowActions {
    /// Publish the provided [`DagSpec`] specification.
    pub async fn publish(&self, dag_spec: DagSpec) -> Result<PublishResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        // == Craft and submit the publish DAG transaction ==

        let tx =
            dag::publish_ptb(nexus_objects, dag_spec).map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

        // == Find the published DAG object ID ==

        let dag_tag = crate::move_bindings::struct_tag::<dag_move::DAG>(nexus_objects);
        let dag_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if *object_type.address() == *dag_tag.address()
                    && object_type.module() == dag_tag.module()
                    && object_type.name() == dag_tag.name()
                {
                    Some(obj.object_id())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!("DAG object ID not found in TX response"))
            })?;

        Ok(PublishResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_object_id,
        })
    }

    /// Execute a published DAG through the configured standard default agent.
    ///
    /// The `entry_data` [`HashMap`] already holds information about the storage
    /// kind for each port.
    ///
    /// `storage_conf` can accept [`StorageConf::default`] if no remote storage
    /// is expected.
    ///
    /// `priority_fee_per_gas_unit` is the per-transaction priority fee to pass
    /// down to the DAG execution.
    ///
    /// Use [`WorkflowActions::inspect_execution`] to monitor the execution.
    #[cfg(feature = "walrus")]
    pub async fn execute(
        &self,
        dag_object_id: sui::types::Address,
        entry_data: HashMap<String, VecMap<InputPort, NexusData>>,
        priority_fee_per_gas_unit: u64,
        entry_group: Option<&str>,
        storage_conf: &StorageConf,
    ) -> Result<ExecuteResult, NexusError> {
        let address = self.client.signer.get_active_address();
        self.execute_default_agent_dag(
            dag_object_id,
            entry_data,
            priority_fee_per_gas_unit,
            entry_group,
            storage_conf,
            AgentDagExecuteOptions {
                payment_source: payment_source_from_address(address)
                    .map_err(NexusError::TransactionBuilding)?,
                payment_coin: None,
                payment_coin_balance: None,
                payment_max_budget: self.client.gas.get_budget(),
            },
        )
        .await
    }

    /// Execute a published DAG through the configured standard default agent
    /// with explicit standard payment options.
    #[cfg(feature = "walrus")]
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_default_agent_dag(
        &self,
        dag_object_id: sui::types::Address,
        entry_data: HashMap<String, VecMap<InputPort, NexusData>>,
        priority_fee_per_gas_unit: u64,
        entry_group: Option<&str>,
        storage_conf: &StorageConf,
        options: AgentDagExecuteOptions,
    ) -> Result<ExecuteResult, NexusError> {
        // == Commit data to their respective storage ==

        let mut input_data = HashMap::new();

        for (vertex, ports_data) in entry_data {
            let committed_data = ports_data.commit_all(storage_conf).await.map_err(|e| {
                NexusError::Storage(anyhow!("Failed to commit data for port '{vertex}': {e}"))
            })?;

            input_data.insert(vertex, committed_data);
        }

        // == Craft and submit the execute DAG transaction ==

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let dag = self
            .client
            .crawler()
            .get_object::<dag_move::DAG>(dag_object_id)
            .await
            .map_err(NexusError::Rpc)?;

        let tools_gas = self.client.fetch_tool_gas_for_dag(&dag.data).await?;

        let registry = tap::fetch_configured_agent_registry(self.client.crawler(), nexus_objects)
            .await
            .map_err(NexusError::Rpc)?;
        let default_executor = resolve_default_agent_dag_executor(nexus_objects, &registry.data)
            .map_err(NexusError::Parsing)?;

        validate_execution_payment_options(
            default_executor.target.agent_id,
            &default_executor.skill_revision.requirements.payment_policy,
            &options.payment_source,
            options.payment_max_budget,
            address,
        )
        .map_err(NexusError::TransactionBuilding)?;
        if let Some(balance) = options.payment_coin_balance {
            if balance < options.payment_max_budget {
                return Err(NexusError::TransactionBuilding(anyhow!(
                    "TAP execution payment coin balance {balance} is below requested budget {}",
                    options.payment_max_budget
                )));
            }
        }
        let agent_execution = dag::AgentDagExecuteInput {
            agent_id: default_executor.target.agent_id,
            skill_id: default_executor.target.skill_id,
            selected_dag: None,
            authorization_templates: Vec::new(),
            payment_source: options.payment_source,
            payment_coin: options.payment_coin,
            payment_coin_balance: options.payment_coin_balance,
            payment_max_budget: options.payment_max_budget,
        };

        let transaction_input_data = input_data
            .clone()
            .into_iter()
            .map(|(vertex, data)| (vertex, data.into_map()))
            .collect();
        let owned_payment_coin = agent_execution
            .payment_coin
            .as_ref()
            .map(|payment_coin| *payment_coin.object_id());

        let tx = dag::execute_default_agent_dag_ptb(
            nexus_objects,
            &dag.object_ref(),
            priority_fee_per_gas_unit,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &transaction_input_data,
            &agent_execution,
            &tools_gas,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;
        if let Some(payment_coin_id) = owned_payment_coin {
            if let Some(updated_payment_coin) = response
                .objects
                .iter()
                .find(|object| object.object_id() == payment_coin_id)
            {
                let payment_gas_config = self.client.gas_config();
                payment_gas_config
                    .release_gas_coin(sui::types::ObjectReference::new(
                        updated_payment_coin.object_id(),
                        updated_payment_coin.version(),
                        updated_payment_coin.digest(),
                    ))
                    .await;
            }
        }

        // == Find the created DAG execution object ID ==

        let execution_tag =
            crate::move_bindings::struct_tag::<execution_move::DAGExecution>(nexus_objects);
        let execution_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if nexus_objects.is_workflow_package(*object_type.address())
                    && object_type.module() == execution_tag.module()
                    && object_type.name() == execution_tag.name()
                {
                    Some(obj.object_id())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!("DAG execution object ID not found in TX response"))
            })?;

        Ok(ExecuteResult {
            tx_digest: response.digest,
            execution_object_id,
            tx_checkpoint: response.checkpoint,
            tap_execution: Some(TapExecutionSubmitMetadata {
                agent_id: default_executor.target.agent_id,
                skill_id: default_executor.target.skill_id,
                dag_id: dag.object_id,
                skill_revision_key: default_executor.skill_revision.key,
                payment_max_budget: options.payment_max_budget,
            }),
        })
    }

    /// Execute the active agent skill for `(agent_id, skill_id)`.
    ///
    /// This resolves the registered DAG from the configured TAP registry, then
    /// calls the explicit agent workflow entry.
    #[cfg(feature = "walrus")]
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_agent_dag(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        entry_data: HashMap<String, VecMap<InputPort, NexusData>>,
        priority_fee_per_gas_unit: u64,
        entry_group: Option<&str>,
        storage_conf: &StorageConf,
        options: AgentDagExecuteOptions,
    ) -> Result<ExecuteResult, NexusError> {
        let mut input_data = HashMap::new();

        for (vertex, ports_data) in entry_data {
            let committed_data = ports_data.commit_all(storage_conf).await.map_err(|e| {
                NexusError::Storage(anyhow!("Failed to commit data for port '{vertex}': {e}"))
            })?;

            input_data.insert(vertex, committed_data);
        }

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let target = tap::fetch_configured_active_tap_skill_execution_target(
            self.client.crawler(),
            nexus_objects,
            agent_id,
            skill_id,
        )
        .await
        .map_err(NexusError::Rpc)?
        .data;

        let dag_id = match target.skill.dag_binding() {
            SkillDagBinding::Pinned { dag_id } => *dag_id,
            SkillDagBinding::RuntimeSelected => {
                return Err(NexusError::Parsing(anyhow!(
                    "runtime-selected skill {agent_id}:{skill_id} requires an explicit DAG selection; use WorkflowActions::execute or `nexus dag execute`"
                )));
            }
        };

        let dag = self
            .client
            .crawler()
            .get_object::<dag_move::DAG>(dag_id)
            .await
            .map_err(NexusError::Rpc)?;

        let tools_gas = self.client.fetch_tool_gas_for_dag(&dag.data).await?;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?;

        validate_execution_payment_options(
            agent_id,
            &target.skill_revision.requirements.payment_policy,
            &options.payment_source,
            options.payment_max_budget,
            address,
        )
        .map_err(NexusError::TransactionBuilding)?;
        if let Some(balance) = options.payment_coin_balance {
            if balance < options.payment_max_budget {
                return Err(NexusError::TransactionBuilding(anyhow!(
                    "TAP execution payment coin balance {balance} is below requested budget {}",
                    options.payment_max_budget
                )));
            }
        }
        let agent_execution = dag::AgentDagExecuteInput {
            agent_id,
            skill_id,
            selected_dag: None,
            authorization_templates: Vec::new(),
            payment_source: options.payment_source,
            payment_coin: options.payment_coin,
            payment_coin_balance: options.payment_coin_balance,
            payment_max_budget: options.payment_max_budget,
        };

        let transaction_input_data = input_data
            .clone()
            .into_iter()
            .map(|(vertex, data)| (vertex, data.into_map()))
            .collect();
        let owned_payment_coin = agent_execution
            .payment_coin
            .as_ref()
            .map(|payment_coin| *payment_coin.object_id());

        let agent_input = tap::agent_input_from_metadata(&agent_object)
            .map_err(NexusError::TransactionBuilding)?;
        let tx = dag::execute_agent_dag_ptb(
            nexus_objects,
            &dag.object_ref(),
            agent_input,
            priority_fee_per_gas_unit,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &transaction_input_data,
            &agent_execution,
            &tools_gas,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;
        if let Some(payment_coin_id) = owned_payment_coin {
            if let Some(updated_payment_coin) = response
                .objects
                .iter()
                .find(|object| object.object_id() == payment_coin_id)
            {
                self.client
                    .gas
                    .release_gas_coin(sui::types::ObjectReference::new(
                        updated_payment_coin.object_id(),
                        updated_payment_coin.version(),
                        updated_payment_coin.digest(),
                    ))
                    .await;
            }
        }

        let execution_tag =
            crate::move_bindings::struct_tag::<execution_move::DAGExecution>(nexus_objects);
        let execution_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if nexus_objects.is_workflow_package(*object_type.address())
                    && object_type.module() == execution_tag.module()
                    && object_type.name() == execution_tag.name()
                {
                    Some(obj.object_id())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!("DAG execution object ID not found in TX response"))
            })?;

        Ok(ExecuteResult {
            tx_digest: response.digest,
            execution_object_id,
            tx_checkpoint: response.checkpoint,
            tap_execution: Some(TapExecutionSubmitMetadata {
                agent_id,
                skill_id,
                dag_id,
                skill_revision_key: target.skill_revision.key,
                payment_max_budget: options.payment_max_budget,
            }),
        })
    }

    /// Inspect a DAG execution given its shared object ID.
    ///
    /// The starting checkpoint is derived from the object's creation
    /// transaction via [`Crawler::get_object_creation_checkpoint`], so
    /// callers do not need to track it themselves. Channel sender drops
    /// once we observe an `ExecutionFinished` event or the timeout elapses.
    ///
    /// The poller task is also returned so that the user can ensure its
    /// completion.
    pub async fn inspect_execution(
        &self,
        dag_execution_id: sui::types::Address,
        timeout: Option<Duration>,
    ) -> Result<InspectExecutionResult, NexusError> {
        // Derive the checkpoint that contains the DAGExecution's creation
        // transaction so the poller catches up over the smallest possible
        // window without the caller having to plumb it through.
        let execution_checkpoint = self
            .client
            .crawler()
            .get_object_creation_checkpoint(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?;

        // Setup MSPC channel.
        let (tx, rx) = unbounded_channel::<NexusEvent>();

        // Create some initial timings and restrictions.
        let timeout = timeout.unwrap_or(Duration::from_secs(3600));
        let poller = self.client.event_poller().clone();
        let mut next_page = poller
            .start_polling(Some(execution_checkpoint))
            .map_err(|e| NexusError::Configuration(format!("{e}")))?;

        let poller = {
            tokio::spawn(async move {
                let timeout = tokio::time::sleep(timeout);

                tokio::pin!(timeout);

                loop {
                    tokio::select! {
                        maybe_page = next_page.recv() => {
                            let events = match maybe_page {
                                Some(Ok(EventPage { events, .. })) => events,
                                Some(Err(e)) => return Err(NexusError::Channel(anyhow!("Error fetching events: {}", e))),
                                None => return Err(NexusError::Channel(anyhow!("Event stream closed unexpectedly while inspecting DAG execution '{dag_execution_id}'"))),
                            };

                            let mut execution_finished_seen = false;

                            for event in events {
                                let Some(execution_id) = event_execution_id(&event.data) else {
                                    continue;
                                };

                                // Only process events for the given execution ID.
                                if execution_id != dag_execution_id {
                                    continue;
                                }

                                if matches!(&event.data, NexusEventKind::ExecutionFinished(_)) {
                                    tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;
                                    execution_finished_seen = true;
                                    continue;
                                }

                                tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;
                            }

                            if execution_finished_seen {
                                return Ok(());
                            }
                        }

                        _ = &mut timeout => {
                            return Err(NexusError::Timeout(anyhow!("Timeout {timeout:?} reached while inspecting DAG execution '{dag_execution_id}'")));
                        }
                    }
                }
            })
        };

        Ok(InspectExecutionResult {
            next_event: rx,
            poller,
        })
    }

    /// Inspect a DAG execution until completion and return a structured summary
    /// with resolved end-state data. The starting checkpoint is derived from
    /// the execution object's creation transaction; see
    /// [`Self::inspect_execution`] for details.
    #[cfg(feature = "walrus")]
    pub async fn inspect_execution_until_completion(
        &self,
        dag_execution_id: sui::types::Address,
        timeout: Option<Duration>,
        storage_conf: &StorageConf,
    ) -> Result<InspectExecutionCompletionResult, NexusError> {
        let mut inspection = self.inspect_execution(dag_execution_id, timeout).await?;

        let mut events = Vec::new();

        while let Some(event) = inspection.next_event.recv().await {
            events.push(event);
        }

        let poller_result = inspection.poller.await.map_err(|e| {
            NexusError::Channel(anyhow!(
                "Execution inspection task failed for DAG execution '{dag_execution_id}': {e}"
            ))
        })?;
        poller_result?;

        build_execution_completion_result(events, dag_execution_id, storage_conf).await
    }

    /// Fetch the TAP execution payment cost summary for a DAG
    /// execution.
    pub async fn execution_cost(
        &self,
        dag_execution_id: sui::types::Address,
    ) -> Result<ExecutionCostResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DAGExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        if execution
            .to_context()
            .map_err(NexusError::Parsing)?
            .is_none()
        {
            return Err(NexusError::Parsing(anyhow!(
                "DAG execution '{dag_execution_id}' has no TAP payment context"
            )));
        }
        let payment = tap::fetch_execution_payment_for_execution(crawler, dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;

        Ok(ExecutionCostResult::from_payment(payment))
    }

    /// Submit the current permissionless expired-execution abort entry.
    ///
    /// This wraps `execution_settlement::abort_expired_execution`; it does not
    /// discover or submit ToolGas candidates.
    ///
    /// If a double-expired active walk is blocked by a finalized on-chain
    /// result whose required stamps are insufficient, this prepends the
    /// self-validating broken-result cleanup call before aborting.
    pub async fn abort_expired_execution(
        &self,
        dag_execution_id: sui::types::Address,
    ) -> Result<AbortExecutionResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DAGExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let clock = crawler
            .get_object::<SuiClock>(move_boundary::CLOCK_OBJECT_ID)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let cleaned_broken_onchain_results = broken_onchain_result_cleanups_for_abort(
            crawler,
            &self.client.nexus_objects,
            dag_execution_id,
            &execution,
            clock.timestamp_ms,
        )
        .await
        .map_err(NexusError::TransactionBuilding)?;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id())
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let execution_ref = crawler
            .get_object_metadata(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let address = self.client.signer.get_active_address();
        let tx = dag::abort_expired_execution_for_self_ptb(
            &self.client.nexus_objects,
            &dag_ref,
            &execution_ref,
            &cleaned_broken_onchain_results
                .iter()
                .map(|cleanup| dag::BrokenOnchainToolResultCleanupInput {
                    walk_index: cleanup.walk_index,
                    result_ref: cleanup.result_ref.clone(),
                    tool_witness_id: cleanup.tool_witness_id,
                })
                .collect::<Vec<_>>(),
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(AbortExecutionResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id(),
            dag_execution_id,
            cleaned_broken_onchain_results,
        })
    }

    /// Submit permissionless committed-result settlement for one walk.
    pub async fn settle_committed_tool_result_for_walk(
        &self,
        params: SettleCommittedToolResultParams,
    ) -> Result<CommittedToolResultSettlementResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DAGExecution>(params.dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id())
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let execution_ref = crawler
            .get_object_metadata(params.dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;
        let tx = dag::settle_committed_tool_result_for_walk_for_self_ptb(
            objects,
            &dag_ref,
            &execution_ref,
            params.walk_index,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(CommittedToolResultSettlementResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id(),
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
        })
    }

    /// Submit leader authenticated committed result settlement with the leader commit gas charge.
    pub async fn settle_committed_tool_result_for_walk_by_leader(
        &self,
        params: SettleCommittedToolResultByLeaderParams,
    ) -> Result<CommittedToolResultSettlementResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DAGExecution>(params.dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id())
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let execution_ref = crawler
            .get_object_metadata(params.dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?;
        let leader_cap_ref = crawler
            .get_object_metadata(params.leader_cap_id)
            .await
            .map_err(NexusError::Rpc)?;

        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;
        let tx = dag::settle_committed_tool_result_for_walk_by_leader_for_self_ptb(
            objects,
            &dag_ref,
            &execution_ref.object_ref(),
            &execution_ref.owner,
            &leader_cap_ref.object_ref(),
            &leader_cap_ref.owner,
            params.walk_index,
            params.commit_tx_digest,
            params.commit_gas_charge,
            params.settlement_gas_charge,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(CommittedToolResultSettlementResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id(),
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
        })
    }

    /// Record a leader commit gas charge without settling the walk.
    pub async fn record_committed_tool_result_gas_charge_by_leader(
        &self,
        params: RecordCommittedToolResultGasChargeParams,
    ) -> Result<RecordCommittedToolResultGasChargeResult, NexusError> {
        let crawler = self.client.crawler();
        let execution_ref = crawler
            .get_object_metadata(params.dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?;
        let leader_cap_ref = crawler
            .get_object_metadata(params.leader_cap_id)
            .await
            .map_err(NexusError::Rpc)?;

        let address = self.client.signer.get_active_address();
        let tx = dag::record_committed_tool_result_gas_charge_by_leader_for_self_ptb(
            &self.client.nexus_objects,
            &execution_ref.object_ref(),
            &execution_ref.owner,
            &leader_cap_ref.object_ref(),
            &leader_cap_ref.owner,
            params.walk_index,
            params.commit_tx_digest,
            params.commit_gas_charge,
            params.settlement_gas_charge,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(RecordCommittedToolResultGasChargeResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_execution_id: params.dag_execution_id,
            leader_cap_id: params.leader_cap_id,
            walk_index: params.walk_index,
        })
    }

    /// Classify what the SDK would submit for one expired walk without sending it.
    pub async fn inspect_expired_walk_resolution(
        &self,
        params: ResolveExpiredWalkParams,
    ) -> Result<ExpiredWalkResolutionPlan, NexusError> {
        inspect_expired_walk_resolution(self.client.crawler(), &self.client.nexus_objects, params)
            .await
            .map_err(NexusError::Rpc)
    }

    /// Classify and submit the existing Move entry that matches one expired walk.
    pub async fn resolve_expired_walk(
        &self,
        params: ResolveExpiredWalkParams,
    ) -> Result<ExpiredWalkResolutionResult, NexusError> {
        let plan = self.inspect_expired_walk_resolution(params).await?;
        let base = |resolution_kind| ExpiredWalkResolutionResult {
            tx_digest: None,
            tx_checkpoint: None,
            dag_id: plan.dag_id,
            dag_execution_id: plan.dag_execution_id,
            walk_index: plan.walk_index,
            resolution_kind,
        };

        match plan.kind {
            ExpiredWalkResolutionKind::Settled => {
                let settled = self
                    .settle_committed_tool_result_for_walk(SettleCommittedToolResultParams {
                        dag_execution_id: plan.dag_execution_id,
                        walk_index: plan.walk_index,
                    })
                    .await?;
                Ok(ExpiredWalkResolutionResult {
                    tx_digest: Some(settled.tx_digest),
                    tx_checkpoint: Some(settled.tx_checkpoint),
                    ..base(ExpiredWalkResolutionKind::Settled)
                })
            }
            ExpiredWalkResolutionKind::SettledOnchainResult {
                result_ref,
                expected_vertex,
                tool_witness_id,
                finalize_tx_digest,
            } => {
                let crawler = self.client.crawler();
                let dag_ref = crawler
                    .get_object_metadata(plan.dag_id)
                    .await
                    .map_err(NexusError::Rpc)?
                    .object_ref();
                let execution_ref = crawler
                    .get_object_metadata(plan.dag_execution_id)
                    .await
                    .map_err(NexusError::Rpc)?
                    .object_ref();
                let address = self.client.signer.get_active_address();
                let objects = &self.client.nexus_objects;
                let tx = move_boundary::ptb(objects, |tx| {
                    let dag = tx.shared_object(&dag_ref, false)?;
                    let execution = tx.shared_object(&execution_ref, true)?;
                    let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
                    let result = tx.shared_object(&result_ref, true)?;
                    let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
                    let clock = tx.clock()?;
                    dag::settle_onchain_tool_result_for_walk(
                        tx,
                        dag,
                        execution,
                        tool_registry,
                        result,
                        leader_registry,
                        plan.walk_index,
                        &expected_vertex,
                        tool_witness_id,
                        clock,
                    )
                })
                .map_err(NexusError::TransactionBuilding)?;
                let response = self.client.submit_transaction(tx, address).await?;
                Ok(ExpiredWalkResolutionResult {
                    tx_digest: Some(response.digest),
                    tx_checkpoint: Some(response.checkpoint),
                    ..base(ExpiredWalkResolutionKind::SettledOnchainResult {
                        result_ref,
                        expected_vertex,
                        tool_witness_id,
                        finalize_tx_digest,
                    })
                })
            }
            ExpiredWalkResolutionKind::Aborted => {
                let aborted = self.abort_expired_execution(plan.dag_execution_id).await?;
                Ok(ExpiredWalkResolutionResult {
                    tx_digest: Some(aborted.tx_digest),
                    tx_checkpoint: Some(aborted.tx_checkpoint),
                    ..base(ExpiredWalkResolutionKind::Aborted)
                })
            }
            ExpiredWalkResolutionKind::AbortedWithToolGas { selected_candidate } => {
                let tool_gas_id = *selected_candidate.tool_gas_ref.object_id();
                let aborted = self
                    .abort_expired_execution_with_tool_gas(plan.dag_execution_id, Some(tool_gas_id))
                    .await?;
                Ok(ExpiredWalkResolutionResult {
                    tx_digest: Some(aborted.tx_digest),
                    tx_checkpoint: Some(aborted.tx_checkpoint),
                    ..base(ExpiredWalkResolutionKind::AbortedWithToolGas { selected_candidate })
                })
            }
            ExpiredWalkResolutionKind::Skipped { reason } => {
                Ok(base(ExpiredWalkResolutionKind::Skipped { reason }))
            }
        }
    }

    /// Return ToolGas refs that can be passed to
    /// `gas_extension::abort_expired_execution_with_tool_gas` for the current
    /// execution state. This is an advisory snapshot; Move still verifies
    /// timeout and lock state on chain.
    pub async fn abort_expired_execution_tool_gas_candidates(
        &self,
        dag_execution_id: sui::types::Address,
    ) -> Result<Vec<ToolGasAbortCandidate>, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DAGExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag = crawler
            .get_object::<dag_move::DAG>(execution.dag_id())
            .await
            .map_err(NexusError::Rpc)?;
        let vertices = fetch_dag_vertices_bcs(crawler, &dag.data)
            .await
            .map_err(NexusError::Rpc)?;
        let clock = crawler
            .get_object::<SuiClock>(move_boundary::CLOCK_OBJECT_ID)
            .await
            .map_err(NexusError::Rpc)?
            .data;

        let payment = tap::fetch_execution_payment_for_execution(crawler, dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let gas_service_id = *self.client.nexus_objects.gas_service.object_id();
        let refs = fetch_tool_gas_refs_for_abort_candidates(
            crawler,
            gas_service_id,
            filter_tool_gas_abort_candidate_walks(
                dag_execution_id,
                &vertices,
                &execution.walks,
                &payment.locked_vertices,
                clock.timestamp_ms,
            )
            .map_err(NexusError::Parsing)?,
        )
        .await?;

        Ok(refs)
    }

    /// Submit `gas_extension::abort_expired_execution_with_tool_gas` for one
    /// eligible ToolGas candidate. Candidate discovery is advisory; Move still
    /// verifies timeout and lock state on chain.
    pub async fn abort_expired_execution_with_tool_gas(
        &self,
        dag_execution_id: sui::types::Address,
        tool_gas_id: Option<sui::types::Address>,
    ) -> Result<AbortExpiredExecutionResult, NexusError> {
        let candidates = self
            .abort_expired_execution_tool_gas_candidates(dag_execution_id)
            .await?;
        let selected_candidate = select_tool_gas_abort_candidate(candidates, tool_gas_id)?;
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DAGExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id())
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let execution_ref = crawler
            .get_object_metadata(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let tx = gas::abort_expired_execution_with_tool_gas_ptb(
            nexus_objects,
            &selected_candidate.tool_gas_ref,
            &dag_ref,
            &execution_ref,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(AbortExpiredExecutionResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id(),
            dag_execution_id,
            selected_candidate,
        })
    }
}

fn select_tool_gas_abort_candidate(
    candidates: Vec<ToolGasAbortCandidate>,
    tool_gas_id: Option<sui::types::Address>,
) -> Result<ToolGasAbortCandidate, NexusError> {
    if let Some(tool_gas_id) = tool_gas_id {
        candidates
            .into_iter()
            .find(|candidate| *candidate.tool_gas_ref.object_id() == tool_gas_id)
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!(
                    "ToolGas '{tool_gas_id}' is not currently eligible to abort this execution"
                ))
            })
    } else {
        candidates.into_iter().next().ok_or_else(|| {
            NexusError::Parsing(anyhow!(
                "No ToolGas abort candidates are currently eligible for this execution"
            ))
        })
    }
}

impl ExecutionCostResult {
    fn from_payment(payment: ExecutionPayment) -> Self {
        Self {
            payment_id: payment.payment_id(),
            max_budget: payment.max_budget,
            locked_budget: payment.locked_budget,
            consumed: payment.consumed,
            outstanding_locks: payment.locks(),
            accomplished: payment.accomplished,
            refunded: payment.refunded,
        }
    }
}

fn payment_vertex_key(
    execution_id: sui::types::Address,
    vertex: &RuntimeVertex,
    tool_fqn: &crate::ToolFqn,
) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"nexus.payment.vertex.v1");
    bytes.extend(bcs::to_bytes(&execution_id)?);
    bytes.extend(bcs::to_bytes(vertex)?);
    bytes.extend(tool_fqn.to_string().as_bytes());
    Ok(Sha256::digest(bytes).to_vec())
}

fn filter_tool_gas_abort_candidate_walks(
    execution_id: sui::types::Address,
    vertices: &HashMap<graph_move::Vertex, graph_move::VertexInfo>,
    walks: &[DAGWalk],
    locks: &[ExecutionPaymentVertexLock],
    clock_ms: u64,
) -> anyhow::Result<HashMap<crate::ToolFqn, Vec<ToolGasAbortCandidateWalk>>> {
    let mut candidates = HashMap::<crate::ToolFqn, Vec<ToolGasAbortCandidateWalk>>::new();
    for (walk_index, walk) in walks.iter().enumerate() {
        let Some(vertex) = walk.abortable_timeout_expired_vertex(clock_ms) else {
            continue;
        };
        let vertex_info = vertices.get(vertex.vertex()).ok_or_else(|| {
            anyhow!(
                "DAG vertex '{}' missing from fetched DAG",
                vertex.vertex_name()
            )
        })?;
        let tool_fqn = vertex_info.kind.tool_fqn()?;
        let vertex_key = payment_vertex_key(execution_id, vertex, &tool_fqn)?;
        let tool_fqn_bytes = tool_fqn.to_string().into_bytes();
        if locks
            .iter()
            .any(|lock| lock.vertex_key == vertex_key && lock.tool_fqn == tool_fqn_bytes)
        {
            candidates
                .entry(tool_fqn)
                .or_default()
                .push(ToolGasAbortCandidateWalk {
                    walk_index,
                    vertex: vertex.clone(),
                    payment_vertex_key: vertex_key,
                });
        }
    }
    Ok(candidates)
}

async fn fetch_tool_gas_refs_for_abort_candidates(
    crawler: &Crawler,
    gas_service_id: sui::types::Address,
    candidates: HashMap<crate::ToolFqn, Vec<ToolGasAbortCandidateWalk>>,
) -> Result<Vec<ToolGasAbortCandidate>, NexusError> {
    let mut result = Vec::new();
    for (tool_fqn, matching_walks) in candidates {
        let tool_gas_id = crate::move_bindings::derive_tool_gas_id(gas_service_id, &tool_fqn)
            .map_err(NexusError::Parsing)?;
        let tool_gas_ref = crawler
            .get_object_metadata(tool_gas_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        result.push(ToolGasAbortCandidate {
            tool_fqn,
            tool_gas_ref,
            matching_walks,
        });
    }
    result.sort_by_key(|candidate| candidate.tool_fqn.to_string());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::NexusEventKind,
            fqn,
            move_bindings::{
                interface::{
                    agent::{Agent, SkillDagBinding, SkillRequirement, SkillSchedulePolicy},
                    dag as dag_move,
                    graph::{self as graph_move, PostFailureAction, RuntimeVertex},
                    payment::{
                        ExecutionPaymentFinalState,
                        SkillPaymentPolicy,
                        VertexExecutionPaymentSettlementKind,
                    },
                    verifier::{VerifierConfig, VerifierMode},
                    version::InterfaceVersion,
                },
                move_std::{
                    ascii::String as MoveString,
                    option::Option as MoveOption,
                    type_name::TypeName,
                },
                primitives::data::NexusData,
                registry::agent_registry::{
                    AgentRecord,
                    AgentRegistry,
                    DefaultDagExecutor,
                    DefaultDagExecutorFieldKey,
                    SkillRecord,
                },
                sui_framework::{table::Table as MoveTable, vec_set::VecSet},
                workflow::{
                    execution::DagExecutionPaymentFieldKey,
                    execution_events::{
                        EndStateReachedEvent,
                        ExecutionFinishedEvent,
                        TerminalErrEvalRecordedEvent,
                        WalkAdvancedEvent,
                    },
                    execution_failure::WorkflowFailureClass,
                },
            },
            sui::traits::*,
            test_utils::{nexus_mocks, sui_mocks},
            types::{AgentRegistrySnapshot, DefaultDagExecutorTarget, SkillRecordContext},
        },
        serde::Serialize,
        std::sync::Arc,
        tokio::sync::Mutex,
    };

    #[derive(Clone, Debug, Serialize)]
    struct DynamicFieldValueBcs<K, V> {
        id: sui::types::Address,
        name: K,
        value: V,
    }

    fn inline_bytes(value: &'static [u8]) -> NexusData {
        NexusData::inline_one(value.to_vec())
    }

    fn clock_bcs(timestamp_ms: u64) -> Vec<u8> {
        bcs::to_bytes(&SuiClock::new(move_boundary::CLOCK_OBJECT_ID, timestamp_ms))
            .expect("clock BCS should serialize")
    }

    #[test]
    fn generated_sui_clock_bcs_matches_live_object_shape() {
        let bytes = clock_bcs(61_000);

        assert_eq!(bytes.len(), 40);
        let clock = bcs::from_bytes::<SuiClock>(&bytes).expect("clock BCS should decode");
        assert_eq!(clock.id.address(), move_boundary::CLOCK_OBJECT_ID);
        assert_eq!(clock.timestamp_ms, 61_000);
    }

    #[test]
    fn unresolved_timeout_skip_reason_distinguishes_pending_from_terminal_walks() {
        let active = DAGWalk::Active {
            next_vertex: RuntimeVertex::plain("tool"),
            timeout_ms: 30_000,
            requires_vertex_authorization_grant: false,
            created_at: 1_000,
        };
        let pending_settlement = DAGWalk::PendingSettlement {
            next_vertex: RuntimeVertex::plain("tool"),
            timeout_ms: 30_000,
            requires_vertex_authorization_grant: false,
            created_at: 1_000,
        };

        assert_eq!(
            unresolved_timeout_skip_reason(&active),
            EXPIRED_WALK_NOT_DOUBLE_TIMEOUT_EXPIRED_REASON
        );
        assert_eq!(
            unresolved_timeout_skip_reason(&pending_settlement),
            EXPIRED_WALK_NOT_DOUBLE_TIMEOUT_EXPIRED_REASON
        );
        assert_eq!(
            unresolved_timeout_skip_reason(&DAGWalk::Successful),
            EXPIRED_WALK_ALREADY_TERMINAL_REASON
        );
    }

    #[derive(Clone, Debug, Serialize)]
    struct UnrelatedDynamicFieldKey {
        marker: u64,
    }

    fn committed_tool_result_bcs(
        expected_vertex: RuntimeVertex,
        primary_leader: sui::types::Address,
        _secondary_leader: sui::types::Address,
        primary_failure: Option<FailureEvidenceKind>,
        secondary_failure: Option<FailureEvidenceKind>,
    ) -> execution_move::CommittedToolResult {
        execution_move::CommittedToolResult {
            expected_vertex,
            variant: graph_move::OutputVariant::new("ok"),
            variant_ports_to_data: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            failure_evidence_kind: MoveOption::from_option(primary_failure.clone()),
            primary_failure_evidence_kind: MoveOption::from_option(primary_failure),
            secondary_failure_evidence_kind: MoveOption::from_option(secondary_failure),
            current_leader_cap_id: object_id(primary_leader),
            has_finalized_onchain_payload: false,
            leader_records: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![crate::move_bindings::sui_framework::vec_map::Entry {
                    key: object_id(primary_leader),
                    value: execution_move::CommittedToolResultLeaderRecord {
                        commit_tx_digest: vec![1, 2, 3],
                        recipient: sui::types::Address::from_static("0x44"),
                        commit_gas_charge: MoveOption::from_option(Some(10)),
                        settlement_gas_charge: MoveOption::from_option(None),
                    },
                }],
            },
        }
    }

    fn raw_inline_nexus_data_bcs(one: impl Into<Vec<u8>>) -> NexusData {
        NexusData {
            storage: b"inline".to_vec(),
            one: one.into(),
            many: vec![],
        }
    }

    fn object_id(bytes: sui::types::Address) -> crate::move_bindings::sui_framework::object::ID {
        crate::move_bindings::sui_framework::object::ID::new(bytes)
    }

    fn output_variant(name: &str) -> crate::move_bindings::interface::graph::OutputVariant {
        crate::move_bindings::interface::graph::OutputVariant {
            name: MoveString::from(name),
        }
    }

    fn dag_bcs(vertices_size: u64) -> dag_move::DAG {
        dag_move::DAG {
            id: crate::move_bindings::sui_framework::object::UID::new(sui_mocks::mock_sui_address()),
            vertices: linked_table::LinkedTable::new(sui_mocks::mock_sui_address(), vertices_size),
            entry_groups: crate::move_bindings::sui_framework::vec_map::VecMap { contents: vec![] },
            edges: MoveTable::new(sui_mocks::mock_sui_address(), 0),
            outputs: MoveTable::new(sui_mocks::mock_sui_address(), 0),
            defaults_to_input_ports: MoveTable::new(sui_mocks::mock_sui_address(), 0),
            post_failure_action: MoveOption::from_option(None::<graph_move::PostFailureAction>),
            leader_verifier: VerifierConfig::default(),
            tool_verifier: VerifierConfig::default(),
        }
    }

    fn empty_object_table<T0, T1>(
        id: sui::types::Address,
    ) -> crate::move_bindings::sui_framework::object_table::ObjectTable<T0, T1> {
        crate::move_bindings::sui_framework::object_table::ObjectTable {
            id: crate::move_bindings::sui_framework::object::UID::new(id),
            size: 0,
            phantom_t0: std::marker::PhantomData,
            phantom_t1: std::marker::PhantomData,
        }
    }

    fn dag_execution_bcs(
        execution_ref: &sui::types::ObjectReference,
        dag_ref: &sui::types::ObjectReference,
        walks: Vec<execution_move::DAGWalk>,
    ) -> execution_move::DAGExecution {
        execution_move::DAGExecution {
            id: crate::move_bindings::sui_framework::object::UID::new(*execution_ref.object_id()),
            dag: object_id(*dag_ref.object_id()),
            entry_group: graph_move::EntryGroup::new(DEFAULT_ENTRY_GROUP),
            invoker: sui::types::Address::from_static("0x1"),
            created_at: 0,
            priority_fee_per_gas_unit: 0,
            agent_id: object_id(sui::types::Address::from_static("0xa")),
            skill_id: 11,
            interface_version: InterfaceVersion::new(7),
            scheduled_task_id: MoveOption::from_option(None),
            scheduled_occurrence_index: MoveOption::from_option(None),
            last_request_for_execution_emitted_at_digest: vec![],
            last_request_for_execution_leaders: vec![],
            network: object_id(sui::types::Address::from_static("0xf")),
            evaluations: empty_object_table(sui_mocks::mock_sui_address()),
            terminal_records: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            submission_failure_records: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            pending_retry_handoff_cap_ids: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            walk_request_authorities: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            pending_gas_settlements: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            active_walks: walks
                .iter()
                .filter(|walk| matches!(walk, execution_move::DAGWalk::Active { .. }))
                .count() as u64,
            pending_abort_walks: 0,
            pending_settlement_walks: 0,
            successful_walks: 0,
            failed_walks: 0,
            aborted_walks: 0,
            consumed_walks: 0,
            cancelled_walks: 0,
            walks,
        }
    }

    fn mock_get_dag_execution_bcs(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        nexus_objects: &crate::types::NexusObjects,
        execution_ref: sui::types::ObjectReference,
        dag_ref: &sui::types::ObjectReference,
        walks: Vec<execution_move::DAGWalk>,
    ) {
        let owner = sui::types::Owner::Shared(execution_ref.version());
        let execution = dag_execution_bcs(&execution_ref, dag_ref, walks);
        sui_mocks::grpc::mock_get_object_bcs_for(
            ledger_service_mock,
            execution_ref,
            owner,
            bcs::to_bytes(&execution).expect("DAGExecution BCS should serialize"),
            crate::move_bindings::struct_tag::<execution_move::DAGExecution>(nexus_objects),
        );
    }

    fn offchain_vertex_info(tool_fqn: &crate::ToolFqn) -> graph_move::VertexInfo {
        graph_move::VertexInfo {
            kind: graph_move::VertexKind::OffChain {
                _variant_name: "OffChain".into(),
                tool_fqn: tool_fqn.to_string().into(),
            },
            input_ports: VecSet { contents: vec![] },
            post_failure_action: MoveOption::from_option(None::<graph_move::PostFailureAction>),
            leader_verifier: MoveOption::from_option(None::<VerifierConfig>),
            tool_verifier: MoveOption::from_option(None::<VerifierConfig>),
        }
    }

    fn ports_data_map(
        entries: Vec<(&str, &'static [u8])>,
    ) -> crate::move_bindings::sui_framework::vec_map::VecMap<
        crate::move_bindings::interface::graph::OutputPort,
        crate::move_bindings::primitives::data::NexusData,
    > {
        crate::move_bindings::sui_framework::vec_map::VecMap {
            contents: entries
                .into_iter()
                .map(
                    |(name, value)| crate::move_bindings::sui_framework::vec_map::Entry {
                        key: crate::move_bindings::interface::graph::OutputPort {
                            name: MoveString::from(name),
                        },
                        value: crate::move_bindings::primitives::data::NexusData {
                            storage: b"inline".to_vec(),
                            one: value.to_vec(),
                            many: vec![],
                        },
                    },
                )
                .collect(),
        }
    }

    async fn crawler_from_mocks(
        ledger_service_mock: sui_mocks::grpc::MockLedgerService,
        state_service_mock: sui_mocks::grpc::MockStateService,
    ) -> Crawler {
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        Crawler::new(Arc::new(Mutex::new(client)))
    }

    #[tokio::test]
    async fn fetch_committed_tool_result_for_walk_returns_none_when_absent() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields::<execution_move::CommittedToolResultKey>(
            &mut state_service_mock,
            vec![],
        );
        let crawler = crawler_from_mocks(
            sui_mocks::grpc::MockLedgerService::new(),
            state_service_mock,
        )
        .await;

        let result = fetch_committed_tool_result_for_walk(&crawler, execution_id, 7)
            .await
            .expect("fetch should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_committed_tool_result_for_walk_skips_unrelated_keys_and_decodes_match() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf1"));
        let field_id = *field_ref.object_id();
        let primary_leader = sui::types::Address::from_static("0xa1");
        let secondary_leader = sui::types::Address::from_static("0xa2");
        let committed = committed_tool_result_bcs(
            RuntimeVertex::plain("retryable"),
            primary_leader,
            secondary_leader,
            Some(FailureEvidenceKind::ToolEvidence),
            None,
        );
        let field_value = DynamicFieldValueBcs {
            id: sui::types::Address::from_static("0xdf"),
            name: execution_move::CommittedToolResultKey { walk_index: 7 },
            value: committed,
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        state_service_mock
            .expect_list_dynamic_fields()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                let mut unrelated = sui::grpc::DynamicField::default();
                unrelated.set_field_id(sui::types::Address::from_static("0xf0").to_string());
                unrelated.set_name(
                    bcs::to_bytes(&UnrelatedDynamicFieldKey { marker: 1 })
                        .expect("unrelated key bcs"),
                );
                let mut wanted = sui::grpc::DynamicField::default();
                wanted.set_field_id(field_id.to_string());
                wanted.set_name(
                    bcs::to_bytes(&execution_move::CommittedToolResultKey { walk_index: 7 })
                        .expect("committed key bcs"),
                );
                response.set_dynamic_fields(vec![unrelated, wanted]);
                Ok(tonic::Response::new(response))
            });

        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            field_ref,
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&field_value).expect("field value bcs"),
            sui::types::StructTag::new(
                sui::types::Address::TWO,
                sui::types::Identifier::from_static("dynamic_field"),
                sui::types::Identifier::from_static("Field"),
                vec![],
            ),
        );
        let crawler = crawler_from_mocks(ledger_service_mock, state_service_mock).await;

        let result = fetch_committed_tool_result_for_walk(&crawler, execution_id, 7)
            .await
            .expect("fetch should succeed")
            .expect("committed result should exist");

        assert_eq!(result.expected_vertex, RuntimeVertex::plain("retryable"));
        assert_eq!(
            result.primary_failure_evidence_kind,
            Some(FailureEvidenceKind::ToolEvidence)
        );
        assert_eq!(result.secondary_failure_evidence_kind, None);
        assert!(result.leader_record(primary_leader).is_some());
        assert!(result.leader_record(secondary_leader).is_none());
    }

    #[tokio::test]
    async fn fetch_committed_tool_result_for_walk_decodes_metadata_with_raw_output_payload() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf2"));
        let field_id = *field_ref.object_id();
        let primary_leader = sui::types::Address::from_static("0xa1");
        let secondary_leader = sui::types::Address::from_static("0xa2");
        let mut committed = committed_tool_result_bcs(
            RuntimeVertex::plain("retryable"),
            primary_leader,
            secondary_leader,
            Some(FailureEvidenceKind::ToolEvidence),
            None,
        );
        committed.variant_ports_to_data = crate::move_bindings::sui_framework::vec_map::VecMap {
            contents: vec![crate::move_bindings::sui_framework::vec_map::Entry {
                key: graph_move::OutputPort::new("reason"),
                value: raw_inline_nexus_data_bcs(b"not-json".to_vec()),
            }],
        };
        let field_value = DynamicFieldValueBcs {
            id: sui::types::Address::from_static("0xdf"),
            name: execution_move::CommittedToolResultKey { walk_index: 7 },
            value: committed,
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        state_service_mock
            .expect_list_dynamic_fields()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                let mut wanted = sui::grpc::DynamicField::default();
                wanted.set_field_id(field_id.to_string());
                wanted.set_name(
                    bcs::to_bytes(&execution_move::CommittedToolResultKey { walk_index: 7 })
                        .expect("committed key bcs"),
                );
                response.set_dynamic_fields(vec![wanted]);
                Ok(tonic::Response::new(response))
            });

        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            field_ref,
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&field_value).expect("field value bcs"),
            sui::types::StructTag::new(
                sui::types::Address::TWO,
                sui::types::Identifier::from_static("dynamic_field"),
                sui::types::Identifier::from_static("Field"),
                vec![],
            ),
        );
        let crawler = crawler_from_mocks(ledger_service_mock, state_service_mock).await;

        let result = fetch_committed_tool_result_for_walk(&crawler, execution_id, 7)
            .await
            .expect("fetch should ignore raw output payload bytes")
            .expect("committed result should exist");

        assert_eq!(result.expected_vertex, RuntimeVertex::plain("retryable"));
        assert_eq!(
            result.primary_failure_evidence_kind,
            Some(FailureEvidenceKind::ToolEvidence)
        );
        assert_eq!(result.secondary_failure_evidence_kind, None);
        assert!(result.leader_record(primary_leader).is_some());
        assert!(result.leader_record(secondary_leader).is_none());
    }

    fn offchain_vertex_node_bcs(
        tool_fqn: &crate::ToolFqn,
    ) -> linked_table::Node<graph_move::Vertex, graph_move::VertexInfo> {
        linked_table::Node {
            prev: crate::move_bindings::move_std::option::Option::from_option(
                None::<graph_move::Vertex>,
            ),
            next: crate::move_bindings::move_std::option::Option::from_option(
                None::<graph_move::Vertex>,
            ),
            value: offchain_vertex_info(tool_fqn),
        }
    }

    #[derive(Clone)]
    struct RegistryObjectMock {
        registry_object: AgentRegistry,
        agent_field_ref: sui::types::ObjectReference,
        skill_field_ref: sui::types::ObjectReference,
        default_executor_field_ref: Option<sui::types::ObjectReference>,
        default_executor_value: Option<DefaultDagExecutor>,
        agent_record: AgentRecord,
        skill_record: SkillRecord,
        skill_context: SkillRecordContext,
    }

    fn registry_object_mock(registry: &AgentRegistrySnapshot) -> RegistryObjectMock {
        assert_eq!(registry.agents.len(), 1, "test registry has one agent");
        assert_eq!(registry.skills.len(), 1, "test registry has one skill");
        let agent = registry.agents[0].clone();
        let skill_context = registry.skills[0].clone();
        let skill_record = skill_context.record.clone();
        let default_executor_field_ref = registry
            .default_executor
            .as_ref()
            .map(|_| sui_mocks::mock_sui_object_ref());
        let default_executor_value = registry.default_executor.clone();

        RegistryObjectMock {
            registry_object: AgentRegistry {
                id: crate::move_bindings::sui_framework::object::UID::new(registry.id),
                agents: MoveTable::new(sui::types::Address::from_static("0x9000"), 1),
            },
            agent_field_ref: sui_mocks::mock_sui_object_ref(),
            skill_field_ref: sui_mocks::mock_sui_object_ref(),
            default_executor_field_ref,
            default_executor_value,
            agent_record: agent,
            skill_record,
            skill_context,
        }
    }

    fn mock_fetch_registry_from_tables(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &crate::types::NexusObjects,
        registry: &AgentRegistrySnapshot,
    ) {
        let mock = registry_object_mock(registry);
        sui_mocks::grpc::mock_get_object_bcs_for(
            ledger_service_mock,
            nexus_objects.agent_registry.clone(),
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&mock.registry_object).expect("raw registry bcs"),
            crate::move_bindings::struct_tag::<AgentRegistry>(nexus_objects),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            state_service_mock,
            vec![(
                mock.skill_context.agent_id,
                mock.agent_field_ref.object_id().to_owned(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger_service_mock,
            vec![(
                mock.agent_field_ref,
                sui::types::Owner::Shared(1),
                mock.skill_context.agent_id,
                mock.agent_record,
            )],
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            state_service_mock,
            vec![(
                mock.skill_context.skill_id,
                mock.skill_field_ref.object_id().to_owned(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger_service_mock,
            vec![(
                mock.skill_field_ref,
                sui::types::Owner::Shared(1),
                mock.skill_context.skill_id,
                mock.skill_record,
            )],
        );
        if let (Some(field_ref), Some(value)) =
            (mock.default_executor_field_ref, mock.default_executor_value)
        {
            sui_mocks::grpc::mock_list_dynamic_fields(
                state_service_mock,
                vec![(
                    DefaultDagExecutorFieldKey::default(),
                    *field_ref.object_id(),
                )],
            );
            sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
                ledger_service_mock,
                vec![(
                    field_ref,
                    sui::types::Owner::Shared(1),
                    DefaultDagExecutorFieldKey::default(),
                    value,
                )],
            );
        } else {
            sui_mocks::grpc::mock_list_dynamic_fields::<DefaultDagExecutorFieldKey>(
                state_service_mock,
                vec![],
            );
            sui_mocks::grpc::mock_get_dynamic_table_values_bcs::<
                DefaultDagExecutorFieldKey,
                DefaultDagExecutor,
            >(ledger_service_mock, vec![]);
        }
    }

    fn mock_events_get_checkpoint_with_supported_events(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        objects: crate::types::NexusObjects,
        nexus_events: Vec<NexusEventKind>,
        cp: u64,
    ) {
        ledger_service
            .expect_get_checkpoint()
            .returning(move |request| {
                let checkpoint_number = match request.into_inner().checkpoint_id {
                    Some(sui::grpc::get_checkpoint_request::CheckpointId::SequenceNumber(
                        number,
                    )) => number,
                    _ => cp,
                };
                let mut response = sui::grpc::GetCheckpointResponse::default();
                let mut checkpoint = sui::grpc::Checkpoint::default();
                let mut transactions = vec![];
                for index in 0..10 {
                    let mut transaction = sui::grpc::ExecutedTransaction::default();
                    transaction.set_digest(sui_mocks::grpc::mock_event_transaction_digest(
                        checkpoint_number,
                        index,
                    ));
                    transactions.push(transaction);
                }
                checkpoint.set_transactions(transactions);
                checkpoint.set_sequence_number(checkpoint_number);
                response.set_checkpoint(checkpoint);
                Ok(tonic::Response::new(response))
            });

        ledger_service
            .expect_batch_get_transactions()
            .returning(move |request| {
                let requested_digests = request.into_inner().digests;
                let mut response = sui::grpc::BatchGetTransactionsResponse::default();
                let mut events = vec![];

                #[derive(Serialize)]
                struct Wrapper<T> {
                    event: T,
                }

                for event in nexus_events.clone() {
                    let module = if matches!(event, NexusEventKind::DAGCreated(_)) {
                        "dag"
                    } else {
                        "execution"
                    };
                    let t = format!(
                        "{}::event::EventWrapper<{}::{module}::{}>",
                        objects.primitives_pkg_id,
                        objects.workflow_pkg_id,
                        event.name()
                    );

                    let mut grpc_event = sui::grpc::Event::default();
                    grpc_event.set_package_id(objects.workflow_pkg_id);
                    grpc_event.set_module(module.to_string());
                    grpc_event.set_sender(sui::types::Address::ZERO);
                    grpc_event.set_event_type(t);
                    grpc_event.set_contents(match event {
                        NexusEventKind::WalkAdvanced(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::EndStateReached(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::ExecutionFinished(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::TerminalErrEvalRecorded(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::DAGCreated(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        _ => panic!("Unsupported event type for BCS serialization"),
                    });
                    events.push(grpc_event);
                }
                let event_digest =
                    sui_mocks::grpc::mock_event_transaction_digest(cp, 0).to_string();
                let transactions = requested_digests
                    .into_iter()
                    .map(|digest| {
                        let mut transaction = sui::grpc::ExecutedTransaction::default();
                        transaction.set_checkpoint(
                            sui_mocks::grpc::mock_event_transaction_checkpoint(&digest)
                                .unwrap_or(cp),
                        );
                        transaction.set_digest(digest.clone());

                        let mut tx_events = sui::grpc::TransactionEvents::default();
                        if digest == event_digest {
                            tx_events.set_events(events.clone());
                        }
                        transaction.set_events(tx_events);

                        let mut result = sui::grpc::GetTransactionResult::default();
                        result.set_transaction(transaction);
                        result
                    })
                    .collect();
                response.set_transactions(transactions);
                Ok(tonic::Response::new(response))
            });
    }

    #[tokio::test]
    async fn test_workflow_actions_publish() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let dag_created = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    crate::move_bindings::struct_tag::<dag_move::DAG>(&nexus_objects),
                    true,
                    0,
                    dag_object_id.to_bcs().unwrap(),
                )
                .unwrap(),
            ),
            sui::types::Owner::Shared(0),
            digest,
            1000,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![dag_created],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let dag = DagSpec::default();

        let result = client
            .workflow()
            .publish(dag)
            .await
            .expect("Failed to publish DAG");

        assert_eq!(result.dag_object_id, dag_object_id);
        assert_eq!(result.tx_digest, digest);
        assert_eq!(result.tx_checkpoint, 1);
    }

    #[cfg(feature = "walrus")]
    #[tokio::test]
    async fn test_workflow_actions_execute() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let mut nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();
        let tool_fqn = fqn!("xyz.taluslabs.test@1");
        let default_agent = sui::types::Address::generate(&mut rng);
        let default_skill_id = 11;
        let default_agent_ref = sui::types::ObjectReference::new(
            default_agent,
            1,
            sui::types::Digest::generate(&mut rng),
        );
        nexus_objects.default_dag_executor = DefaultDagExecutorTarget {
            agent_id: default_agent,
            skill_id: default_skill_id,
        };

        let requirements = SkillRequirement {
            input_commitment: vec![1],
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        };
        let agent_registry = AgentRegistrySnapshot {
            id: *nexus_objects.agent_registry.object_id(),
            agents: vec![AgentRecord {
                active: true,
                skills: MoveTable::new(sui::types::Address::generate(&mut rng), 1),
            }],
            skills: vec![SkillRecordContext {
                agent_id: default_agent,
                skill_id: default_skill_id,
                record: SkillRecord {
                    description: vec![3],
                    active: true,
                    dag_binding: SkillDagBinding::runtime_selected(),
                    requirements: requirements.clone(),
                    current_interface_revision: InterfaceVersion::new(1),
                    scheduled_task_count: 0,
                },
            }],
            default_executor: Some(DefaultDagExecutor {
                agent: Agent::from_ids(
                    default_agent,
                    1,
                    Some(*nexus_objects.agent_registry.object_id()),
                ),
                skill_id: default_skill_id,
            }),
        };

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let execution_created = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        nexus_objects.workflow_pkg_id,
                        sui::types::Identifier::from_static("execution"),
                        sui::types::Identifier::from_static("DAGExecution"),
                        vec![],
                    ),
                    true,
                    0,
                    execution_object_id.to_bcs().unwrap(),
                )
                .unwrap(),
            ),
            sui::types::Owner::Shared(0),
            tx_digest,
            1000,
        );

        // DAG
        let dag = dag_bcs(1);

        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&dag).expect("DAG BCS should serialize"),
        );

        // DagSpec.vertices
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(
                graph_move::Vertex::new("ToolVertex"),
                *tool_gas_ref.object_id(),
            )],
        );

        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service_mock,
            vec![(
                tool_gas_ref.clone(),
                sui::types::Owner::Shared(0),
                graph_move::Vertex::new("ToolVertex"),
                offchain_vertex_node_bcs(&tool_fqn),
            )],
        );

        // ToolGas
        sui_mocks::grpc::mock_get_objects_metadata(
            &mut ledger_service_mock,
            vec![(tool_gas_ref, sui::types::Owner::Shared(0), None)],
        );

        mock_fetch_registry_from_tables(
            &mut ledger_service_mock,
            &mut state_service_mock,
            &nexus_objects,
            &agent_registry,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            default_agent_ref,
            sui::types::Owner::Shared(0),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![execution_created],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            state_service_mock: Some(state_service_mock),
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let entry_data = HashMap::from([(
            "entry_vertex".to_string(),
            VecMap::<InputPort, NexusData>::from_map(HashMap::from([
                ("entry_port".to_string(), inline_bytes(b"data")),
                ("another_entry_port".to_string(), inline_bytes(b"data")),
            ])),
        )]);

        let price_priority_fee = 0_u64;

        let result = client
            .workflow()
            .execute(
                *dag_ref.object_id(),
                entry_data,
                price_priority_fee,
                None,
                &StorageConf::default(),
            )
            .await
            .expect("Failed to execute DAG");

        assert_eq!(result.execution_object_id, execution_object_id);
        assert_eq!(result.tx_digest, tx_digest);
        let tap_execution = result.tap_execution.expect("TAP execution metadata");
        assert_eq!(tap_execution.payment_max_budget, 1000);
    }

    #[cfg(feature = "walrus")]
    #[tokio::test]
    async fn test_workflow_actions_execute_agent_dag_pinned_skill() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let tool_fqn = fqn!("xyz.taluslabs.standard_tap@1");
        let tool_gas_id = crate::move_bindings::derive_tool_gas_id(
            *nexus_objects.gas_service.object_id(),
            &tool_fqn,
        )
        .expect("derive tool gas id");
        let tool_gas_ref = sui::types::ObjectReference::new(
            tool_gas_id,
            1,
            sui::types::Digest::generate(&mut rng),
        );
        let agent_id = sui::types::Address::generate(&mut rng);
        let skill_id = 22;
        let agent_ref =
            sui::types::ObjectReference::new(agent_id, 2, sui::types::Digest::generate(&mut rng));
        let requirements = SkillRequirement {
            input_commitment: vec![1],
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        };
        let agent_registry = AgentRegistrySnapshot {
            id: *nexus_objects.agent_registry.object_id(),
            agents: vec![AgentRecord {
                active: true,
                skills: MoveTable::new(sui::types::Address::generate(&mut rng), 1),
            }],
            skills: vec![SkillRecordContext {
                agent_id,
                skill_id,
                record: SkillRecord {
                    description: vec![3],
                    active: true,
                    dag_binding: SkillDagBinding::pinned(*dag_ref.object_id()),
                    requirements: requirements.clone(),
                    current_interface_revision: InterfaceVersion::new(1),
                    scheduled_task_count: 0,
                },
            }],
            default_executor: None,
        };

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let execution_created = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        nexus_objects.workflow_pkg_id,
                        sui::types::Identifier::from_static("execution"),
                        sui::types::Identifier::from_static("DAGExecution"),
                        vec![],
                    ),
                    true,
                    0,
                    execution_object_id.to_bcs().unwrap(),
                )
                .unwrap(),
            ),
            sui::types::Owner::Shared(0),
            tx_digest,
            1000,
        );
        let dag = dag_bcs(1);
        mock_fetch_registry_from_tables(
            &mut ledger_service_mock,
            &mut state_service_mock,
            &nexus_objects,
            &agent_registry,
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&dag).expect("DAG BCS should serialize"),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(
                graph_move::Vertex::new("ToolVertex"),
                *tool_gas_ref.object_id(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service_mock,
            vec![(
                tool_gas_ref.clone(),
                sui::types::Owner::Shared(0),
                graph_move::Vertex::new("ToolVertex"),
                offchain_vertex_node_bcs(&tool_fqn),
            )],
        );
        sui_mocks::grpc::mock_get_objects_metadata(
            &mut ledger_service_mock,
            vec![(tool_gas_ref, sui::types::Owner::Shared(0), None)],
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref,
            sui::types::Owner::Shared(2),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref,
            vec![execution_created],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            state_service_mock: Some(state_service_mock),
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;
        let entry_data = HashMap::from([(
            "entry_vertex".to_string(),
            VecMap::<InputPort, NexusData>::from_map(HashMap::from([(
                "entry_port".to_string(),
                inline_bytes(b"data"),
            )])),
        )]);

        let result = client
            .workflow()
            .execute_agent_dag(
                agent_id,
                skill_id,
                entry_data,
                0,
                Some("custom"),
                &StorageConf::default(),
                AgentDagExecuteOptions {
                    payment_max_budget: 100,
                    ..Default::default()
                },
            )
            .await
            .expect("agent DAG execution succeeds");

        assert_eq!(result.execution_object_id, execution_object_id);
        assert_eq!(result.tx_digest, tx_digest);
        let metadata = result.tap_execution.expect("TAP execution metadata");
        assert_eq!(metadata.agent_id, agent_id);
        assert_eq!(metadata.skill_id, skill_id);
        assert_eq!(metadata.dag_id, *dag_ref.object_id());
        assert_eq!(metadata.payment_max_budget, 100);
    }

    #[tokio::test]
    async fn test_workflow_actions_inspect_execution() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);
        let execution_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_object_creation_checkpoint(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                execution_object_id,
                42,
                sui::types::Digest::generate(&mut rng),
            ),
            42,
            sui::types::Digest::generate(&mut rng),
            1,
        );

        let walk_advanced_event = NexusEventKind::WalkAdvanced(WalkAdvancedEvent {
            dag: object_id(dag_object_id),
            execution: object_id(execution_object_id),
            walk_index: 0,
            vertex: RuntimeVertex::Plain {
                vertex: TypeName::new("initial").into(),
            },
            variant: output_variant("ok"),
            variant_ports_to_data: ports_data_map(vec![]),
        });

        let end_state_reached_event = NexusEventKind::EndStateReached(EndStateReachedEvent {
            dag: object_id(dag_object_id),
            execution: object_id(execution_object_id),
            walk_index: 0,
            vertex: RuntimeVertex::Plain {
                vertex: TypeName::new("initial").into(),
            },
            variant: output_variant("ok"),
            variant_ports_to_data: ports_data_map(vec![]),
        });
        let execution_finished_event = NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
            dag: object_id(dag_object_id),
            execution: object_id(execution_object_id),
            has_any_walk_failed: false,
            has_any_walk_succeeded: true,
            was_aborted: false,
        });

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);

        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![
                walk_advanced_event.clone(),
                end_state_reached_event.clone(),
                execution_finished_event.clone(),
            ],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let mut result = client
            .workflow()
            .inspect_execution(execution_object_id, Some(std::time::Duration::from_secs(5)))
            .await
            .expect("Failed to setup channel");

        let mut events = vec![];

        while let Some(event) = result.next_event.recv().await {
            match &event.data {
                NexusEventKind::ExecutionFinished(_) => {
                    events.push(event);

                    break;
                }
                _ => events.push(event),
            }
        }

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0].data, NexusEventKind::WalkAdvanced(_)));
        assert!(matches!(events[1].data, NexusEventKind::EndStateReached(_)));
        assert!(matches!(
            events[2].data,
            NexusEventKind::ExecutionFinished(_)
        ));
        assert!(result.poller.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_workflow_actions_inspect_execution_timeout() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_object_creation_checkpoint(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                execution_object_id,
                42,
                sui::types::Digest::generate(&mut rng),
            ),
            42,
            sui::types::Digest::generate(&mut rng),
            1,
        );

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);

        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let mut result = client
            .workflow()
            .inspect_execution(execution_object_id, Some(std::time::Duration::from_secs(3)))
            .await
            .expect("Failed to setup channel");

        let mut events = vec![];

        while let Some(event) = result.next_event.recv().await {
            events.push(event);
        }

        assert_eq!(events.len(), 0);
        assert!(matches!(
            result.poller.await.unwrap(),
            Err(NexusError::Timeout(_))
        ));
    }

    #[cfg(feature = "walrus")]
    #[tokio::test]
    async fn test_workflow_actions_inspect_execution_until_completion() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);
        let execution_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_object_creation_checkpoint(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                execution_object_id,
                42,
                sui::types::Digest::generate(&mut rng),
            ),
            42,
            sui::types::Digest::generate(&mut rng),
            1,
        );

        let walk_advanced_event = NexusEventKind::WalkAdvanced(WalkAdvancedEvent {
            dag: object_id(dag_object_id),
            execution: object_id(execution_object_id),
            walk_index: 0,
            vertex: RuntimeVertex::plain("initial"),
            variant: output_variant("ok"),
            variant_ports_to_data: ports_data_map(vec![]),
        });
        let terminal_err_eval_event =
            NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
                dag: object_id(dag_object_id),
                execution: object_id(execution_object_id),
                walk_index: 1,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalToolFailure,
                outcome: MoveOption::from_option(Some(PostFailureAction::Terminate)),
                reason: MoveString::from("tool failed"),
                err_eval_hash: vec![9, 8, 7],
                duplicate: false,
            });
        let end_state_reached_event = NexusEventKind::EndStateReached(EndStateReachedEvent {
            dag: object_id(dag_object_id),
            execution: object_id(execution_object_id),
            walk_index: 0,
            vertex: RuntimeVertex::plain("final"),
            variant: output_variant("ok"),
            variant_ports_to_data: ports_data_map(vec![("answer", b"42")]),
        });
        let execution_finished_event = NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
            dag: object_id(dag_object_id),
            execution: object_id(execution_object_id),
            has_any_walk_failed: true,
            has_any_walk_succeeded: true,
            was_aborted: false,
        });

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);
        mock_events_get_checkpoint_with_supported_events(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![
                walk_advanced_event,
                terminal_err_eval_event,
                end_state_reached_event,
                execution_finished_event,
            ],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .workflow()
            .inspect_execution_until_completion(
                execution_object_id,
                Some(std::time::Duration::from_secs(5)),
                &StorageConf::default(),
            )
            .await
            .expect("Failed to inspect execution until completion");

        assert_eq!(
            result.terminal_state,
            WorkflowExecutionTerminalState::Failed
        );
        assert!(result.execution_finished.has_any_walk_failed);
        assert!(result.execution_finished.has_any_walk_succeeded);
        assert!(matches!(
            result.events.last().map(|event| &event.data),
            Some(NexusEventKind::ExecutionFinished(_))
        ));
        assert_eq!(result.terminal_err_eval_recordings.len(), 1);
        assert_eq!(
            result.terminal_err_eval_recordings[0].failure_class,
            WorkflowFailureClass::TerminalToolFailure
        );
        assert!(result.events.len() >= 2);
    }

    #[cfg(feature = "walrus")]
    #[tokio::test]
    async fn test_workflow_actions_inspect_execution_until_completion_timeout() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_object_creation_checkpoint(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                execution_object_id,
                42,
                sui::types::Digest::generate(&mut rng),
            ),
            42,
            sui::types::Digest::generate(&mut rng),
            1,
        );
        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);
        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .workflow()
            .inspect_execution_until_completion(
                execution_object_id,
                Some(std::time::Duration::from_secs(3)),
                &StorageConf::default(),
            )
            .await;

        assert!(matches!(result, Err(NexusError::Timeout(_))));
    }

    #[test]
    fn test_event_execution_id_supports_terminal_err_eval_recorded() {
        let execution = sui::types::Address::TWO;
        let event = NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
            dag: object_id(sui::types::Address::ZERO),
            execution: object_id(execution),
            walk_index: 2,
            vertex: RuntimeVertex::plain("failable"),
            leader: sui::types::Address::THREE,
            failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
            outcome: MoveOption::from_option(Some(PostFailureAction::Terminate)),
            reason: MoveString::from("timeout"),
            err_eval_hash: vec![4, 5, 6],
            duplicate: true,
        });

        assert_eq!(event_execution_id(&event), Some(execution));
    }

    #[cfg(feature = "walrus")]
    #[test]
    fn test_terminal_state_from_execution_finished() {
        let success = ExecutionFinishedEvent {
            dag: object_id(sui::types::Address::ZERO),
            execution: object_id(sui::types::Address::TWO),
            has_any_walk_failed: false,
            has_any_walk_succeeded: true,
            was_aborted: false,
        };
        let failed = ExecutionFinishedEvent {
            has_any_walk_failed: true,
            has_any_walk_succeeded: false,
            ..success.clone()
        };
        let aborted = ExecutionFinishedEvent {
            has_any_walk_failed: true,
            has_any_walk_succeeded: true,
            was_aborted: true,
            ..success.clone()
        };
        let no_walk_outcome = ExecutionFinishedEvent {
            has_any_walk_failed: false,
            has_any_walk_succeeded: false,
            was_aborted: false,
            ..success.clone()
        };

        assert_eq!(
            terminal_state_from_execution_finished(&success),
            WorkflowExecutionTerminalState::Succeeded
        );
        assert_eq!(
            terminal_state_from_execution_finished(&failed),
            WorkflowExecutionTerminalState::Failed
        );
        assert_eq!(
            terminal_state_from_execution_finished(&aborted),
            WorkflowExecutionTerminalState::Aborted
        );
        assert_eq!(
            terminal_state_from_execution_finished(&no_walk_outcome),
            WorkflowExecutionTerminalState::NoWalkOutcome
        );
    }

    #[cfg(feature = "walrus")]
    #[tokio::test]
    async fn test_build_execution_completion_result_resolves_end_states() {
        let execution = sui::types::Address::TWO;
        let events = vec![
            NexusEvent {
                id: (sui::types::Digest::ZERO, 0),
                generics: vec![],
                data: NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
                    dag: object_id(sui::types::Address::ZERO),
                    execution: object_id(execution),
                    walk_index: 1,
                    vertex: RuntimeVertex::plain("failable"),
                    leader: sui::types::Address::THREE,
                    failure_class: WorkflowFailureClass::TerminalToolFailure,
                    outcome: MoveOption::from_option(Some(PostFailureAction::Terminate)),
                    reason: MoveString::from("tool failed"),
                    err_eval_hash: vec![1, 2, 3],
                    duplicate: false,
                }),
                distribution: None,
            },
            NexusEvent {
                id: (sui::types::Digest::ZERO, 1),
                generics: vec![],
                data: NexusEventKind::EndStateReached(EndStateReachedEvent {
                    dag: object_id(sui::types::Address::ZERO),
                    execution: object_id(execution),
                    walk_index: 0,
                    vertex: RuntimeVertex::plain("final"),
                    variant: output_variant("ok"),
                    variant_ports_to_data: ports_data_map(vec![("answer", b"42")]),
                }),
                distribution: None,
            },
            NexusEvent {
                id: (sui::types::Digest::ZERO, 2),
                generics: vec![],
                data: NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
                    dag: object_id(sui::types::Address::ZERO),
                    execution: object_id(execution),
                    has_any_walk_failed: true,
                    has_any_walk_succeeded: true,
                    was_aborted: false,
                }),
                distribution: None,
            },
        ];

        let result = build_execution_completion_result(events, execution, &StorageConf::default())
            .await
            .expect("summary should build");

        assert_eq!(
            result.terminal_state,
            WorkflowExecutionTerminalState::Failed
        );
        assert_eq!(result.terminal_err_eval_recordings.len(), 1);
        assert_eq!(result.end_states.len(), 1);
        assert_eq!(
            result.end_states[0].event.vertex,
            RuntimeVertex::plain("final")
        );
        assert_eq!(
            result.end_states[0]
                .resolved_ports_to_data
                .get("answer")
                .expect("answer port")
                .inline_one_bytes()
                .expect("answer should be inline bytes"),
            b"42"
        );
    }

    #[tokio::test]
    async fn test_workflow_actions_execution_cost() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_ref = sui_mocks::mock_sui_object_ref();
        let payment_ref = sui_mocks::mock_sui_object_ref();
        let execution_id = *execution_ref.object_id();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let payment_id = *payment_ref.object_id();

        mock_get_dag_execution_bcs(
            &mut ledger_service_mock,
            &nexus_objects,
            execution_ref,
            &sui::types::ObjectReference::new(
                sui::types::Address::from_static("0xd"),
                0,
                sui::types::Digest::ZERO,
            ),
            vec![],
        );

        sui_mocks::grpc::mock_list_dynamic_object_fields(
            &mut state_service_mock,
            vec![(
                DagExecutionPaymentFieldKey::default(),
                sui::types::Address::from_static("0xdf"),
                payment_id,
            )],
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            payment_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&ExecutionPayment {
                id: crate::move_bindings::sui_framework::object::UID::new(payment_id),
                execution_id,
                agent_id: crate::move_bindings::sui_framework::object::ID::new(
                    sui::types::Address::from_static("0xa"),
                ),
                skill_id: 11,
                interface_revision: crate::move_bindings::interface::version::InterfaceVersion::new(
                    7,
                ),
                payment_policy:
                    crate::move_bindings::interface::payment::SkillPaymentPolicy::UserFunded,
                source_kind:
                    crate::move_bindings::interface::payment::PaymentSourceKind::user_funded(
                        sui::types::Address::from_static("0x1"),
                    ),
                max_budget: 100_000,
                locked_budget: 100_000,
                funds: crate::move_bindings::sui_framework::balance::Balance {
                    value: 58_000,
                    phantom_t0: std::marker::PhantomData,
                },
                consumed: 42_000,
                accomplished: true,
                refunded: false,
                final_state: ExecutionPaymentFinalState::Accomplished,
                tool_cost_snapshot: crate::move_bindings::sui_framework::vec_map::VecMap {
                    contents: vec![],
                },
                locked_vertices: vec![],
            })
            .expect("execution payment bcs"),
            crate::move_bindings::struct_tag::<
                crate::move_bindings::interface::payment::ExecutionPayment,
            >(&nexus_objects),
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .workflow()
            .execution_cost(execution_id)
            .await
            .expect("Failed to fetch execution cost");

        assert_eq!(result.payment_id, *payment_ref.object_id());
        assert_eq!(result.max_budget, 100_000);
        assert_eq!(result.locked_budget, 100_000);
        assert_eq!(result.consumed, 42_000);
        assert_eq!(result.outstanding_locks, 0);
        assert!(result.accomplished);
        assert!(!result.refunded);
    }

    #[tokio::test]
    async fn test_abort_expired_execution_tool_gas_candidates_returns_empty_snapshot() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_ref = sui_mocks::mock_sui_object_ref();
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let payment_ref = sui_mocks::mock_sui_object_ref();
        let dag = dag_bcs(0);
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        mock_get_dag_execution_bcs(
            &mut ledger_service_mock,
            &nexus_objects,
            execution_ref.clone(),
            &dag_ref,
            vec![],
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            dag_ref,
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&dag).expect("DAG BCS should serialize"),
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<graph_move::Vertex>(
            &mut state_service_mock,
            vec![],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs::<
            graph_move::Vertex,
            linked_table::Node<graph_move::Vertex, graph_move::VertexInfo>,
        >(&mut ledger_service_mock, vec![]);
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                move_boundary::CLOCK_OBJECT_ID,
                1,
                sui::types::Digest::from([1; 32]),
            ),
            sui::types::Owner::Shared(1),
            clock_bcs(61_000),
        );
        sui_mocks::grpc::mock_list_dynamic_object_fields(
            &mut state_service_mock,
            vec![(
                DagExecutionPaymentFieldKey::default(),
                sui::types::Address::from_static("0xdf"),
                *payment_ref.object_id(),
            )],
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            payment_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&ExecutionPayment {
                id: crate::move_bindings::sui_framework::object::UID::new(*payment_ref.object_id()),
                execution_id: *execution_ref.object_id(),
                agent_id: crate::move_bindings::sui_framework::object::ID::new(
                    sui::types::Address::from_static("0xa"),
                ),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(7),
                payment_policy: SkillPaymentPolicy::UserFunded,
                source_kind:
                    crate::move_bindings::interface::payment::PaymentSourceKind::user_funded(
                        sui::types::Address::from_static("0x1"),
                    ),
                max_budget: 100_000,
                locked_budget: 0,
                funds: crate::move_bindings::sui_framework::balance::Balance {
                    value: 100_000,
                    phantom_t0: std::marker::PhantomData,
                },
                consumed: 0,
                accomplished: false,
                refunded: false,
                final_state: ExecutionPaymentFinalState::Pending,
                tool_cost_snapshot: crate::move_bindings::sui_framework::vec_map::VecMap {
                    contents: vec![],
                },
                locked_vertices: vec![],
            })
            .expect("execution payment bcs"),
            crate::move_bindings::struct_tag::<
                crate::move_bindings::interface::payment::ExecutionPayment,
            >(&nexus_objects),
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let candidates = client
            .workflow()
            .abort_expired_execution_tool_gas_candidates(*execution_ref.object_id())
            .await
            .expect("empty candidate snapshot should parse");

        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn test_abort_expired_execution_with_tool_gas_submits_selected_candidate() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let execution_ref = sui_mocks::mock_sui_object_ref();
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let payment_ref = sui_mocks::mock_sui_object_ref();
        let tool_fqn = fqn!("xyz.taluslabs.payable@1");
        let tool_gas_id = crate::move_bindings::derive_tool_gas_id(
            *nexus_objects.gas_service.object_id(),
            &tool_fqn,
        )
        .unwrap();
        let tool_gas_ref = sui_mocks::object_ref_for_id(tool_gas_id);
        let vertex = RuntimeVertex::plain("payable");
        let field_ref = sui_mocks::mock_sui_object_ref();
        let dag = dag_bcs(1);
        let execution_walks = vec![execution_move::DAGWalk::Active {
            next_vertex: vertex.clone(),
            timeout_ms: 30_000,
            requires_vertex_authorization_grant: false,
            created_at: 1_000,
        }];
        let payment_vertex_key =
            payment_vertex_key(*execution_ref.object_id(), &vertex, &tool_fqn).unwrap();
        let current_locked_vertices = vec![ExecutionPaymentVertexLock {
            vertex_key: payment_vertex_key.clone(),
            tool_fqn: tool_fqn.to_string().into_bytes(),
            amount: 10,
            settlement_kind: VertexExecutionPaymentSettlementKind::Paid,
        }];
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        mock_get_dag_execution_bcs(
            &mut ledger_service_mock,
            &nexus_objects,
            execution_ref.clone(),
            &dag_ref,
            execution_walks.clone(),
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&dag).expect("DAG BCS should serialize"),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(graph_move::Vertex::new("payable"), *field_ref.object_id())],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service_mock,
            vec![(
                field_ref,
                sui::types::Owner::Shared(1),
                graph_move::Vertex::new("payable"),
                offchain_vertex_node_bcs(&tool_fqn),
            )],
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                move_boundary::CLOCK_OBJECT_ID,
                1,
                sui::types::Digest::from([1; 32]),
            ),
            sui::types::Owner::Shared(1),
            clock_bcs(61_000),
        );
        sui_mocks::grpc::mock_list_dynamic_object_fields(
            &mut state_service_mock,
            vec![(
                DagExecutionPaymentFieldKey::default(),
                sui::types::Address::from_static("0xdf"),
                *payment_ref.object_id(),
            )],
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            payment_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&ExecutionPayment {
                id: crate::move_bindings::sui_framework::object::UID::new(*payment_ref.object_id()),
                execution_id: *execution_ref.object_id(),
                agent_id: crate::move_bindings::sui_framework::object::ID::new(
                    sui::types::Address::from_static("0xa"),
                ),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(7),
                payment_policy: SkillPaymentPolicy::UserFunded,
                source_kind:
                    crate::move_bindings::interface::payment::PaymentSourceKind::user_funded(
                        sui::types::Address::from_static("0x1"),
                    ),
                max_budget: 100_000,
                locked_budget: 10,
                funds: crate::move_bindings::sui_framework::balance::Balance {
                    value: 100_000,
                    phantom_t0: std::marker::PhantomData,
                },
                consumed: 0,
                accomplished: false,
                refunded: false,
                final_state: ExecutionPaymentFinalState::Pending,
                tool_cost_snapshot: crate::move_bindings::sui_framework::vec_map::VecMap {
                    contents: vec![],
                },
                locked_vertices: current_locked_vertices,
            })
            .expect("execution payment bcs"),
            crate::move_bindings::struct_tag::<
                crate::move_bindings::interface::payment::ExecutionPayment,
            >(&nexus_objects),
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(tool_gas_ref.version()),
            None,
        );
        mock_get_dag_execution_bcs(
            &mut ledger_service_mock,
            &nexus_objects,
            execution_ref.clone(),
            &dag_ref,
            execution_walks,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(dag_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            state_service_mock: Some(state_service_mock),
        });
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let client = NexusClient::builder()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_objects.clone())
            .with_gas(vec![gas_coin_ref], 1000)
            .build()
            .await
            .expect("mock client should build");

        let result = client
            .workflow()
            .abort_expired_execution_with_tool_gas(*execution_ref.object_id(), Some(tool_gas_id))
            .await
            .expect("abort transaction should submit");

        assert_eq!(result.tx_digest, tx_digest);
        assert_eq!(result.tx_checkpoint, 1);
        assert_eq!(result.dag_id, *dag_ref.object_id());
        assert_eq!(result.dag_execution_id, *execution_ref.object_id());
        assert_eq!(result.selected_candidate.tool_fqn, tool_fqn);
        assert_eq!(result.selected_candidate.tool_gas_ref, tool_gas_ref);
        assert_eq!(result.selected_candidate.matching_walks.len(), 1);
        assert_eq!(
            result.selected_candidate.matching_walks[0].payment_vertex_key,
            payment_vertex_key
        );
    }

    async fn mock_client_for_workflow_submit(
        nexus_objects: &crate::types::NexusObjects,
        gas_coin_ref: sui::types::ObjectReference,
        ledger_service_mock: sui_mocks::grpc::MockLedgerService,
        tx_service_mock: sui_mocks::grpc::MockTransactionExecutionService,
        sub_service_mock: sui_mocks::grpc::MockSubscriptionService,
    ) -> NexusClient {
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rand::thread_rng());
        NexusClient::builder()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_objects.clone())
            .with_gas(vec![gas_coin_ref], 1000)
            .build()
            .await
            .expect("mock client should build")
    }

    #[tokio::test]
    async fn test_abort_expired_execution_submits_workflow_transaction() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let execution_ref = sui_mocks::mock_sui_object_ref();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        mock_get_dag_execution_bcs(
            &mut ledger_service_mock,
            &nexus_objects,
            execution_ref.clone(),
            &dag_ref,
            vec![],
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                move_boundary::CLOCK_OBJECT_ID,
                1,
                sui::types::Digest::from([1; 32]),
            ),
            sui::types::Owner::Shared(1),
            clock_bcs(61_000),
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(dag_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );
        let client = mock_client_for_workflow_submit(
            &nexus_objects,
            gas_coin_ref,
            ledger_service_mock,
            tx_service_mock,
            sub_service_mock,
        )
        .await;

        let result = client
            .workflow()
            .abort_expired_execution(*execution_ref.object_id())
            .await
            .expect("abort transaction should submit");

        assert_eq!(result.tx_digest, tx_digest);
        assert_eq!(result.tx_checkpoint, 1);
        assert_eq!(result.dag_id, *dag_ref.object_id());
        assert_eq!(result.dag_execution_id, *execution_ref.object_id());
        assert!(result.cleaned_broken_onchain_results.is_empty());
    }

    #[tokio::test]
    async fn test_settle_committed_tool_result_for_walk_submits_workflow_transaction() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let execution_ref = sui_mocks::mock_sui_object_ref();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        mock_get_dag_execution_bcs(
            &mut ledger_service_mock,
            &nexus_objects,
            execution_ref.clone(),
            &dag_ref,
            vec![],
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(dag_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );
        let client = mock_client_for_workflow_submit(
            &nexus_objects,
            gas_coin_ref,
            ledger_service_mock,
            tx_service_mock,
            sub_service_mock,
        )
        .await;

        let result = client
            .workflow()
            .settle_committed_tool_result_for_walk(SettleCommittedToolResultParams {
                dag_execution_id: *execution_ref.object_id(),
                walk_index: 7,
            })
            .await
            .expect("settlement transaction should submit");

        assert_eq!(result.tx_digest, tx_digest);
        assert_eq!(result.tx_checkpoint, 1);
        assert_eq!(result.dag_id, *dag_ref.object_id());
        assert_eq!(result.dag_execution_id, *execution_ref.object_id());
        assert_eq!(result.walk_index, 7);
    }

    #[tokio::test]
    async fn test_leader_settlement_and_record_gas_paths_submit_transactions() {
        let mut rng = rand::thread_rng();
        let settle_digest = sui::types::Digest::generate(&mut rng);
        let record_digest = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let settle_gas_ref = sui_mocks::mock_sui_object_ref();
        let record_gas_ref = sui_mocks::mock_sui_object_ref();
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let execution_ref = sui_mocks::mock_sui_object_ref();
        let leader_cap_ref = sui_mocks::mock_sui_object_ref();

        let mut settle_ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut settle_tx = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut settle_sub = sui_mocks::grpc::MockSubscriptionService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut settle_ledger, 1000);
        mock_get_dag_execution_bcs(
            &mut settle_ledger,
            &nexus_objects,
            execution_ref.clone(),
            &dag_ref,
            vec![],
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut settle_ledger,
            dag_ref.clone(),
            sui::types::Owner::Shared(dag_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut settle_ledger,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut settle_ledger,
            leader_cap_ref.clone(),
            sui::types::Owner::Immutable,
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut settle_tx,
            &mut settle_sub,
            &mut settle_ledger,
            settle_digest,
            settle_gas_ref.clone(),
            vec![],
            vec![],
            vec![],
        );
        let settle_client = mock_client_for_workflow_submit(
            &nexus_objects,
            settle_gas_ref,
            settle_ledger,
            settle_tx,
            settle_sub,
        )
        .await;
        let settle_result = settle_client
            .workflow()
            .settle_committed_tool_result_for_walk_by_leader(
                SettleCommittedToolResultByLeaderParams {
                    dag_execution_id: *execution_ref.object_id(),
                    leader_cap_id: *leader_cap_ref.object_id(),
                    walk_index: 8,
                    commit_tx_digest: vec![1, 2, 3],
                    commit_gas_charge: 11,
                    settlement_gas_charge: 13,
                },
            )
            .await
            .expect("leader settlement transaction should submit");

        assert_eq!(settle_result.tx_digest, settle_digest);
        assert_eq!(settle_result.tx_checkpoint, 1);
        assert_eq!(settle_result.dag_id, *dag_ref.object_id());
        assert_eq!(settle_result.walk_index, 8);

        let mut record_ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut record_tx = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut record_sub = sui_mocks::grpc::MockSubscriptionService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut record_ledger, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut record_ledger,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut record_ledger,
            leader_cap_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0xabc")),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut record_tx,
            &mut record_sub,
            &mut record_ledger,
            record_digest,
            record_gas_ref.clone(),
            vec![],
            vec![],
            vec![],
        );
        let record_client = mock_client_for_workflow_submit(
            &nexus_objects,
            record_gas_ref,
            record_ledger,
            record_tx,
            record_sub,
        )
        .await;
        let record_result = record_client
            .workflow()
            .record_committed_tool_result_gas_charge_by_leader(
                RecordCommittedToolResultGasChargeParams {
                    dag_execution_id: *execution_ref.object_id(),
                    leader_cap_id: *leader_cap_ref.object_id(),
                    walk_index: 9,
                    commit_tx_digest: vec![4, 5, 6],
                    commit_gas_charge: 17,
                    settlement_gas_charge: 19,
                },
            )
            .await
            .expect("leader gas record transaction should submit");

        assert_eq!(record_result.tx_digest, record_digest);
        assert_eq!(record_result.tx_checkpoint, 1);
        assert_eq!(record_result.dag_execution_id, *execution_ref.object_id());
        assert_eq!(record_result.leader_cap_id, *leader_cap_ref.object_id());
        assert_eq!(record_result.walk_index, 9);
    }

    #[test]
    fn dag_vertex_requires_verifier_proof_prefers_vertex_override() {
        let mut dag = dag_bcs(0);
        dag.leader_verifier = VerifierConfig {
            mode: VerifierMode::LeaderRegisteredKey,
            method: "signed_http_v1".into(),
        };
        let mut vertex = offchain_vertex_info(&fqn!("xyz.example.tool@1"));
        vertex.leader_verifier = MoveOption::from_option(Some(VerifierConfig::default()));

        assert!(!dag_vertex_requires_verifier_proof(&dag, &vertex));
    }

    #[test]
    fn execution_terminal_record_matches_retryable_vertex_uses_typed_records() {
        let vertex = RuntimeVertex::with_iterator("v1", 2, 5);

        fn record(
            vertex: RuntimeVertex,
            failure_class: WorkflowFailureClass,
        ) -> execution_move::TerminalErrEvalRecord {
            execution_move::TerminalErrEvalRecord {
                walk_index: 9,
                vertex,
                leader: sui::types::Address::THREE,
                failure_class,
                outcome: MoveOption::from_option(None::<PostFailureAction>),
                reason: MoveString::from("retryable failure"),
                variant_ports_to_data: VecMap { contents: vec![] },
                err_eval_hash: vec![1, 2, 3],
            }
        }

        let retryable_records = VecMap {
            contents: vec![crate::move_bindings::sui_framework::vec_map::Entry {
                key: 9,
                value: record(vertex.clone(), WorkflowFailureClass::Retryable),
            }],
        };
        let terminal_records = VecMap {
            contents: vec![crate::move_bindings::sui_framework::vec_map::Entry {
                key: 9,
                value: record(
                    RuntimeVertex::plain("terminal_vertex"),
                    WorkflowFailureClass::TerminalSubmissionFailure,
                ),
            }],
        };

        assert!(execution_terminal_record_matches_retryable_vertex(
            &retryable_records,
            9,
            &vertex,
        ));
        assert!(!execution_terminal_record_matches_retryable_vertex(
            &retryable_records,
            10,
            &vertex,
        ));
        assert!(!execution_terminal_record_matches_retryable_vertex(
            &terminal_records,
            9,
            &RuntimeVertex::plain("terminal_vertex"),
        ));
    }

    #[test]
    fn fetch_vertex_input_port_names_reads_declared_ports_from_typed_vertex() {
        let mut vertex = offchain_vertex_info(&fqn!("xyz.example.tool@1"));
        vertex.input_ports = VecSet {
            contents: vec![
                graph_move::InputPort::new("z_port"),
                graph_move::InputPort::new("a_port"),
            ],
        };

        assert_eq!(
            vertex.declared_input_port_names(),
            vec!["a_port".to_string(), "z_port".to_string()]
        );
    }

    #[test]
    fn tool_gas_abort_filter_returns_exact_expired_locked_tool_vertices() {
        let execution_id = sui::types::Address::from_static("0xabc");
        let tool_fqn = fqn!("xyz.taluslabs.payable@1");
        let other_tool_fqn = fqn!("xyz.taluslabs.other@1");
        let payable_vertex = RuntimeVertex::plain("payable");
        let idle_vertex = RuntimeVertex::plain("idle");
        let matching_key = payment_vertex_key(execution_id, &payable_vertex, &tool_fqn).unwrap();
        let mut vertices = HashMap::new();
        vertices.insert(
            graph_move::Vertex::new("payable"),
            offchain_vertex_info(&tool_fqn),
        );
        vertices.insert(
            graph_move::Vertex::new("idle"),
            offchain_vertex_info(&other_tool_fqn),
        );
        let walks = vec![
            DAGWalk::Active {
                next_vertex: payable_vertex.clone(),
                timeout_ms: 30_000,
                requires_vertex_authorization_grant: false,
                created_at: 1_000,
            },
            DAGWalk::Active {
                next_vertex: idle_vertex,
                timeout_ms: 30_000,
                requires_vertex_authorization_grant: false,
                created_at: 61_000,
            },
            DAGWalk::PendingAbort {
                at_vertex: RuntimeVertex::plain("already_pending"),
            },
        ];
        let locks = vec![ExecutionPaymentVertexLock {
            vertex_key: matching_key.clone(),
            tool_fqn: tool_fqn.to_string().into_bytes(),
            amount: 10,
            settlement_kind: VertexExecutionPaymentSettlementKind::Paid,
        }];

        let candidates =
            filter_tool_gas_abort_candidate_walks(execution_id, &vertices, &walks, &locks, 61_000)
                .unwrap();
        let matching = candidates
            .get(&tool_fqn)
            .expect("candidate for locked tool");

        assert_eq!(candidates.len(), 1);
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].walk_index, 0);
        assert_eq!(matching[0].vertex, payable_vertex);
        assert_eq!(matching[0].payment_vertex_key, matching_key);
    }

    #[test]
    fn tool_gas_abort_filter_ignores_nonmatching_payment_locks() {
        let execution_id = sui::types::Address::from_static("0xabc");
        let tool_fqn = fqn!("xyz.taluslabs.payable@1");
        let other_tool_fqn = fqn!("xyz.taluslabs.other@1");
        let payable_vertex = RuntimeVertex::plain("payable");
        let matching_key = payment_vertex_key(execution_id, &payable_vertex, &tool_fqn).unwrap();
        let mut vertices = HashMap::new();
        vertices.insert(
            graph_move::Vertex::new("payable"),
            offchain_vertex_info(&tool_fqn),
        );
        let walks = vec![DAGWalk::Active {
            next_vertex: payable_vertex,
            timeout_ms: 30_000,
            requires_vertex_authorization_grant: false,
            created_at: 1_000,
        }];
        let locks = vec![
            ExecutionPaymentVertexLock {
                vertex_key: matching_key,
                tool_fqn: other_tool_fqn.to_string().into_bytes(),
                amount: 10,
                settlement_kind: VertexExecutionPaymentSettlementKind::Paid,
            },
            ExecutionPaymentVertexLock {
                vertex_key: vec![1, 2, 3],
                tool_fqn: tool_fqn.to_string().into_bytes(),
                amount: 10,
                settlement_kind: VertexExecutionPaymentSettlementKind::Paid,
            },
        ];

        let candidates =
            filter_tool_gas_abort_candidate_walks(execution_id, &vertices, &walks, &locks, 61_000)
                .unwrap();

        assert!(candidates.is_empty());
    }

    #[test]
    fn tool_gas_abort_filter_errors_when_expired_vertex_is_missing_from_dag() {
        let execution_id = sui::types::Address::from_static("0xabc");
        let walks = vec![DAGWalk::Active {
            next_vertex: RuntimeVertex::plain("missing"),
            timeout_ms: 30_000,
            requires_vertex_authorization_grant: false,
            created_at: 1_000,
        }];

        let error = filter_tool_gas_abort_candidate_walks(
            execution_id,
            &HashMap::new(),
            &walks,
            &[],
            61_000,
        )
        .expect_err("expired walk should require a fetched DAG vertex");

        assert!(error
            .to_string()
            .contains("DAG vertex 'missing' missing from fetched DAG"));
    }

    #[tokio::test]
    async fn fetch_tool_gas_refs_for_abort_candidates_derives_metadata_refs() {
        let gas_service_id = sui::types::Address::from_static("0xabc");
        let tool_fqn = fqn!("xyz.taluslabs.payable@1");
        let tool_gas_id =
            crate::move_bindings::derive_tool_gas_id(gas_service_id, &tool_fqn).unwrap();
        let tool_gas_ref = sui_mocks::object_ref_for_id(tool_gas_id);
        let candidate_walk = ToolGasAbortCandidateWalk {
            walk_index: 2,
            vertex: RuntimeVertex::plain("payable"),
            payment_vertex_key: vec![1, 2, 3],
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(tool_gas_ref.version()),
            None,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(std::sync::Arc::new(tokio::sync::Mutex::new(client)));
        let candidates = fetch_tool_gas_refs_for_abort_candidates(
            &crawler,
            gas_service_id,
            HashMap::from([(tool_fqn.clone(), vec![candidate_walk.clone()])]),
        )
        .await
        .expect("candidate refs should be fetched");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].tool_fqn, tool_fqn);
        assert_eq!(candidates[0].tool_gas_ref, tool_gas_ref);
        assert_eq!(candidates[0].matching_walks, vec![candidate_walk]);
    }

    #[test]
    fn select_tool_gas_abort_candidate_uses_first_or_required_candidate() {
        let first_id = sui::types::Address::from_static("0x111");
        let second_id = sui::types::Address::from_static("0x222");
        let candidates = vec![
            ToolGasAbortCandidate {
                tool_fqn: fqn!("xyz.taluslabs.first@1"),
                tool_gas_ref: sui::types::ObjectReference::new(
                    first_id,
                    1,
                    sui::types::Digest::default(),
                ),
                matching_walks: Vec::new(),
            },
            ToolGasAbortCandidate {
                tool_fqn: fqn!("xyz.taluslabs.second@1"),
                tool_gas_ref: sui::types::ObjectReference::new(
                    second_id,
                    1,
                    sui::types::Digest::default(),
                ),
                matching_walks: Vec::new(),
            },
        ];

        let selected = select_tool_gas_abort_candidate(candidates.clone(), None).unwrap();
        assert_eq!(*selected.tool_gas_ref.object_id(), first_id);

        let selected =
            select_tool_gas_abort_candidate(candidates.clone(), Some(second_id)).unwrap();
        assert_eq!(*selected.tool_gas_ref.object_id(), second_id);

        let missing = sui::types::Address::from_static("0x333");
        let error = select_tool_gas_abort_candidate(candidates, Some(missing)).unwrap_err();
        assert!(error
            .to_string()
            .contains("is not currently eligible to abort this execution"));

        let error = select_tool_gas_abort_candidate(Vec::new(), None).unwrap_err();
        assert!(error
            .to_string()
            .contains("No ToolGas abort candidates are currently eligible"));
    }
}
