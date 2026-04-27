use {
    crate::{sui, types::*, ToolFqn},
    anyhow::{bail, Result},
    serde::{de::DeserializeOwned, Deserialize, Serialize},
};

mod parsing;
mod polling;

pub use {parsing::*, polling::*};

/// Distribution metadata for distributed events. This contains metadata about
/// the event deadline as well as the priority in which leaders should attempt
/// to execute the event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistributedEventMetadata {
    /// The execution window duration.
    #[serde(
        rename = "deadline_ms",
        deserialize_with = "deserialize_u64_to_duration",
        serialize_with = "serialize_duration_to_u64"
    )]
    pub deadline: chrono::Duration,
    /// The timestamp by which the event was requested.
    #[serde(
        rename = "requested_at_ms",
        deserialize_with = "deserialize_u64_to_datetime",
        serialize_with = "serialize_datetime_to_u64"
    )]
    pub requested_at: chrono::DateTime<chrono::Utc>,
    /// The priority list of leader addresses.
    pub leaders: Vec<sui::types::Address>,
    /// The task ID.
    pub task_id: sui::types::Address,
}

/// Struct holding the Sui event ID, the event generic arguments and the data
/// as one of [NexusEventKind].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NexusEvent {
    /// The event transaction digest and event sequence.
    pub id: (sui::types::Digest, u64),
    /// If the `T in NexusEvent<T>` is also a generic, this field holds the
    /// generic type. Note that this can be nested indefinitely.
    pub generics: Vec<sui::types::TypeTag>,
    /// The event data.
    pub data: NexusEventKind,
    /// If the event is a distributed event, this field holds the distribution
    /// metadata.
    pub distribution: Option<DistributedEventMetadata>,
}

macro_rules! events {
    (
        $(
            $struct_name:ident => $variant:ident, $name:expr
        ),* $(,)?
    ) => {

        // == enum NexusEventKind ==

        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(tag = "_nexus_event_type", content = "event")]
        pub enum NexusEventKind {
            $(
                #[serde(rename = $name)]
                $variant($struct_name),
            )*
        }

        impl NexusEventKind {
            /// Returns the name of the event kind as a string.
            pub fn name(&self) -> String {
                match self {
                    $(
                        NexusEventKind::$variant(_) => stringify!($struct_name).to_string(),
                    )*
                }
            }
        }

        // == Parsing from BCS ==

        pub(super) fn parse_bcs(name: &str, bytes: &[u8]) -> Result<(NexusEventKind, Option<DistributedEventMetadata>)> {
            #[derive(Deserialize)]
            struct Wrapper<T> {
                event: T,
            }

            #[derive(Deserialize)]
            struct DistributedWrapper<T> {
                event: T,
                deadline_ms: u64,
                requested_at_ms: u64,
                task_id: sui::types::Address,
                leaders: Vec<sui::types::Address>,
            }


            match name {
                $(
                    stringify!($struct_name) => {
                        match bcs::from_bytes::<DistributedWrapper<$struct_name>>(bytes) {
                            Ok(distributed) => {
                                let metadata = DistributedEventMetadata {
                                    deadline: chrono::Duration::milliseconds(distributed.deadline_ms as i64),
                                    requested_at: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(distributed.requested_at_ms as i64)
                                        .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
                                    task_id: distributed.task_id,
                                    leaders: distributed.leaders,
                                };

                                Ok((NexusEventKind::$variant(distributed.event), Some(metadata)))
                            }
                            Err(_) => {
                                 let obj: Wrapper<$struct_name> = bcs::from_bytes(bytes)?;

                                 Ok((NexusEventKind::$variant(obj.event), None))
                            }
                        }
                    }
                )*
                _ => bail!("Unknown event: {}", name),
            }
        }
    };
}

// Enumeration with all available events coming from the on-chain part of
// Nexus. Also includes BCS parsing implementations.
events! {
    RequestScheduledOccurrenceEvent => RequestScheduledOccurrence, "RequestScheduledOccurrenceEvent",
    RequestScheduledWalkEvent => RequestScheduledWalk, "RequestScheduledWalkEvent",
    OccurrenceScheduledEvent => OccurrenceScheduled, "OccurrenceScheduledEvent",
    RequestWalkExecutionEvent => RequestWalkExecution, "RequestWalkExecutionEvent",
    AnnounceInterfacePackageEvent => AnnounceInterfacePackage, "AnnounceInterfacePackageEvent",
    AgentCreatedEvent => AgentCreated, "AgentCreatedEvent",
    SkillRegisteredEvent => SkillRegistered, "SkillRegisteredEvent",
    EndpointRevisionAnnouncedEvent => EndpointRevisionAnnounced, "EndpointRevisionAnnouncedEvent",
    EndpointRevisionActivatedEvent => EndpointRevisionActivated, "EndpointRevisionActivatedEvent",
    WorksheetResolvedEvent => WorksheetResolved, "WorksheetResolvedEvent",
    AgentSkillExecutionRequestedEvent => AgentSkillExecutionRequested, "AgentSkillExecutionRequestedEvent",
    VertexAuthorizationCreatedEvent => VertexAuthorizationCreated, "VertexAuthorizationCreatedEvent",
    VertexAuthorizationBoundEvent => VertexAuthorizationBound, "VertexAuthorizationBoundEvent",
    VertexAuthorizationVerifiedEvent => VertexAuthorizationVerified, "VertexAuthorizationVerifiedEvent",
    VertexAuthorizationRevokedEvent => VertexAuthorizationRevoked, "VertexAuthorizationRevokedEvent",
    VertexAuthorizationExpiredEvent => VertexAuthorizationExpired, "VertexAuthorizationExpiredEvent",
    AgentSkillPaymentCreatedEvent => AgentSkillPaymentCreated, "AgentSkillPaymentCreatedEvent",
    GasPaymentConsumedEvent => GasPaymentConsumed, "GasPaymentConsumedEvent",
    ExecutionAccomplishedEvent => ExecutionAccomplished, "ExecutionAccomplishedEvent",
    ExecutionRefundedEvent => ExecutionRefunded, "ExecutionRefundedEvent",
    ScheduledSkillExecutionCreatedEvent => ScheduledSkillExecutionCreated, "ScheduledSkillExecutionCreatedEvent",
    ScheduledSkillExecutionTriggeredEvent => ScheduledSkillExecutionTriggered, "ScheduledSkillExecutionTriggeredEvent",
    ScheduledSkillExecutionCompletedEvent => ScheduledSkillExecutionCompleted, "ScheduledSkillExecutionCompletedEvent",
    ToolRegisteredEvent => ToolRegistered, "ToolRegisteredEvent",
    ToolUnregisteredEvent => ToolUnregistered, "ToolUnregisteredEvent",
    WalkAdvancedEvent => WalkAdvanced, "WalkAdvancedEvent",
    WalkFailedEvent => WalkFailed, "WalkFailedEvent",
    TerminalErrEvalRecordedEvent => TerminalErrEvalRecorded, "TerminalErrEvalRecordedEvent",
    VerificationVerdictEvent => VerificationVerdictRecorded, "VerificationVerdictEvent",
    WalkAbortedEvent => WalkAborted, "WalkAbortedEvent",
    WalkCancelledEvent => WalkCancelled, "WalkCancelledEvent",
    EndStateReachedEvent => EndStateReached, "EndStateReachedEvent",
    ExecutionFinishedEvent => ExecutionFinished, "ExecutionFinishedEvent",
    MissedOccurrenceEvent => MissedOccurrence, "MissedOccurrenceEvent",
    TaskCreatedEvent => TaskCreated, "TaskCreatedEvent",
    TaskPausedEvent => TaskPaused, "TaskPausedEvent",
    TaskResumedEvent => TaskResumed, "TaskResumedEvent",
    TaskCanceledEvent => TaskCanceled, "TaskCanceledEvent",
    OccurrenceConsumedEvent => OccurrenceConsumed, "OccurrenceConsumedEvent",
    PeriodicScheduleConfiguredEvent => PeriodicScheduleConfigured, "PeriodicScheduleConfiguredEvent",
    FoundingLeaderCapCreatedEvent => FoundingLeaderCapCreated, "FoundingLeaderCapCreatedEvent",
    LeaderCapIssuedEvent => LeaderCapIssued, "LeaderCapIssuedEvent",
    GasLockUpdateEvent => GasLockUpdate, "GasLockUpdateEvent",
    DAGCreatedEvent => DAGCreated, "DAGCreatedEvent",
    ToolRegistryCreatedEvent => ToolRegistryCreated, "ToolRegistryCreatedEvent",
}

fn deserialize_move_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    MoveOption::<T>::deserialize(deserializer).map(|value| value.0)
}

// == Event definitions ==

/// Fired by the on-chain part of Nexus when a DAG vertex execution is
/// requested.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestWalkExecutionEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub invoker: sui::types::Address,
    pub walk_index: u64,
    pub next_vertex: RuntimeVertex,
    pub evaluations: sui::types::Address,
    /// This field defines the package ID, module and name of the Agent that
    /// holds the DAG. Used to confirm the tool evaluation with the Agent.
    pub worksheet_from_type: TypeName,
    /// UID of the TAP witness object that created the worksheet used to start
    /// this execution.
    pub worksheet_from_uid: sui::types::Address,
    /// Standard TAP agent identity. Absent for legacy witness executions.
    #[serde(
        default,
        deserialize_with = "deserialize_move_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub tap_agent_id: Option<AgentId>,
    /// Standard TAP skill identity. Absent for legacy witness executions.
    #[serde(
        default,
        deserialize_with = "deserialize_move_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub tap_skill_id: Option<SkillId>,
    /// Standard TAP endpoint revision pinned for this execution.
    #[serde(
        default,
        deserialize_with = "deserialize_move_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub tap_interface_revision: Option<InterfaceRevision>,
    /// Standard TAP endpoint object pinned for this execution.
    #[serde(
        default,
        deserialize_with = "deserialize_move_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub tap_endpoint_object_id: Option<sui::types::Address>,
    /// Standard TAP payment object bound to this execution.
    #[serde(
        default,
        deserialize_with = "deserialize_move_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub tap_payment_id: Option<sui::types::Address>,
    /// Optional standard TAP authorization plan hash.
    #[serde(
        default,
        deserialize_with = "deserialize_move_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub tap_authorization_plan_hash: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestWalkStandardTapContext {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub endpoint_object_id: sui::types::Address,
    pub payment_id: sui::types::Address,
    pub authorization_plan_hash: Option<Vec<u8>>,
}

impl RequestWalkExecutionEvent {
    pub fn standard_tap_context(&self) -> Result<Option<RequestWalkStandardTapContext>> {
        if self.tap_agent_id.is_none()
            && self.tap_skill_id.is_none()
            && self.tap_interface_revision.is_none()
            && self.tap_endpoint_object_id.is_none()
            && self.tap_payment_id.is_none()
            && self.tap_authorization_plan_hash.is_none()
        {
            return Ok(None);
        }

        let Some(agent_id) = self.tap_agent_id else {
            bail!(
                "RequestWalkExecutionEvent has partial standard TAP context: missing tap_agent_id"
            );
        };
        let Some(skill_id) = self.tap_skill_id else {
            bail!(
                "RequestWalkExecutionEvent has partial standard TAP context: missing tap_skill_id"
            );
        };
        let Some(interface_revision) = self.tap_interface_revision else {
            bail!(
                "RequestWalkExecutionEvent has partial standard TAP context: missing tap_interface_revision"
            );
        };
        let Some(endpoint_object_id) = self.tap_endpoint_object_id else {
            bail!(
                "RequestWalkExecutionEvent has partial standard TAP context: missing tap_endpoint_object_id"
            );
        };
        let Some(payment_id) = self.tap_payment_id else {
            bail!("RequestWalkExecutionEvent has partial standard TAP context: missing tap_payment_id");
        };

        Ok(Some(RequestWalkStandardTapContext {
            agent_id,
            skill_id,
            interface_revision,
            endpoint_object_id,
            payment_id,
            authorization_plan_hash: self.tap_authorization_plan_hash.clone(),
        }))
    }
}

/// Fired via the Nexus `interface` package when a new Agent is registered.
/// Provides the agent's interface so that we can invoke
/// `confirm_tool_eval_for_walk` on it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnnounceInterfacePackageEvent {
    pub shared_objects: Vec<SharedObjectRef>,
}

/// Fired when a TAP agent is created and receives an on-chain identity handle.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentCreatedEvent {
    pub agent_id: AgentId,
    pub owner: sui::types::Address,
    pub operator: sui::types::Address,
    pub metadata_hash: Vec<u8>,
    pub auth_mode: u8,
}

/// Fired when a published DAG/TAP package is registered as a skill.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillRegisteredEvent {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub dag_id: sui::types::Address,
    pub tap_package_id: sui::types::Address,
    pub workflow_hash: Vec<u8>,
    pub requirements_hash: Vec<u8>,
    pub payment_policy_hash: Vec<u8>,
    pub schedule_policy_hash: Vec<u8>,
    pub capability_schema_hash: Vec<u8>,
}

/// Fired when an endpoint revision is announced for a registered skill.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EndpointRevisionAnnouncedEvent {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub package_id: sui::types::Address,
    pub endpoint_object_id: sui::types::Address,
    pub endpoint_object_version: u64,
    pub endpoint_object_digest: Vec<u8>,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub requirements: TapSkillRequirements,
    pub config_digest: Vec<u8>,
    pub active_for_new_executions: bool,
}

/// Fired when active revision state changes for a skill.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EndpointRevisionActivatedEvent {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub active_for_new_executions: bool,
}

/// Fired when worksheet routing resolves a pinned endpoint.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorksheetResolvedEvent {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub endpoint_object_id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub worksheet_id: sui::types::Address,
}

/// Fired when immediate execution is requested for an agent skill.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentSkillExecutionRequestedEvent {
    pub execution_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub payment_id: sui::types::Address,
    pub authorization_plan_hash: Option<Vec<u8>>,
}

/// Fired when vertex authorization intent is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VertexAuthorizationCreatedEvent {
    pub grant_id: sui::types::Address,
    pub grantor: sui::types::Address,
    pub target_object_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub walk_execution_id: sui::types::Address,
    pub vertex_execution_id: sui::types::Address,
    pub expires_at_ms: u64,
    pub max_uses: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VertexAuthorizationBoundEvent {
    pub grant_id: sui::types::Address,
    pub walk_execution_id: sui::types::Address,
    pub vertex_execution_id: sui::types::Address,
    pub leader_assignment_id: sui::types::Address,
    pub interface_revision: InterfaceRevision,
    pub payment_id: Option<sui::types::Address>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VertexAuthorizationVerifiedEvent {
    pub grant_id: sui::types::Address,
    pub walk_execution_id: sui::types::Address,
    pub vertex_execution_id: sui::types::Address,
    pub leader_assignment_id: sui::types::Address,
    pub tool_package: sui::types::Address,
    pub operation_hash: Vec<u8>,
    pub used: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VertexAuthorizationRevokedEvent {
    pub grant_id: sui::types::Address,
    pub grantor: sui::types::Address,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VertexAuthorizationExpiredEvent {
    pub grant_id: sui::types::Address,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentSkillPaymentCreatedEvent {
    pub payment_id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub payer: sui::types::Address,
    pub max_budget: u64,
    pub auth_mode: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GasPaymentConsumedEvent {
    pub payment_id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub amount: u64,
    pub consumed_total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionAccomplishedEvent {
    pub execution_id: sui::types::Address,
    pub payment_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub result_summary_hash: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionRefundedEvent {
    pub execution_id: sui::types::Address,
    pub payment_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub refund_reason: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScheduledSkillExecutionCreatedEvent {
    pub scheduled_task_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub long_term_gas_coin_id: sui::types::Address,
    pub schedule_entries_hash: Vec<u8>,
    pub first_after_ms: u64,
    pub max_occurrences: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScheduledSkillExecutionTriggeredEvent {
    pub scheduled_task_id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub occurrence_index: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScheduledSkillExecutionCompletedEvent {
    pub scheduled_task_id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub continue_recurring: bool,
    pub next_after_ms: u64,
}

/// Fired by the Nexus Workflow when a new tool is registered so that the Leader
/// can also register it in Redis. This way the Leader knows how and where to
/// evaluate the tool.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolRegisteredEvent {
    pub tool: sui::types::Address,
    /// The tool domain, name and version. See [ToolFqn] for more information.
    pub fqn: ToolFqn,
}

/// Fired by the Nexus Workflow when a tool is unregistered. The Leader should
/// remove the tool definition from its Redis registry.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolUnregisteredEvent {
    pub tool: sui::types::Address,
    /// The tool domain, name and version. See [ToolFqn] for more information.
    pub fqn: ToolFqn,
}

/// Fired by the Nexus Workflow when a walk has advanced. This event is used to
/// inspect DAG execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkAdvancedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub walk_index: u64,
    /// Which vertex was just executed.
    pub vertex: RuntimeVertex,
    /// Which output variant was evaluated.
    pub variant: TypeName,
    /// What data is associated with the variant.
    pub variant_ports_to_data: PortsData,
}

/// Fired by the Nexus Workflow when a walk has failed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkFailedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub walk_index: u64,
    /// Which vertex was being executed when the failure happened.
    pub vertex: RuntimeVertex,
    /// The error message associated with the failure.
    pub reason: String,
}

/// Fired when the authoritative per-walk terminal `_err_eval` record is
/// created or updated.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TerminalErrEvalRecordedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub walk_index: u64,
    /// Which vertex produced the terminal `_err_eval`.
    pub vertex: RuntimeVertex,
    /// Which leader submitted the terminal record.
    pub leader: sui::types::Address,
    /// How the workflow classified the failure.
    pub failure_class: WorkflowFailureClass,
    /// Which post-failure action was resolved onchain.
    pub outcome: PostFailureAction,
    /// The sanitized terminal reason string recorded onchain.
    pub reason: String,
    /// Hash of the `_err_eval` payload associated with this record.
    pub err_eval_hash: Vec<u8>,
    /// Whether this event reflects a duplicate submission converging on the
    /// existing terminal record.
    pub duplicate: bool,
}

/// Verifier-aware submission verdict emitted by the workflow.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct VerificationVerdictEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub walk_index: u64,
    pub vertex: RuntimeVertex,
    pub leader: sui::types::Address,
    pub submission_kind: VerificationSubmissionKind,
    pub failure_evidence_kind: FailureEvidenceKind,
    pub leader_verifier_mode: VerifierMode,
    pub leader_verifier_method: String,
    pub tool_verifier_mode: VerifierMode,
    pub tool_verifier_method: String,
    pub checked_leader_kid: Option<u64>,
    pub checked_tool_kid: Option<u64>,
    pub payload_or_reason_hash: Vec<u8>,
    pub submission_role: VerificationSubmissionRole,
    pub checked_identity: Vec<u8>,
    pub policy_mode: VerifierMode,
    pub verdict_reference: Vec<u8>,
    pub verdict: VerificationVerdict,
}

/// Submission-failure evidence payload recorded for terminal submission
/// failures.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SubmissionFailureEvidenceRecordedEvent {
    pub execution: sui::types::Address,
    pub walk_index: u64,
    pub vertex: RuntimeVertex,
    pub failed_leader: sui::types::Address,
    pub winning_leader: Option<sui::types::Address>,
    pub reason: String,
    pub err_eval_hash: Vec<u8>,
}

/// Fired by the Nexus Workflow when a walk was aborted.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkAbortedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub walk_index: u64,
    /// Which vertex was being executed when the abort happened.
    pub vertex: RuntimeVertex,
}

/// Fired by the Nexus Workflow when a walk was cancelled.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkCancelledEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub walk_index: u64,
    /// Which vertex was being executed when the cancellation happened.
    pub vertex: RuntimeVertex,
}

/// Fired by the Nexus Workflow when a walk has halted in an end state. This
/// event is used to inspect DAG execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EndStateReachedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub walk_index: u64,
    /// Which vertex was just executed.
    pub vertex: RuntimeVertex,
    /// Which output variant was evaluated.
    pub variant: TypeName,
    /// What data is associated with the variant.
    pub variant_ports_to_data: PortsData,
}

/// Fired by the Nexus Workflow when all walks have halted in their end states
/// and there is no more work to be done. This event is used to inspect DAG
/// execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionFinishedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub has_any_walk_failed: bool,
    pub has_any_walk_succeeded: bool,
    pub was_aborted: bool,
}

/// Request wrapper emitted when scheduling an occurrence.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestScheduledOccurrenceEvent {
    pub request: OccurrenceScheduledEvent,
    pub priority: u64,
    pub request_ms: u64,
    pub start_ms: u64,
    pub deadline_ms: u64,
}

/// Request wrapper emitted when scheduling a walk execution.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestScheduledWalkEvent {
    pub request: RequestWalkExecutionEvent,
    pub priority: u64,
    pub request_ms: u64,
    pub start_ms: u64,
    pub deadline_ms: u64,
}

/// Fired when a scheduler occurrence is enqueued; used as the payload of
/// `RequestScheduledOccurrenceEvent`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OccurrenceScheduledEvent {
    pub task: sui::types::Address,
    pub generator: PolicySymbol,
}

/// Emitted when a scheduled occurrence misses its deadline and is pruned.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MissedOccurrenceEvent {
    pub task: sui::types::Address,
    pub start_time_ms: u64,
    pub deadline_ms: Option<u64>,
    pub pruned_at: u64,
    pub priority_fee_per_gas_unit: u64,
    pub generator: PolicySymbol,
}

/// Emitted after a scheduler task object is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCreatedEvent {
    pub task: sui::types::Address,
    pub owner: sui::types::Address,
}

/// Emitted when scheduling for a task is paused.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskPausedEvent {
    pub task: sui::types::Address,
}

/// Emitted when scheduling for a task is resumed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskResumedEvent {
    pub task: sui::types::Address,
}

/// Emitted when scheduling for a task is canceled.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCanceledEvent {
    pub task: sui::types::Address,
}

/// Emitted whenever a pending occurrence is consumed for execution.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OccurrenceConsumedEvent {
    pub task: sui::types::Address,
    pub start_time_ms: u64,
    pub deadline_ms: Option<u64>,
    pub priority_fee_per_gas_unit: u64,
    pub generator: PolicySymbol,
    pub executed_at: u64,
}

/// Emitted whenever the periodic schedule is configured or cleared.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PeriodicScheduleConfiguredEvent {
    pub task: sui::types::Address,
    pub period_ms: Option<u64>,
    pub deadline_offset_ms: Option<u64>,
    pub max_iterations: Option<u64>,
    pub generated: Option<u64>,
    pub priority_fee_per_gas_unit: Option<u64>,
    pub last_generated_start_ms: Option<u64>,
}

/// Fired by the Nexus Workflow when a new founding LeaderCap is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundingLeaderCapCreatedEvent {
    pub leader_cap: sui::types::Address,
    pub network: sui::types::Address,
}

/// Fired by the Nexus Workflow when a leader capability is issued and transferred.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeaderCapIssuedEvent {
    pub registry: sui::types::Address,
    pub leader_cap_id: sui::types::Address,
    pub network: sui::types::Address,
    pub leader: sui::types::Address,
}

/// Fired by the Gas service when the gas settlement is updated. This event is
/// used to determine whether a tool invocation was paid for by the caller.
/// Combination of `execution` and `vertex` uniquely identifies the tool
/// invocation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GasLockUpdateEvent {
    pub execution: sui::types::Address,
    pub vertex: RuntimeVertex,
    pub tool_fqn: ToolFqn,
    pub was_locked: bool,
}

/// Fired when the leader claims gas from a user's budget.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeaderClaimedGasEvent {
    pub network: sui::types::Address,
    pub amount: u64,
    pub purpose: String,
}

/// Fired by the Nexus Workflow when a new DAG is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DAGCreatedEvent {
    /// Address of the created DAG.
    pub dag: sui::types::Address,
}

/// Fired by the Nexus Workflow when a new ToolRegistry is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolRegistryCreatedEvent {
    /// Address of the created ToolRegistry.
    pub registry: sui::types::Address,
}

#[cfg(test)]
mod tests {
    use super::*;

    events!(
        DummyEvent => Dummy, "DummyEvent",
        AnotherEvent => Another, "AnotherEvent",
    );

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct DummyEvent {
        pub value: u32,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct AnotherEvent {
        pub text: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct Wrapper<T> {
        event: T,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct DistributedWrapper<T> {
        event: T,
        deadline_ms: u64,
        requested_at_ms: u64,
        task_id: sui::types::Address,
        leaders: Vec<sui::types::Address>,
    }

    #[test]
    fn test_nexus_event_kind_name_helper() {
        let dummy = DummyEvent { value: 42 };
        let another = AnotherEvent {
            text: "hello".to_string(),
        };

        let kind_dummy = NexusEventKind::Dummy(dummy.clone());
        let kind_another = NexusEventKind::Another(another.clone());

        assert_eq!(kind_dummy.name(), "DummyEvent");
        assert_eq!(kind_another.name(), "AnotherEvent");
    }

    #[test]
    fn test_nexus_event_kind_enum_generation() {
        let dummy = DummyEvent { value: 1 };
        let another = AnotherEvent {
            text: "abc".to_string(),
        };

        let kind_dummy = NexusEventKind::Dummy(dummy.clone());
        let kind_another = NexusEventKind::Another(another.clone());

        match kind_dummy {
            NexusEventKind::Dummy(ev) => assert_eq!(ev, dummy),
            _ => panic!("Expected Dummy variant"),
        }

        match kind_another {
            NexusEventKind::Another(ev) => assert_eq!(ev, another),
            _ => panic!("Expected Another variant"),
        }
    }

    #[test]
    fn test_nexus_event_kind_bcs_deser() {
        let dummy = Wrapper {
            event: DummyEvent { value: 99 },
        };
        let another = Wrapper {
            event: AnotherEvent {
                text: "xyz".to_string(),
            },
        };

        let dummy_bytes = bcs::to_bytes(&dummy).unwrap();
        let another_bytes = bcs::to_bytes(&another).unwrap();

        let (parsed_dummy, dis1) = parse_bcs("DummyEvent", &dummy_bytes).unwrap();
        let (parsed_another, dis2) = parse_bcs("AnotherEvent", &another_bytes).unwrap();

        assert!(dis1.is_none());
        assert!(dis2.is_none());

        match parsed_dummy {
            NexusEventKind::Dummy(ev) => assert_eq!(ev, dummy.event),
            _ => panic!("Expected Dummy variant"),
        }

        match parsed_another {
            NexusEventKind::Another(ev) => assert_eq!(ev, another.event),
            _ => panic!("Expected Another variant"),
        }
    }

    #[test]
    fn test_distributed_nexus_event_kind_bcs_deser() {
        let dummy = DistributedWrapper {
            event: DummyEvent { value: 99 },
            deadline_ms: 0,
            requested_at_ms: 0,
            task_id: sui::types::Address::TWO,
            leaders: vec![sui::types::Address::TWO],
        };
        let another = DistributedWrapper {
            event: AnotherEvent {
                text: "xyz".to_string(),
            },
            deadline_ms: 100,
            requested_at_ms: 1500,
            task_id: sui::types::Address::ZERO,
            leaders: vec![sui::types::Address::ZERO],
        };

        let dummy_bytes = bcs::to_bytes(&dummy).unwrap();
        let another_bytes = bcs::to_bytes(&another).unwrap();

        let (parsed_dummy, dis1) = parse_bcs("DummyEvent", &dummy_bytes).unwrap();
        let (parsed_another, dis2) = parse_bcs("AnotherEvent", &another_bytes).unwrap();

        let dis1 = dis1.expect("Expected distribution metadata for dummy event");
        let dis2 = dis2.expect("Expected distribution metadata for another event");

        assert_eq!(dis1.leaders, vec![sui::types::Address::TWO]);
        assert_eq!(dis1.task_id, sui::types::Address::TWO);
        assert_eq!(dis1.deadline, chrono::Duration::milliseconds(0));
        assert_eq!(
            dis1.requested_at,
            chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap()
        );

        assert_eq!(dis2.leaders, vec![sui::types::Address::ZERO]);
        assert_eq!(dis2.task_id, sui::types::Address::ZERO);
        assert_eq!(dis2.deadline, chrono::Duration::milliseconds(100));
        assert_eq!(
            dis2.requested_at,
            chrono::DateTime::<chrono::Utc>::from_timestamp(1, 500_000_000).unwrap()
        );

        match parsed_dummy {
            NexusEventKind::Dummy(ev) => assert_eq!(ev, dummy.event),
            _ => panic!("Expected Dummy variant"),
        }

        match parsed_another {
            NexusEventKind::Another(ev) => assert_eq!(ev, another.event),
            _ => panic!("Expected Another variant"),
        }
    }

    #[test]
    fn test_parse_bcs_unknown_event() {
        let dummy = Wrapper {
            event: DummyEvent { value: 123 },
        };
        let bytes = bcs::to_bytes(&dummy).unwrap();
        let result = parse_bcs("UnknownEvent", &bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_bcs_terminal_err_eval_recorded_event() {
        let event = Wrapper {
            event: TerminalErrEvalRecordedEvent {
                dag: sui::types::Address::ZERO,
                execution: sui::types::Address::TWO,
                walk_index: 7,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalToolFailure,
                outcome: PostFailureAction::TransientContinue,
                reason: "terminal _err_eval".to_string(),
                err_eval_hash: vec![1, 2, 3, 4],
                duplicate: true,
            },
        };

        let bytes = bcs::to_bytes(&event).unwrap();
        let (parsed, distribution) =
            super::parse_bcs("TerminalErrEvalRecordedEvent", &bytes).unwrap();

        assert!(distribution.is_none());
        match parsed {
            crate::events::NexusEventKind::TerminalErrEvalRecorded(parsed) => {
                assert_eq!(parsed.walk_index, 7);
                assert_eq!(
                    parsed.failure_class,
                    WorkflowFailureClass::TerminalToolFailure
                );
                assert_eq!(parsed.outcome, PostFailureAction::TransientContinue);
                assert_eq!(parsed.reason, "terminal _err_eval");
                assert_eq!(parsed.err_eval_hash, vec![1, 2, 3, 4]);
                assert!(parsed.duplicate);
            }
            _ => panic!("Expected TerminalErrEvalRecorded variant"),
        }
    }

    #[test]
    fn test_parse_bcs_verification_verdict_event() {
        let event = Wrapper {
            event: VerificationVerdictEvent {
                dag: sui::types::Address::from_static("0x1"),
                execution: sui::types::Address::TWO,
                walk_index: 7,
                vertex: RuntimeVertex::plain("verified"),
                leader: sui::types::Address::THREE,
                submission_kind: VerificationSubmissionKind::Success,
                failure_evidence_kind: FailureEvidenceKind::ToolEvidence,
                leader_verifier_mode: VerifierMode::LeaderRegisteredKey,
                leader_verifier_method: "signed_http_v1".to_string(),
                tool_verifier_mode: VerifierMode::None,
                tool_verifier_method: String::new(),
                checked_leader_kid: Some(11),
                checked_tool_kid: Some(12),
                payload_or_reason_hash: vec![1, 2, 3, 4],
                submission_role: VerificationSubmissionRole::Tool,
                checked_identity: vec![5, 6, 7],
                policy_mode: VerifierMode::LeaderRegisteredKey,
                verdict_reference: vec![8, 9],
                verdict: VerificationVerdict::Accepted,
            },
        };

        let bytes = bcs::to_bytes(&event).unwrap();
        let (parsed, distribution) = super::parse_bcs("VerificationVerdictEvent", &bytes).unwrap();

        assert!(distribution.is_none());
        match parsed {
            crate::events::NexusEventKind::VerificationVerdictRecorded(parsed) => {
                assert_eq!(parsed.walk_index, 7);
                assert_eq!(parsed.vertex, RuntimeVertex::plain("verified"));
                assert_eq!(parsed.checked_leader_kid, Some(11));
                assert_eq!(parsed.checked_tool_kid, Some(12));
                assert_eq!(parsed.policy_mode, VerifierMode::LeaderRegisteredKey);
                assert_eq!(parsed.verdict, VerificationVerdict::Accepted);
            }
            _ => panic!("Expected VerificationVerdictRecorded variant"),
        }
    }

    #[test]
    fn test_parse_bcs_standard_tap_events() {
        let event = Wrapper {
            event: EndpointRevisionAnnouncedEvent {
                agent_id: AgentId(sui::types::Address::from_static("0xa")),
                skill_id: SkillId(sui::types::Address::from_static("0xb")),
                interface_revision: InterfaceRevision(3),
                package_id: sui::types::Address::from_static("0xc"),
                endpoint_object_id: sui::types::Address::from_static("0xd"),
                endpoint_object_version: 7,
                endpoint_object_digest: vec![4; 32],
                shared_objects: vec![TapSharedObjectRef::mutable(
                    sui::types::Address::from_static("0xe"),
                    9,
                )],
                requirements: TapSkillRequirements::default(),
                config_digest: vec![1, 2, 3],
                active_for_new_executions: true,
            },
        };

        let bytes = bcs::to_bytes(&event).unwrap();
        let (parsed, distribution) =
            super::parse_bcs("EndpointRevisionAnnouncedEvent", &bytes).unwrap();

        assert!(distribution.is_none());
        match parsed {
            crate::events::NexusEventKind::EndpointRevisionAnnounced(parsed) => {
                assert_eq!(parsed.agent_id, event.event.agent_id);
                assert_eq!(parsed.skill_id, event.event.skill_id);
                assert_eq!(parsed.interface_revision, InterfaceRevision(3));
                assert_eq!(parsed.shared_objects[0].initial_shared_version, 9);
                assert!(parsed.shared_objects[0].mutable);
                assert!(parsed.active_for_new_executions);
            }
            _ => panic!("Expected EndpointRevisionAnnounced variant"),
        }
    }
}
