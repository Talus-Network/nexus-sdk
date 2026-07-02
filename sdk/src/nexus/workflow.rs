//! Commands related to workflow management in Nexus.
//!
//! - [`WorkflowActions::publish`] to publish a [`Dag`] instance to Nexus.
//! - [`WorkflowActions::execute`] to execute a published DAG.
//! - [`WorkflowActions::inspect_execution`] to monitor the execution of a DAG.

use {
    crate::{
        events::{EventPage, NexusEvent, NexusEventKind},
        idents::{interface, sui_framework, workflow},
        nexus::{
            client::NexusClient,
            crawler::{Crawler, Response},
            error::NexusError,
            models::{
                BcsMap,
                CommittedToolResultLeaderRecordBcs,
                Dag,
                DagEdge,
                DagEdgeBcs,
                DagExecution,
                DagExecutionWalk,
                DagOutputVariantPort,
                DagVertexInfo,
                DagVertexInfoBcs,
                DagVertexInputPort,
                LinkedTableNodeBcs,
                RawNexusDataBcs,
            },
            tap,
        },
        sui,
        transactions::{dag, gas},
        types::{
            derive_tool_gas_id,
            deserialize_sui_u64,
            interface::{
                agent::SkillDagBinding,
                payment::{ExecutionPayment, ExecutionPaymentVertexLock},
            },
            payment_source_from_address,
            resolve_active_tap_skill_execution_target,
            resolve_default_tap_dag_executor,
            validate_execution_payment_options,
            workflow::execution_events::{
                EndStateReachedEvent,
                ExecutionFinishedEvent,
                TerminalErrEvalRecordedEvent,
            },
            AgentId,
            AgentRegistrySnapshot,
            Dag as JsonDag,
            DefaultDagExecutorRecord,
            FailureEvidenceKind,
            MoveOption,
            NexusData,
            NexusObjects,
            PortsData,
            RuntimeVertex,
            SkillId,
            SkillRevisionLookupKey,
            StorageConf,
            TypeName,
            VerifierConfig,
            VerifierMode,
            WorkflowFailureClass,
            DEFAULT_ENTRY_GROUP,
        },
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

#[derive(Clone, Debug)]
pub struct PublishResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_object_id: sui::types::Address,
}

pub struct ExecuteResult {
    pub tx_digest: sui::types::Digest,
    pub execution_object_id: sui::types::Address,
    pub tx_checkpoint: u64,
    pub tap_execution: Option<TapExecutionSubmitMetadata>,
}

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
pub struct RecordCommittedToolResultGasChargeResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_execution_id: sui::types::Address,
    pub leader_cap_id: sui::types::Address,
    pub walk_index: u64,
}

/// Dynamic-field key for `DAGExecution` committed tool results.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CommittedToolResultKey {
    pub walk_index: u64,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct CommittedToolResultBcs {
    expected_vertex: RuntimeVertex,
    #[allow(dead_code)]
    variant: TypeName,
    #[allow(dead_code)]
    variant_ports_to_data: BcsMap<TypeName, RawNexusDataBcs>,
    #[allow(dead_code)]
    failure_evidence_kind: MoveOption<FailureEvidenceKind>,
    primary_failure_evidence_kind: MoveOption<FailureEvidenceKind>,
    secondary_failure_evidence_kind: MoveOption<FailureEvidenceKind>,
    current_leader_cap_id: sui::types::Address,
    leader_records: BcsMap<sui::types::Address, CommittedToolResultLeaderRecordBcs>,
}

/// Narrowed committed-result view for off-chain freshness checks.
///
/// This is separate from `DagExecution` because callers only need committed-result state and
/// should not read or decode the full execution object for early wake decisions.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CommittedToolResultView {
    pub expected_vertex: RuntimeVertex,
    pub primary_failure_evidence_kind: Option<FailureEvidenceKind>,
    pub secondary_failure_evidence_kind: Option<FailureEvidenceKind>,
    pub current_leader_cap_id: sui::types::Address,
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

pub use crate::types::primitives::onchain_tool_result::OnchainToolResult;
type OnchainToolResultKey = crate::types::workflow::execution::OnchainToolResultKey;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
struct ExecutionPaymentInsufficientSettlementFieldKey {}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
struct ExecutionPaymentInsufficientSettlementBcs {
    walks: Vec<u64>,
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
    pub fn consume_object_input(&self) -> Option<sui::tx::ObjectInput> {
        self.consume_ready_result().map(|(_, object_ref)| {
            sui::tx::ObjectInput::shared(*object_ref.object_id(), object_ref.version(), true)
        })
    }
}

fn onchain_tool_result_is_finalized(result: &OnchainToolResult) -> bool {
    result.finalized
        && result.tag.0.is_some()
        && result.named_payload.0.is_some()
        && result.finalize_tx_digest.0.is_some()
        && result.finalize_recipient.0.is_some()
}

impl From<CommittedToolResultBcs> for CommittedToolResultView {
    fn from(value: CommittedToolResultBcs) -> Self {
        Self {
            expected_vertex: value.expected_vertex,
            primary_failure_evidence_kind: value.primary_failure_evidence_kind.0,
            secondary_failure_evidence_kind: value.secondary_failure_evidence_kind.0,
            current_leader_cap_id: value.current_leader_cap_id,
            leader_records: value
                .leader_records
                .contents
                .into_iter()
                .map(|entry| CommittedToolResultLeaderRecordView {
                    leader_cap_id: entry.key,
                    commit_tx_digest: entry.value.commit_tx_digest,
                    recipient: entry.value.recipient,
                    commit_gas_charge: entry.value.commit_gas_charge.0,
                    settlement_gas_charge: entry.value.settlement_gas_charge.0,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SuiClock {
    #[serde(deserialize_with = "deserialize_sui_u64")]
    timestamp_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub struct AgentDagExecuteOptions {
    pub payment_source: Vec<u8>,
    pub payment_coin: Option<sui::types::ObjectReference>,
    pub payment_coin_balance: Option<u64>,
    pub payment_max_budget: u64,
}

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

fn object_argument_from_metadata(
    tx: &mut sui::tx::TransactionBuilder,
    metadata: &Response<()>,
    mutable: bool,
) -> Result<sui::tx::Argument, NexusError> {
    match metadata.owner {
        sui::types::Owner::Shared(version) => Ok(tx.object(sui::tx::ObjectInput::shared(
            metadata.object_id,
            version,
            mutable,
        ))),
        sui::types::Owner::Immutable if !mutable => Ok(tx.object(sui::tx::ObjectInput::immutable(
            metadata.object_id,
            metadata.version,
            metadata.digest,
        ))),
        sui::types::Owner::Address(_) if !mutable => Ok(tx.object(sui::tx::ObjectInput::owned(
            metadata.object_id,
            metadata.version,
            metadata.digest,
        ))),
        ref owner => Err(NexusError::TransactionBuilding(anyhow!(
            "object '{}' has unsupported owner for transaction input: {owner:?}",
            metadata.object_id
        ))),
    }
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

pub fn dag_vertex_requires_verifier_proof(dag: &Dag, vertex: &DagVertexInfo) -> bool {
    verifier_mode_requires_proof(
        effective_verifier_config(&dag.leader_verifier, vertex.leader_verifier.as_ref()).mode,
    ) || verifier_mode_requires_proof(
        effective_verifier_config(&dag.tool_verifier, vertex.tool_verifier.as_ref()).mode,
    )
}

pub async fn fetch_dag_vertices_bcs(
    crawler: &Crawler,
    dag: &Dag,
) -> anyhow::Result<HashMap<crate::types::TypeName, DagVertexInfo>> {
    crawler
        .get_dynamic_fields_bcs::<
            crate::types::TypeName,
            LinkedTableNodeBcs<crate::types::TypeName, DagVertexInfoBcs>,
        >(dag.vertices.id(), dag.vertices.size())
        .await?
        .into_iter()
        .map(|(vertex, node)| Ok((vertex, node.value.into_sdk()?)))
        .collect()
}

/// Fetch the committed result for one walk from `DAGExecution` dynamic fields.
///
/// Returns `Ok(None)` when `CommittedToolResultKey { walk_index }` is absent.
pub async fn fetch_committed_tool_result_for_walk(
    crawler: &Crawler,
    nexus_objects: &NexusObjects,
    execution_id: sui::types::Address,
    walk_index: u64,
) -> anyhow::Result<Option<CommittedToolResultView>> {
    let expected_value_types = committed_tool_result_value_types(nexus_objects);
    crawler
        .get_optional_dynamic_field_bcs_matching_value_type::<
            CommittedToolResultKey,
            CommittedToolResultBcs,
        >(
            execution_id,
            CommittedToolResultKey { walk_index },
            &expected_value_types,
        )
        .await
        .map(|value| value.map(CommittedToolResultView::from))
}

pub async fn inspect_expired_walk_resolution(
    crawler: &Crawler,
    objects: &crate::types::NexusObjects,
    params: ResolveExpiredWalkParams,
) -> anyhow::Result<ExpiredWalkResolutionPlan> {
    let clock = crawler
        .get_object::<SuiClock>(sui_framework::CLOCK_OBJECT_ID)
        .await?
        .data;
    inspect_expired_walk_resolution_at(crawler, objects, params, clock.timestamp_ms).await
}

pub async fn inspect_expired_walk_resolution_at(
    crawler: &Crawler,
    objects: &crate::types::NexusObjects,
    params: ResolveExpiredWalkParams,
    clock_ms: u64,
) -> anyhow::Result<ExpiredWalkResolutionPlan> {
    let execution = crawler
        .get_object::<DagExecution>(params.dag_execution_id)
        .await?
        .data;
    let Some(walk) = usize::try_from(params.walk_index)
        .ok()
        .and_then(|index| execution.walks.get(index))
    else {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id,
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Skipped {
                reason: "walk index no longer exists in execution".to_string(),
            },
        });
    };

    let Some(timeout_vertex) = walk.timeout_expired_vertex(clock_ms) else {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id,
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Skipped {
                reason: "walk is not double-timeout expired or is already terminal".to_string(),
            },
        });
    };

    if fetch_committed_tool_result_for_walk(crawler, objects, params.dag_execution_id, params.walk_index)
        .await?
        .is_some()
    {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id,
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Settled,
        });
    }

    let Some(abort_vertex) = walk.abortable_timeout_expired_vertex(clock_ms) else {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id,
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
            kind: ExpiredWalkResolutionKind::Skipped {
                reason: format!(
                    "timeout-expired walk at vertex '{}' is not abortable without a committed result",
                    timeout_vertex.name().name
                ),
            },
        });
    };

    let payment = tap::fetch_execution_payment_for_execution(crawler, params.dag_execution_id)
        .await?
        .data;
    let dag = crawler.get_object::<Dag>(execution.dag_id).await?;
    let vertices = fetch_dag_vertices_bcs(crawler, &dag.data).await?;
    let vertex_info = vertices.get(&abort_vertex.name()).ok_or_else(|| {
        anyhow!(
            "DAG vertex '{}' missing from fetched DAG",
            abort_vertex.name().name
        )
    })?;
    let tool_fqn = vertex_info.kind.tool_fqn().clone();
    let vertex_key = payment_vertex_key(params.dag_execution_id, abort_vertex, &tool_fqn)?;
    let tool_fqn_bytes = tool_fqn.to_string().into_bytes();
    let locked = payment
        .locked_vertices
        .iter()
        .any(|lock| lock.vertex_key == vertex_key && lock.tool_fqn == tool_fqn_bytes);

    if !locked {
        return Ok(ExpiredWalkResolutionPlan {
            dag_id: execution.dag_id,
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
        dag_id: execution.dag_id,
        dag_execution_id: params.dag_execution_id,
        walk_index: params.walk_index,
        kind: ExpiredWalkResolutionKind::AbortedWithToolGas { selected_candidate },
    })
}

pub async fn fetch_onchain_tool_result_state_for_walk(
    crawler: &Crawler,
    nexus_objects: &NexusObjects,
    execution_id: sui::types::Address,
    walk_index: u64,
) -> anyhow::Result<OnchainToolResultState> {
    let committed_result =
        fetch_committed_tool_result_for_walk(crawler, nexus_objects, execution_id, walk_index)
            .await?;
    let insufficient_settlement = crawler
        .get_optional_dynamic_field_bcs::<
            ExecutionPaymentInsufficientSettlementFieldKey,
            ExecutionPaymentInsufficientSettlementBcs,
        >(execution_id, ExecutionPaymentInsufficientSettlementFieldKey {})
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
        .get_optional_dynamic_field_bcs::<OnchainToolResultKey, crate::types::ID>(
            execution_id,
            OnchainToolResultKey { walk_index },
        )
        .await?;
    let Some(result_id) = result_id else {
        return Ok(OnchainToolResultState::NoResult);
    };

    let result_id = result_id.bytes;
    let result_response = crawler
        .get_object_contents_bcs::<OnchainToolResult>(result_id)
        .await?;
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

fn committed_tool_result_value_types(nexus_objects: &NexusObjects) -> Vec<String> {
    let mut value_types = vec![committed_tool_result_value_type(
        nexus_objects.workflow_type_origin_pkg_id(),
    )];
    let current = committed_tool_result_value_type(nexus_objects.workflow_pkg_id);
    if !value_types.contains(&current) {
        value_types.push(current);
    }
    value_types
}

fn committed_tool_result_value_type(workflow_pkg_id: sui::types::Address) -> String {
    format!(
        "{workflow_pkg_id}::{}::{}",
        workflow::Execution::COMMITTED_TOOL_RESULT.module,
        workflow::Execution::COMMITTED_TOOL_RESULT.name
    )
}

pub async fn fetch_dag_default_values_bcs<T>(
    crawler: &Crawler,
    dag: &Dag,
) -> anyhow::Result<HashMap<DagVertexInputPort, T>>
where
    T: serde::de::DeserializeOwned,
{
    crawler
        .get_dynamic_fields_bcs::<DagVertexInputPort, T>(
            dag.defaults_to_input_ports.id(),
            dag.defaults_to_input_ports.size(),
        )
        .await
}

pub async fn fetch_dag_edges_bcs(
    crawler: &Crawler,
    dag: &Dag,
) -> anyhow::Result<HashMap<crate::types::TypeName, Vec<DagEdge>>> {
    crawler
        .get_dynamic_fields_bcs::<crate::types::TypeName, Vec<DagEdgeBcs>>(
            dag.edges.id(),
            dag.edges.size(),
        )
        .await?
        .into_iter()
        .map(|(vertex, edges)| {
            Ok((
                vertex,
                edges.into_iter().map(DagEdgeBcs::into_sdk).collect(),
            ))
        })
        .collect()
}

pub async fn fetch_dag_outputs_bcs(
    crawler: &Crawler,
    dag: &Dag,
) -> anyhow::Result<HashMap<crate::types::TypeName, Vec<DagOutputVariantPort>>> {
    crawler
        .get_dynamic_fields_bcs::<crate::types::TypeName, Vec<DagOutputVariantPort>>(
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
    let dag = crawler.get_object::<Dag>(dag_object_id).await?;
    let mut vertices = fetch_dag_vertices_bcs(crawler, &dag.data).await?;
    let vertex_name = next_vertex.name();
    let vertex = vertices
        .remove(&vertex_name)
        .ok_or_else(|| anyhow!("Vertex '{vertex_name}' not found in DAG verifier config"))?;

    Ok(dag_vertex_requires_verifier_proof(&dag.data, &vertex))
}

pub async fn fetch_vertex_input_port_names(
    crawler: &Crawler,
    dag: &Dag,
    vertex_name: &crate::types::TypeName,
) -> anyhow::Result<Vec<String>> {
    let mut vertices = fetch_dag_vertices_bcs(crawler, dag).await?;
    let vertex = vertices.remove(vertex_name).ok_or_else(|| {
        anyhow!("Vertex '{vertex_name}' not found in DAG vertices dynamic fields")
    })?;

    Ok(vertex.declared_input_port_names())
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct ExecutionErrEvalArbitrationState {
    #[serde(default)]
    terminal_records: serde_json::Value,
}

async fn fetch_execution_err_eval_state(
    crawler: &Crawler,
    object_id: sui::types::Address,
) -> anyhow::Result<Response<ExecutionErrEvalArbitrationState>> {
    crawler.get_object(object_id).await
}

pub fn execution_terminal_record_matches_retryable_vertex(
    value: &serde_json::Value,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
) -> anyhow::Result<bool> {
    let mut pending = vec![value];
    let walk_index_key = walk_index.to_string();
    let prioritized_nested_keys = ["contents", "entries", "fields", "inner", "vec", "value"];

    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(record_value) = object.get(&walk_index_key) {
                    if let Some(record) =
                        crate::types::parse_execution_terminal_record_value(record_value)?
                    {
                        return Ok(&record.vertex == next_vertex
                            && record.failure_class == WorkflowFailureClass::Retryable);
                    }
                }

                if let (Some(key), Some(record_value)) = (object.get("key"), object.get("value")) {
                    if crate::types::parse_u64_value(key)? == Some(walk_index) {
                        if let Some(record) =
                            crate::types::parse_execution_terminal_record_value(record_value)?
                        {
                            return Ok(&record.vertex == next_vertex
                                && record.failure_class == WorkflowFailureClass::Retryable);
                        }
                    }
                }

                for (name, nested) in object.iter().rev() {
                    if matches!(
                        name.as_str(),
                        "contents" | "entries" | "fields" | "inner" | "vec" | "value"
                    ) {
                        continue;
                    }

                    pending.push(nested);
                }

                for &nested_key in prioritized_nested_keys.iter().rev() {
                    if let Some(nested) = object.get(nested_key) {
                        pending.push(nested);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                for nested in values.iter().rev() {
                    pending.push(nested);
                }
            }
            _ => {}
        }
    }

    Ok(false)
}

pub async fn should_settle_tool_err_eval_gas(
    crawler: &Crawler,
    execution: sui::types::Address,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
) -> anyhow::Result<bool> {
    let state = fetch_execution_err_eval_state(crawler, execution).await?;
    execution_terminal_record_matches_retryable_vertex(
        &state.data.terminal_records,
        walk_index,
        next_vertex,
    )
}

impl WorkflowActions {
    /// Publish the provided JSON [`Dag`].
    pub async fn publish(&self, json_dag: JsonDag) -> Result<PublishResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        // == Craft and submit the publish DAG transaction ==

        let mut tx = sui::tx::TransactionBuilder::new();

        let mut dag_arg = dag::empty(&mut tx, nexus_objects);

        dag_arg = match dag::create(&mut tx, nexus_objects, dag_arg, json_dag) {
            Ok(dag_arg) => dag_arg,
            Err(e) => {
                return Err(NexusError::TransactionBuilding(e));
            }
        };

        dag::publish(&mut tx, nexus_objects, dag_arg);

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        // == Find the published DAG object ID ==

        let dag_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if *object_type.address() == nexus_objects.interface_pkg_id
                    && *object_type.module() == interface::Dag::DAG.module
                    && *object_type.name() == interface::Dag::DAG.name
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
    pub async fn execute(
        &self,
        dag_object_id: sui::types::Address,
        entry_data: HashMap<String, PortsData>,
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
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_default_agent_dag(
        &self,
        dag_object_id: sui::types::Address,
        entry_data: HashMap<String, PortsData>,
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
            .get_object::<Dag>(dag_object_id)
            .await
            .map_err(NexusError::Rpc)?;

        let tools_gas = self.client.fetch_tool_gas_for_dag(&dag.data).await?;

        let registry = tap::fetch_configured_agent_registry(self.client.crawler(), nexus_objects)
            .await
            .map_err(NexusError::Rpc)?;
        let default_executor = resolve_default_agent_dag_executor(nexus_objects, &registry.data)
            .map_err(NexusError::Parsing)?;

        let mut tx = sui::tx::TransactionBuilder::new();
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

        if let Err(e) = dag::execute_default_agent_dag(
            &mut tx,
            nexus_objects,
            &dag.object_ref(),
            priority_fee_per_gas_unit,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &transaction_input_data,
            &agent_execution,
            &tools_gas,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let owned_payment_coin = agent_execution
            .payment_coin
            .as_ref()
            .map(|payment_coin| *payment_coin.object_id());

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;
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

        let execution_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if nexus_objects.is_workflow_package(*object_type.address())
                    && *object_type.module() == workflow::Execution::DAG_EXECUTION.module
                    && *object_type.name() == workflow::Execution::DAG_EXECUTION.name
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
    /// calls the explicit agent workflow entry instead of the legacy default-agent entry.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_agent_dag(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        entry_data: HashMap<String, PortsData>,
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
            .get_object::<Dag>(dag_id)
            .await
            .map_err(NexusError::Rpc)?;

        let tools_gas = self.client.fetch_tool_gas_for_dag(&dag.data).await?;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::tx::TransactionBuilder::new();
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

        if let Err(e) = dag::execute_agent_dag(
            &mut tx,
            nexus_objects,
            &dag.object_ref(),
            tap::agent_input_from_metadata(&agent_object)
                .map_err(NexusError::TransactionBuilding)?,
            priority_fee_per_gas_unit,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &transaction_input_data,
            &agent_execution,
            &tools_gas,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let owned_payment_coin = agent_execution
            .payment_coin
            .as_ref()
            .map(|payment_coin| *payment_coin.object_id());

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;
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

        let execution_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if nexus_objects.is_workflow_package(*object_type.address())
                    && *object_type.module() == workflow::Execution::DAG_EXECUTION.module
                    && *object_type.name() == workflow::Execution::DAG_EXECUTION.name
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
            .get_object::<DagExecution>(dag_execution_id)
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
    pub async fn abort_expired_execution(
        &self,
        dag_execution_id: sui::types::Address,
    ) -> Result<AbortExecutionResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DagExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let execution_ref = crawler
            .get_object_metadata(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        dag::abort_expired_execution(
            &mut tx,
            &self.client.nexus_objects,
            &dag_ref,
            &execution_ref,
        );
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(AbortExecutionResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id,
            dag_execution_id,
        })
    }

    /// Submit permissionless committed-result settlement for one walk.
    pub async fn settle_committed_tool_result_for_walk(
        &self,
        params: SettleCommittedToolResultParams,
    ) -> Result<CommittedToolResultSettlementResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DagExecution>(params.dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id)
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
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.object(sui::tx::ObjectInput::shared(
            *dag_ref.object_id(),
            dag_ref.version(),
            false,
        ));
        let execution_arg = tx.object(sui::tx::ObjectInput::shared(
            *execution_ref.object_id(),
            execution_ref.version(),
            true,
        ));
        let tool_registry = tx.object(sui::tx::ObjectInput::shared(
            *objects.tool_registry.object_id(),
            objects.tool_registry.version(),
            false,
        ));
        let clock = tx.object(sui::tx::ObjectInput::shared(
            sui_framework::CLOCK_OBJECT_ID,
            1,
            false,
        ));
        dag::settle_committed_tool_result_for_walk(
            &mut tx,
            objects,
            dag,
            execution_arg,
            tool_registry,
            params.walk_index,
            clock,
        );
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(CommittedToolResultSettlementResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id,
            dag_execution_id: params.dag_execution_id,
            walk_index: params.walk_index,
        })
    }

    /// Resolve one double-timeout walk by settling committed results or aborting no-result walks.
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
                    ..base(ExpiredWalkResolutionKind::AbortedWithToolGas {
                        selected_candidate: aborted.selected_candidate,
                    })
                })
            }
            ExpiredWalkResolutionKind::Skipped { reason } => {
                Ok(base(ExpiredWalkResolutionKind::Skipped { reason }))
            }
        }
    }

    /// Submit leader-authenticated committed-result settlement with the leader's commit gas charge.
    pub async fn settle_committed_tool_result_for_walk_by_leader(
        &self,
        params: SettleCommittedToolResultByLeaderParams,
    ) -> Result<CommittedToolResultSettlementResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DagExecution>(params.dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id)
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
        let mut tx = sui::tx::TransactionBuilder::new();
        let dag = tx.object(sui::tx::ObjectInput::shared(
            *dag_ref.object_id(),
            dag_ref.version(),
            false,
        ));
        let execution_arg = object_argument_from_metadata(&mut tx, &execution_ref, true)?;
        let tool_registry = tx.object(sui::tx::ObjectInput::shared(
            *objects.tool_registry.object_id(),
            objects.tool_registry.version(),
            false,
        ));
        let leader_cap = object_argument_from_metadata(&mut tx, &leader_cap_ref, false)?;
        let clock = tx.object(sui::tx::ObjectInput::shared(
            sui_framework::CLOCK_OBJECT_ID,
            1,
            false,
        ));
        dag::settle_committed_tool_result_for_walk_by_leader(
            &mut tx,
            objects,
            dag,
            execution_arg,
            tool_registry,
            leader_cap,
            params.walk_index,
            params.commit_tx_digest,
            params.commit_gas_charge,
            params.settlement_gas_charge,
            clock,
        );
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(CommittedToolResultSettlementResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id,
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
        let mut tx = sui::tx::TransactionBuilder::new();
        let execution_arg = object_argument_from_metadata(&mut tx, &execution_ref, true)?;
        let leader_cap = object_argument_from_metadata(&mut tx, &leader_cap_ref, false)?;
        dag::record_committed_tool_result_gas_charge_by_leader(
            &mut tx,
            &self.client.nexus_objects,
            execution_arg,
            leader_cap,
            params.walk_index,
            params.commit_tx_digest,
            params.commit_gas_charge,
            params.settlement_gas_charge,
        );
        let response = self.client.submit_transaction(tx, address).await?;

        Ok(RecordCommittedToolResultGasChargeResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_execution_id: params.dag_execution_id,
            leader_cap_id: params.leader_cap_id,
            walk_index: params.walk_index,
        })
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
            .get_object::<DagExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag = crawler
            .get_object::<Dag>(execution.dag_id)
            .await
            .map_err(NexusError::Rpc)?;
        let vertices = fetch_dag_vertices_bcs(crawler, &dag.data)
            .await
            .map_err(NexusError::Rpc)?;
        let clock = crawler
            .get_object::<SuiClock>(sui_framework::CLOCK_OBJECT_ID)
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
            .get_object::<DagExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let dag_ref = crawler
            .get_object_metadata(execution.dag_id)
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
        let mut tx = sui::tx::TransactionBuilder::new();
        gas::abort_expired_execution_with_tool_gas(
            &mut tx,
            nexus_objects,
            &selected_candidate.tool_gas_ref,
            &dag_ref,
            &execution_ref,
        );

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);
        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let signature = self.client.signer.sign_tx(&tx).await?;
        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(AbortExpiredExecutionResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_id: execution.dag_id,
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
    vertices: &HashMap<crate::types::TypeName, DagVertexInfo>,
    walks: &[DagExecutionWalk],
    locks: &[ExecutionPaymentVertexLock],
    clock_ms: u64,
) -> anyhow::Result<HashMap<crate::ToolFqn, Vec<ToolGasAbortCandidateWalk>>> {
    let mut candidates = HashMap::<crate::ToolFqn, Vec<ToolGasAbortCandidateWalk>>::new();
    for (walk_index, walk) in walks.iter().enumerate() {
        let Some(vertex) = walk.abortable_timeout_expired_vertex(clock_ms) else {
            continue;
        };
        let vertex_info = vertices.get(&vertex.name()).ok_or_else(|| {
            anyhow!(
                "DAG vertex '{}' missing from fetched DAG",
                vertex.name().name
            )
        })?;
        let tool_fqn = vertex_info.kind.tool_fqn().clone();
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
        let tool_gas_id =
            derive_tool_gas_id(gas_service_id, &tool_fqn).map_err(NexusError::Parsing)?;
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
            nexus::{
                crawler::{DynamicMap, Set},
                models::{Dag, DagVertexInfo, DagVertexKind},
            },
            sui::traits::*,
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                derive_tool_gas_id,
                interface::{
                    agent::{Agent, SkillDagBinding, SkillRequirement, SkillSchedulePolicy},
                    payment::{
                        ExecutionPaymentFinalState,
                        SkillPaymentPolicy,
                        VertexExecutionPaymentSettlementKind,
                    },
                    version::InterfaceVersion,
                },
                registry::agent_registry::{
                    AgentRecord,
                    AgentRegistry,
                    DefaultDagExecutor,
                    DefaultDagExecutorFieldKey,
                    SkillRecord,
                },
                workflow::{
                    execution::DagExecutionPaymentFieldKey,
                    execution_events::{
                        EndStateReachedEvent,
                        ExecutionFinishedEvent,
                        TerminalErrEvalRecordedEvent,
                        WalkAdvancedEvent,
                    },
                },
                AgentRegistrySnapshot,
                DefaultDagExecutorTarget,
                MoveString,
                MoveTable,
                NexusData,
                PostFailureAction,
                RuntimeVertex,
                SkillRecordContext,
                TypeName,
                VerifierConfig,
                VerifierMode,
                WorkflowFailureClass,
            },
        },
        serde::Serialize,
        serde_json::json,
        std::sync::Arc,
        tokio::sync::Mutex,
    };

    #[derive(Clone, Debug, Serialize)]
    struct DynamicFieldValueBcs<K, V> {
        id: sui::types::Address,
        name: K,
        value: V,
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
    ) -> CommittedToolResultBcs {
        CommittedToolResultBcs {
            expected_vertex,
            variant: TypeName::new("ok"),
            variant_ports_to_data: BcsMap { contents: vec![] },
            failure_evidence_kind: MoveOption(primary_failure.clone()),
            primary_failure_evidence_kind: MoveOption(primary_failure),
            secondary_failure_evidence_kind: MoveOption(secondary_failure),
            current_leader_cap_id: primary_leader,
            leader_records: BcsMap {
                contents: vec![crate::nexus::models::BcsMapEntry {
                    key: primary_leader,
                    value: CommittedToolResultLeaderRecordBcs {
                        commit_tx_digest: vec![1, 2, 3],
                        recipient: sui::types::Address::from_static("0x44"),
                        commit_gas_charge: MoveOption(Some(10)),
                        settlement_gas_charge: MoveOption(None),
                    },
                }],
            },
        }
    }

    fn raw_inline_nexus_data_bcs(one: impl Into<Vec<u8>>) -> RawNexusDataBcs {
        RawNexusDataBcs {
            storage: b"inline".to_vec(),
            one: one.into(),
            many: vec![],
        }
    }

    fn object_id(bytes: sui::types::Address) -> crate::types::sui_framework::object::ID {
        crate::types::sui_address_to_id(bytes)
    }

    fn output_variant(name: &str) -> crate::types::interface::graph::OutputVariant {
        crate::types::interface::graph::OutputVariant {
            name: MoveString::from(name),
        }
    }

    fn ports_data_map(
        entries: Vec<(&str, serde_json::Value)>,
    ) -> crate::types::sui_framework::vec_map::VecMap<
        crate::types::interface::graph::OutputPort,
        crate::types::primitives::data::NexusData,
    > {
        crate::types::sui_framework::vec_map::VecMap {
            contents: entries
                .into_iter()
                .map(
                    |(name, value)| crate::types::sui_framework::vec_map::Entry {
                        key: crate::types::interface::graph::OutputPort {
                            name: MoveString::from(name),
                        },
                        value: crate::types::primitives::data::NexusData {
                            storage: b"inline".to_vec(),
                            one: serde_json::to_vec(&value).expect("inline JSON encodes"),
                            many: vec![],
                        },
                    },
                )
                .collect(),
        }
    }

    fn dynamic_field_struct_tag() -> sui::types::StructTag {
        sui::types::StructTag::new(
            sui::types::Address::TWO,
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![],
        )
    }

    fn onchain_tool_result_bcs(
        result_id: sui::types::Address,
        execution_id: sui::types::Address,
        finalized: bool,
    ) -> OnchainToolResult {
        OnchainToolResult {
            id: crate::types::sui_address_to_uid(result_id),
            execution_id: object_id(execution_id),
            finalized,
            stamps: MoveOption(
                finalized
                    .then_some(crate::types::sui_framework::vec_map::VecMap { contents: vec![] }),
            ),
            tag: MoveOption(finalized.then_some(b"ok".to_vec())),
            named_payload: MoveOption(
                finalized
                    .then_some(crate::types::sui_framework::vec_map::VecMap { contents: vec![] }),
            ),
            finalize_tx_digest: MoveOption(finalized.then_some(vec![1, 2, 3])),
            finalize_recipient: MoveOption(
                finalized.then_some(sui::types::Address::from_static("0x45")),
            ),
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
        sui_mocks::grpc::mock_list_dynamic_fields::<CommittedToolResultKey>(
            &mut state_service_mock,
            vec![],
        );
        let crawler = crawler_from_mocks(
            sui_mocks::grpc::MockLedgerService::new(),
            state_service_mock,
        )
        .await;
        let nexus_objects = sui_mocks::mock_nexus_objects();

        let result =
            fetch_committed_tool_result_for_walk(&crawler, &nexus_objects, execution_id, 7)
                .await
                .expect("fetch should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_committed_tool_result_for_walk_ignores_same_shape_foreign_field() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf9"));
        let field_id = *field_ref.object_id();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        let nexus_objects = sui_mocks::mock_nexus_objects();

        state_service_mock
            .expect_list_dynamic_fields()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                let mut same_shape_foreign = sui::grpc::DynamicField::default();
                same_shape_foreign.set_field_id(field_id.to_string());
                same_shape_foreign.set_name(
                    bcs::to_bytes(&CommittedToolResultKey { walk_index: 7 })
                        .expect("committed key bcs"),
                );
                same_shape_foreign.set_value_type("0xbad::execution::ForeignCommittedToolResult");
                response.set_dynamic_fields(vec![same_shape_foreign]);
                Ok(tonic::Response::new(response))
            });

        ledger_service_mock.expect_get_object().times(0);
        let crawler = crawler_from_mocks(ledger_service_mock, state_service_mock).await;

        let result =
            fetch_committed_tool_result_for_walk(&crawler, &nexus_objects, execution_id, 7)
                .await
                .expect("foreign same-shape field should not block lookup");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_committed_tool_result_for_walk_skips_unrelated_keys_and_decodes_match() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf1"));
        let field_id = *field_ref.object_id();
        let primary_leader = sui::types::Address::from_static("0xa1");
        let secondary_leader = sui::types::Address::from_static("0xa2");
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let committed_value_type =
            committed_tool_result_value_type(nexus_objects.workflow_type_origin_pkg_id());
        let committed = committed_tool_result_bcs(
            RuntimeVertex::plain("retryable"),
            primary_leader,
            secondary_leader,
            Some(FailureEvidenceKind::ToolEvidence),
            None,
        );
        let field_value = DynamicFieldValueBcs {
            id: sui::types::Address::from_static("0xdf"),
            name: CommittedToolResultKey { walk_index: 7 },
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
                    bcs::to_bytes(&CommittedToolResultKey { walk_index: 7 })
                        .expect("committed key bcs"),
                );
                wanted.set_value_type(committed_value_type.clone());
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

        let result =
            fetch_committed_tool_result_for_walk(&crawler, &nexus_objects, execution_id, 7)
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
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let committed_value_type =
            committed_tool_result_value_type(nexus_objects.workflow_type_origin_pkg_id());
        let mut committed = committed_tool_result_bcs(
            RuntimeVertex::plain("retryable"),
            primary_leader,
            secondary_leader,
            Some(FailureEvidenceKind::ToolEvidence),
            None,
        );
        committed.variant_ports_to_data = BcsMap {
            contents: vec![crate::nexus::models::BcsMapEntry {
                key: TypeName::new("reason"),
                value: raw_inline_nexus_data_bcs(b"not-json".to_vec()),
            }],
        };
        let field_value = DynamicFieldValueBcs {
            id: sui::types::Address::from_static("0xdf"),
            name: CommittedToolResultKey { walk_index: 7 },
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
                    bcs::to_bytes(&CommittedToolResultKey { walk_index: 7 })
                        .expect("committed key bcs"),
                );
                wanted.set_value_type(committed_value_type.clone());
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

        let result =
            fetch_committed_tool_result_for_walk(&crawler, &nexus_objects, execution_id, 7)
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

    #[tokio::test]
    async fn fetch_onchain_tool_result_state_returns_committed_before_no_result() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf1"));
        let field_id = *field_ref.object_id();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let committed_value_type =
            committed_tool_result_value_type(nexus_objects.workflow_type_origin_pkg_id());
        let committed = committed_tool_result_bcs(
            RuntimeVertex::plain("done"),
            sui::types::Address::from_static("0xa1"),
            sui::types::Address::from_static("0xa2"),
            None,
            None,
        );
        let field_value = DynamicFieldValueBcs {
            id: *field_ref.object_id(),
            name: CommittedToolResultKey { walk_index: 7 },
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
                    bcs::to_bytes(&CommittedToolResultKey { walk_index: 7 })
                        .expect("committed key bcs"),
                );
                wanted.set_value_type(committed_value_type.clone());
                response.set_dynamic_fields(vec![wanted]);
                Ok(tonic::Response::new(response))
            });
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            field_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&field_value).expect("field value bcs"),
            dynamic_field_struct_tag(),
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<ExecutionPaymentInsufficientSettlementFieldKey>(
            &mut state_service_mock,
            vec![],
        );
        let crawler = crawler_from_mocks(ledger_service_mock, state_service_mock).await;

        let state =
            fetch_onchain_tool_result_state_for_walk(&crawler, &nexus_objects, execution_id, 7)
                .await
                .expect("state fetch should succeed");

        assert!(matches!(state, OnchainToolResultState::Committed { .. }));
    }

    #[tokio::test]
    async fn fetch_onchain_tool_result_state_decodes_finalized_result() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let result_id = sui::types::Address::from_static("0xbeef");
        let result_ref = sui_mocks::object_ref_for_id(result_id);
        let result_version = result_ref.version();
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf2"));
        let field_value = DynamicFieldValueBcs {
            id: *field_ref.object_id(),
            name: OnchainToolResultKey { walk_index: 7 },
            value: object_id(result_id),
        };
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields::<CommittedToolResultKey>(
            &mut state_service_mock,
            vec![],
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<ExecutionPaymentInsufficientSettlementFieldKey>(
            &mut state_service_mock,
            vec![],
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<OnchainToolResultKey>(
            &mut state_service_mock,
            vec![(
                OnchainToolResultKey { walk_index: 7 },
                *field_ref.object_id(),
            )],
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            field_ref.clone(),
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&field_value).expect("field value bcs"),
            dynamic_field_struct_tag(),
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            result_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&onchain_tool_result_bcs(result_id, execution_id, true))
                .expect("result bcs"),
            sui::types::StructTag::new(
                sui::types::Address::from_static("0xa1"),
                sui::types::Identifier::from_static("onchain_tool_result"),
                sui::types::Identifier::from_static("OnchainToolResult"),
                vec![],
            ),
        );
        let crawler = crawler_from_mocks(ledger_service_mock, state_service_mock).await;

        let state =
            fetch_onchain_tool_result_state_for_walk(&crawler, &nexus_objects, execution_id, 7)
                .await
                .expect("state fetch should succeed");

        let (consume_ready, consume_ref) = state
            .consume_ready_result()
            .expect("finalized result should be consume-ready");
        assert_eq!(consume_ready.id.id.bytes, result_id);
        assert_eq!(consume_ready.execution_id.bytes, execution_id);
        assert_eq!(consume_ref.object_id(), &result_id);
        let mut tx = sui::tx::TransactionBuilder::new();
        tx.object(
            state
                .consume_object_input()
                .expect("finalized result should expose consume input"),
        );
        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { inputs, .. },
        ) = tx.kind
        else {
            panic!("expected programmable transaction");
        };
        let shared = inputs
            .iter()
            .find_map(|input| match input {
                sui::types::Input::Shared(shared) if shared.object_id() == result_id => {
                    Some(shared)
                }
                _ => None,
            })
            .expect("expected consume object input to be shared");
        assert_eq!(shared.object_id(), result_id);
        assert_eq!(shared.version(), result_version);
        assert!(shared.mutability().is_mutable());

        let OnchainToolResultState::Finalized { result, object_ref } = state else {
            panic!("expected finalized state");
        };
        assert_eq!(result.id.id.bytes, result_id);
        assert_eq!(result.execution_id.bytes, execution_id);
        assert_eq!(result.tag.0, Some(b"ok".to_vec()));
        assert_eq!(result.finalize_tx_digest.0, Some(vec![1, 2, 3]));
        assert_eq!(object_ref.object_id(), &result_id);
    }

    #[tokio::test]
    async fn fetch_onchain_tool_result_state_surfaces_empty_as_invalid() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let result_id = sui::types::Address::from_static("0xbeef");
        let result_ref = sui_mocks::object_ref_for_id(result_id);
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf2"));
        let field_value = DynamicFieldValueBcs {
            id: *field_ref.object_id(),
            name: OnchainToolResultKey { walk_index: 7 },
            value: object_id(result_id),
        };
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields::<CommittedToolResultKey>(
            &mut state_service_mock,
            vec![],
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<ExecutionPaymentInsufficientSettlementFieldKey>(
            &mut state_service_mock,
            vec![],
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<OnchainToolResultKey>(
            &mut state_service_mock,
            vec![(
                OnchainToolResultKey { walk_index: 7 },
                *field_ref.object_id(),
            )],
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            field_ref,
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&field_value).expect("field value bcs"),
            dynamic_field_struct_tag(),
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            result_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&onchain_tool_result_bcs(result_id, execution_id, false))
                .expect("result bcs"),
            sui::types::StructTag::new(
                sui::types::Address::from_static("0xa1"),
                sui::types::Identifier::from_static("onchain_tool_result"),
                sui::types::Identifier::from_static("OnchainToolResult"),
                vec![],
            ),
        );
        let crawler = crawler_from_mocks(ledger_service_mock, state_service_mock).await;

        let state =
            fetch_onchain_tool_result_state_for_walk(&crawler, &nexus_objects, execution_id, 7)
                .await
                .expect("state fetch should succeed");

        assert!(state.consume_ready_result().is_none());
        assert!(state.consume_object_input().is_none());
        assert!(matches!(
            state,
            OnchainToolResultState::InvalidEmpty { result_id: id, .. } if id == result_id
        ));
    }

    #[tokio::test]
    async fn fetch_onchain_tool_result_state_surfaces_insufficient_settlement_marker() {
        let execution_id = sui::types::Address::from_static("0xe1");
        let marker_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xf3"));
        let field_value = DynamicFieldValueBcs {
            id: *marker_ref.object_id(),
            name: ExecutionPaymentInsufficientSettlementFieldKey {},
            value: ExecutionPaymentInsufficientSettlementBcs { walks: vec![7] },
        };
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields::<CommittedToolResultKey>(
            &mut state_service_mock,
            vec![],
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<ExecutionPaymentInsufficientSettlementFieldKey>(
            &mut state_service_mock,
            vec![(
                ExecutionPaymentInsufficientSettlementFieldKey {},
                *marker_ref.object_id(),
            )],
        );
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            marker_ref,
            sui::types::Owner::Shared(0),
            bcs::to_bytes(&field_value).expect("field value bcs"),
            dynamic_field_struct_tag(),
        );
        let crawler = crawler_from_mocks(ledger_service_mock, state_service_mock).await;

        let state =
            fetch_onchain_tool_result_state_for_walk(&crawler, &nexus_objects, execution_id, 7)
                .await
                .expect("state fetch should succeed");

        assert!(matches!(
            state,
            OnchainToolResultState::InsufficientSettlement { marker, .. }
                if marker.walks == vec![7]
        ));
    }

    fn offchain_vertex_node_bcs(
        tool_fqn: &crate::ToolFqn,
    ) -> LinkedTableNodeBcs<TypeName, DagVertexInfoBcs> {
        LinkedTableNodeBcs {
            prev: crate::types::MoveOption(None::<TypeName>),
            next: crate::types::MoveOption(None::<TypeName>),
            value: DagVertexInfoBcs {
                kind: crate::nexus::models::DagVertexKindBcs::OffChain {
                    _variant_name: "OffChain".to_string(),
                    tool_fqn: tool_fqn.to_string(),
                },
                input_ports: crate::types::MoveVecSet { contents: vec![] },
                post_failure_action: crate::types::MoveOption(
                    None::<crate::nexus::models::PostFailureActionBcs>,
                ),
                leader_verifier: crate::types::MoveOption(
                    None::<crate::nexus::models::VerifierConfigBcs>,
                ),
                tool_verifier: crate::types::MoveOption(
                    None::<crate::nexus::models::VerifierConfigBcs>,
                ),
            },
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
                id: crate::types::sui_address_to_uid(registry.id),
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
            sui::types::StructTag::new(
                nexus_objects.registry_pkg_id,
                crate::idents::registry::AGENT_REGISTRY_MODULE,
                sui::types::Identifier::from_static("AgentRegistry"),
                vec![],
            ),
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
            .returning(move |_request| {
                let mut response = sui::grpc::GetCheckpointResponse::default();
                let mut checkpoint = sui::grpc::Checkpoint::default();
                let mut transactions = vec![];
                for _ in 0..10 {
                    let mut transaction = sui::grpc::ExecutedTransaction::default();
                    transaction.set_digest(sui::types::Digest::ZERO);
                    transactions.push(transaction);
                }
                checkpoint.set_transactions(transactions);
                checkpoint.set_sequence_number(cp);
                response.set_checkpoint(checkpoint);
                Ok(tonic::Response::new(response))
            });

        ledger_service
            .expect_batch_get_transactions()
            .returning(move |_request| {
                let mut response = sui::grpc::BatchGetTransactionsResponse::default();
                let mut result = sui::grpc::GetTransactionResult::default();
                let mut transaction = sui::grpc::ExecutedTransaction::default();
                transaction.set_digest(sui::types::Digest::ZERO);
                transaction.set_checkpoint(1);
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
                let mut tx_events = sui::grpc::TransactionEvents::default();
                tx_events.set_events(events);
                transaction.set_events(tx_events);
                result.set_transaction(transaction);
                response.set_transactions(vec![result]);
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
                    sui::types::StructTag::new(
                        nexus_objects.interface_pkg_id,
                        interface::Dag::DAG.module,
                        interface::Dag::DAG.name,
                        vec![],
                    ),
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

        let dag = JsonDag {
            vertices: vec![],
            edges: vec![],
            default_values: None,
            post_failure_action: None,
            leader_verifier: None,
            tool_verifier: None,
            entry_groups: None,
            outputs: None,
        };

        let result = client
            .workflow()
            .publish(dag)
            .await
            .expect("Failed to publish DAG");

        assert_eq!(result.dag_object_id, dag_object_id);
        assert_eq!(result.tx_digest, digest);
        assert_eq!(result.tx_checkpoint, 1);
    }

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
        let dag = Dag {
            vertices: DynamicMap::new(sui_mocks::mock_sui_address(), 1),
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig::default(),
            tool_verifier: VerifierConfig::default(),
        };

        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            json!(dag),
        );

        // Dag.vertices
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(TypeName::new("ToolVertex"), *tool_gas_ref.object_id())],
        );

        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service_mock,
            vec![(
                tool_gas_ref.clone(),
                sui::types::Owner::Shared(0),
                TypeName::new("ToolVertex"),
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
            PortsData::from_map(HashMap::from([
                (
                    "entry_port".to_string(),
                    NexusData::new_inline(json!("data")),
                ),
                (
                    "another_entry_port".to_string(),
                    NexusData::new_inline(json!("data")),
                ),
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

    #[tokio::test]
    async fn test_workflow_actions_execute_agent_dag_pinned_skill() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let tool_fqn = fqn!("xyz.taluslabs.standard_tap@1");
        let tool_gas_id = derive_tool_gas_id(*nexus_objects.gas_service.object_id(), &tool_fqn)
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
        let dag = Dag {
            vertices: DynamicMap::new(sui_mocks::mock_sui_address(), 1),
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig::default(),
            tool_verifier: VerifierConfig::default(),
        };
        mock_fetch_registry_from_tables(
            &mut ledger_service_mock,
            &mut state_service_mock,
            &nexus_objects,
            &agent_registry,
        );
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            json!(dag),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(TypeName::new("ToolVertex"), *tool_gas_ref.object_id())],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service_mock,
            vec![(
                tool_gas_ref.clone(),
                sui::types::Owner::Shared(0),
                TypeName::new("ToolVertex"),
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
            PortsData::from_map(HashMap::from([(
                "entry_port".to_string(),
                NexusData::new_inline(json!("data")),
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
                outcome: MoveOption(Some(PostFailureAction::Terminate)),
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
            variant_ports_to_data: ports_data_map(vec![("answer", json!(42))]),
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
            outcome: MoveOption(Some(PostFailureAction::Terminate)),
            reason: MoveString::from("timeout"),
            err_eval_hash: vec![4, 5, 6],
            duplicate: true,
        });

        assert_eq!(event_execution_id(&event), Some(execution));
    }

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
                    outcome: MoveOption(Some(PostFailureAction::Terminate)),
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
                    variant_ports_to_data: ports_data_map(vec![("answer", json!(42))]),
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
                .as_json(),
            json!(42)
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

        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_ref,
            sui::types::Owner::Shared(0),
            json!({
                "invoker": "0x1",
                "dag": "0xd",
                "agent_id": "0xa",
                "skill_id": "11",
                "interface_version": "7",
                "scheduled_task_id": { "vec": [] },
                "scheduled_occurrence_index": { "vec": [] }
            }),
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
                id: crate::types::sui_address_to_uid(payment_id),
                execution_id,
                agent_id: crate::types::sui_address_to_id(sui::types::Address::from_static("0xa")),
                skill_id: 11,
                interface_revision: crate::types::interface::version::InterfaceVersion::new(7),
                payment_policy: crate::types::interface::payment::SkillPaymentPolicy::UserFunded,
                source_kind: crate::types::interface::payment::PaymentSourceKind::user_funded(
                    sui::types::Address::from_static("0x1"),
                ),
                max_budget: 100_000,
                locked_budget: 100_000,
                funds: crate::types::sui_framework::balance::Balance {
                    value: 58_000,
                    phantom_t0: std::marker::PhantomData,
                },
                consumed: 42_000,
                accomplished: true,
                refunded: false,
                final_state: ExecutionPaymentFinalState::Accomplished,
                tool_cost_snapshot: crate::types::sui_framework::vec_map::VecMap {
                    contents: vec![],
                },
                locked_vertices: vec![],
            })
            .expect("execution payment bcs"),
            sui::types::StructTag::new(
                nexus_objects.interface_pkg_id,
                crate::idents::tap::STANDARD_PAYMENT_MODULE,
                sui::types::Identifier::from_static("ExecutionPayment"),
                vec![],
            ),
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
        let vertices = DynamicMap::new(sui_mocks::mock_sui_address(), 0);
        let dag = Dag {
            vertices,
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig::default(),
            tool_verifier: VerifierConfig::default(),
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(0),
            json!({
                "invoker": "0x1",
                "dag": dag_ref.object_id().to_string(),
                "agent_id": "0xa",
                "skill_id": "11",
                "interface_version": "7",
                "scheduled_task_id": { "vec": [] },
                "scheduled_occurrence_index": { "vec": [] },
                "walks": []
            }),
        );
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            dag_ref,
            sui::types::Owner::Shared(0),
            serde_json::to_value(dag).expect("DAG JSON should serialize"),
        );
        sui_mocks::grpc::mock_list_dynamic_fields::<TypeName>(&mut state_service_mock, vec![]);
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs::<
            TypeName,
            LinkedTableNodeBcs<TypeName, DagVertexInfoBcs>,
        >(&mut ledger_service_mock, vec![]);
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                sui_framework::CLOCK_OBJECT_ID,
                1,
                sui::types::Digest::from([1; 32]),
            ),
            sui::types::Owner::Shared(1),
            json!({ "timestamp_ms": "61000" }),
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
                id: crate::types::sui_address_to_uid(*payment_ref.object_id()),
                execution_id: *execution_ref.object_id(),
                agent_id: crate::types::sui_address_to_id(sui::types::Address::from_static("0xa")),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(7),
                payment_policy: SkillPaymentPolicy::UserFunded,
                source_kind: crate::types::interface::payment::PaymentSourceKind::user_funded(
                    sui::types::Address::from_static("0x1"),
                ),
                max_budget: 100_000,
                locked_budget: 0,
                funds: crate::types::sui_framework::balance::Balance {
                    value: 100_000,
                    phantom_t0: std::marker::PhantomData,
                },
                consumed: 0,
                accomplished: false,
                refunded: false,
                final_state: ExecutionPaymentFinalState::Pending,
                tool_cost_snapshot: crate::types::sui_framework::vec_map::VecMap {
                    contents: vec![],
                },
                locked_vertices: vec![],
            })
            .expect("execution payment bcs"),
            sui::types::StructTag::new(
                nexus_objects.interface_pkg_id,
                crate::idents::tap::STANDARD_PAYMENT_MODULE,
                sui::types::Identifier::from_static("ExecutionPayment"),
                vec![],
            ),
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
        let tool_gas_id =
            derive_tool_gas_id(*nexus_objects.gas_service.object_id(), &tool_fqn).unwrap();
        let tool_gas_ref = sui_mocks::object_ref_for_id(tool_gas_id);
        let vertex = RuntimeVertex::plain("payable");
        let field_ref = sui_mocks::mock_sui_object_ref();
        let dag = Dag {
            vertices: DynamicMap::new(sui_mocks::mock_sui_address(), 1),
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig::default(),
            tool_verifier: VerifierConfig::default(),
        };
        let execution_json = json!({
            "invoker": "0x1",
            "dag": dag_ref.object_id().to_string(),
            "agent_id": "0xa",
            "skill_id": "11",
            "interface_version": "7",
            "scheduled_task_id": { "vec": [] },
            "scheduled_occurrence_index": { "vec": [] },
            "walks": [{
                "Active": {
                    "next_vertex": { "Plain": { "vertex": { "name": "payable" } } },
                    "timeout_ms": "30000",
                    "created_at": "1000"
                }
            }]
        });
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
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(0),
            execution_json.clone(),
        );
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            serde_json::to_value(dag).expect("DAG JSON should serialize"),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(TypeName::new("payable"), *field_ref.object_id())],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service_mock,
            vec![(
                field_ref,
                sui::types::Owner::Shared(1),
                TypeName::new("payable"),
                LinkedTableNodeBcs {
                    prev: crate::types::MoveOption(None::<TypeName>),
                    next: crate::types::MoveOption(None::<TypeName>),
                    value: DagVertexInfoBcs {
                        kind: crate::nexus::models::DagVertexKindBcs::OffChain {
                            _variant_name: "OffChain".to_string(),
                            tool_fqn: tool_fqn.to_string(),
                        },
                        input_ports: crate::types::MoveVecSet { contents: vec![] },
                        post_failure_action: crate::types::MoveOption(
                            None::<crate::nexus::models::PostFailureActionBcs>,
                        ),
                        leader_verifier: crate::types::MoveOption(
                            None::<crate::nexus::models::VerifierConfigBcs>,
                        ),
                        tool_verifier: crate::types::MoveOption(
                            None::<crate::nexus::models::VerifierConfigBcs>,
                        ),
                    },
                },
            )],
        );
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(
                sui_framework::CLOCK_OBJECT_ID,
                1,
                sui::types::Digest::from([1; 32]),
            ),
            sui::types::Owner::Shared(1),
            json!({ "timestamp_ms": "61000" }),
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
                id: crate::types::sui_address_to_uid(*payment_ref.object_id()),
                execution_id: *execution_ref.object_id(),
                agent_id: crate::types::sui_address_to_id(sui::types::Address::from_static("0xa")),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(7),
                payment_policy: SkillPaymentPolicy::UserFunded,
                source_kind: crate::types::interface::payment::PaymentSourceKind::user_funded(
                    sui::types::Address::from_static("0x1"),
                ),
                max_budget: 100_000,
                locked_budget: 10,
                funds: crate::types::sui_framework::balance::Balance {
                    value: 100_000,
                    phantom_t0: std::marker::PhantomData,
                },
                consumed: 0,
                accomplished: false,
                refunded: false,
                final_state: ExecutionPaymentFinalState::Pending,
                tool_cost_snapshot: crate::types::sui_framework::vec_map::VecMap {
                    contents: vec![],
                },
                locked_vertices: current_locked_vertices,
            })
            .expect("execution payment bcs"),
            sui::types::StructTag::new(
                nexus_objects.interface_pkg_id,
                crate::idents::tap::STANDARD_PAYMENT_MODULE,
                sui::types::Identifier::from_static("ExecutionPayment"),
                vec![],
            ),
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(tool_gas_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(0),
            execution_json,
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

    fn mock_execution_json(dag_ref: &sui::types::ObjectReference) -> serde_json::Value {
        json!({
            "invoker": "0x1",
            "dag": dag_ref.object_id().to_string(),
            "agent_id": "0xa",
            "skill_id": "11",
            "interface_version": "7",
            "scheduled_task_id": { "vec": [] },
            "scheduled_occurrence_index": { "vec": [] },
            "walks": []
        })
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
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            mock_execution_json(&dag_ref),
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
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            mock_execution_json(&dag_ref),
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
        sui_mocks::grpc::mock_get_object_json(
            &mut settle_ledger,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            mock_execution_json(&dag_ref),
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
        let dag = Dag {
            vertices: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig {
                mode: VerifierMode::LeaderRegisteredKey,
                method: "signed_http_v1".into(),
            },
            tool_verifier: VerifierConfig::default(),
        };
        let vertex = DagVertexInfo {
            kind: DagVertexKind::OffChain {
                tool_fqn: fqn!("xyz.example.tool@1"),
            },
            leader_verifier: Some(VerifierConfig::default()),
            tool_verifier: None,
            input_ports: Set::default(),
        };

        assert!(!dag_vertex_requires_verifier_proof(&dag, &vertex));
    }

    #[test]
    fn execution_terminal_record_matches_retryable_vertex_handles_wrapped_and_plain_json() {
        let vertex = RuntimeVertex::with_iterator("v1", 2, 5);
        let wrapped = json!({
            "fields": {
                "contents": [{
                    "fields": {
                        "key": "9",
                        "value": {
                            "fields": {
                                "record": {
                                    "fields": {
                                        "vertex": {
                                            "fields": {
                                                "_variant_name": "WithIterator",
                                                "vertex": { "name": "v1" },
                                                "iteration": { "value": "2" },
                                                "out_of": { "u64": 5 }
                                            }
                                        },
                                        "failure_class": {
                                            "fields": {
                                                "@variant": "Retryable"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }]
            }
        });
        let plain = json!({
            "9": {
                "vertex": {
                    "Plain": {
                        "vertex": {
                            "name": "terminal_vertex"
                        }
                    }
                },
                "failure_class": "TerminalSubmissionFailure"
            }
        });

        assert!(execution_terminal_record_matches_retryable_vertex(&wrapped, 9, &vertex).unwrap());
        assert!(!execution_terminal_record_matches_retryable_vertex(
            &plain,
            9,
            &RuntimeVertex::plain("terminal_vertex"),
        )
        .unwrap());
    }

    #[test]
    fn fetch_vertex_input_port_names_reads_declared_ports_from_typed_vertex() {
        let vertex = DagVertexInfo {
            kind: DagVertexKind::OffChain {
                tool_fqn: fqn!("xyz.example.tool@1"),
            },
            leader_verifier: None,
            tool_verifier: None,
            input_ports: [
                crate::nexus::models::DagInputPort {
                    name: "z_port".to_string(),
                },
                crate::nexus::models::DagInputPort {
                    name: "a_port".to_string(),
                },
            ]
            .into_iter()
            .collect(),
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
            TypeName::new("payable"),
            DagVertexInfo {
                kind: DagVertexKind::OffChain {
                    tool_fqn: tool_fqn.clone(),
                },
                leader_verifier: None,
                tool_verifier: None,
                input_ports: Set::default(),
            },
        );
        vertices.insert(
            TypeName::new("idle"),
            DagVertexInfo {
                kind: DagVertexKind::OffChain {
                    tool_fqn: other_tool_fqn,
                },
                leader_verifier: None,
                tool_verifier: None,
                input_ports: Set::default(),
            },
        );
        let walks = vec![
            DagExecutionWalk::Active {
                next_vertex: payable_vertex.clone(),
                timeout_ms: 30_000,
                requires_vertex_authorization_grant: false,
                created_at: 1_000,
            },
            DagExecutionWalk::Active {
                next_vertex: idle_vertex,
                timeout_ms: 30_000,
                requires_vertex_authorization_grant: false,
                created_at: 61_000,
            },
            DagExecutionWalk::PendingAbort {
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
            TypeName::new("payable"),
            DagVertexInfo {
                kind: DagVertexKind::OffChain {
                    tool_fqn: tool_fqn.clone(),
                },
                leader_verifier: None,
                tool_verifier: None,
                input_ports: Set::default(),
            },
        );
        let walks = vec![DagExecutionWalk::Active {
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
    fn tool_gas_abort_filter_ignores_pending_settlement_timeouts() {
        let execution_id = sui::types::Address::from_static("0xabc");
        let tool_fqn = fqn!("xyz.taluslabs.payable@1");
        let payable_vertex = RuntimeVertex::plain("payable");
        let matching_key = payment_vertex_key(execution_id, &payable_vertex, &tool_fqn).unwrap();
        let mut vertices = HashMap::new();
        vertices.insert(
            TypeName::new("payable"),
            DagVertexInfo {
                kind: DagVertexKind::OffChain {
                    tool_fqn: tool_fqn.clone(),
                },
                leader_verifier: None,
                tool_verifier: None,
                input_ports: Set::default(),
            },
        );
        let walks = vec![DagExecutionWalk::PendingSettlement {
            next_vertex: payable_vertex,
            timeout_ms: 30_000,
            requires_vertex_authorization_grant: false,
            created_at: 1_000,
        }];
        let locks = vec![ExecutionPaymentVertexLock {
            vertex_key: matching_key,
            tool_fqn: tool_fqn.to_string().into_bytes(),
            amount: 10,
            settlement_kind: VertexExecutionPaymentSettlementKind::Paid,
        }];

        let candidates =
            filter_tool_gas_abort_candidate_walks(execution_id, &vertices, &walks, &locks, 61_000)
                .unwrap();

        assert!(candidates.is_empty());
    }

    #[test]
    fn tool_gas_abort_filter_errors_when_expired_vertex_is_missing_from_dag() {
        let execution_id = sui::types::Address::from_static("0xabc");
        let walks = vec![DagExecutionWalk::Active {
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
        let tool_gas_id = derive_tool_gas_id(gas_service_id, &tool_fqn).unwrap();
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

    #[test]
    fn dag_edge_bcs_into_sdk_keeps_the_public_edge_shape() {
        let edge = DagEdgeBcs {
            from: DagOutputVariantPort {
                variant: crate::types::TypeName::from("ok"),
                port: crate::types::TypeName::from("value"),
            },
            to: DagVertexInputPort {
                vertex: crate::types::TypeName::from("next"),
                port: crate::nexus::models::DagInputPort {
                    name: "input".to_string(),
                },
            },
            kind: crate::nexus::models::DagEdgeKindBcs::Static,
        };

        let sdk = edge.into_sdk();

        assert_eq!(sdk.from.variant, crate::types::TypeName::from("ok"));
        assert_eq!(sdk.from.port, crate::types::TypeName::from("value"));
    }
}
