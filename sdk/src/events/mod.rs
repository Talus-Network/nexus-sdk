use {
    crate::{sui, types::*, ToolFqn},
    anyhow::{bail, Result},
    serde::{Deserialize, Serialize},
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
}

/// Fired via the Nexus `interface` package when a new Agent is registered.
/// Provides the agent's interface so that we can invoke
/// `confirm_tool_eval_for_walk` on it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnnounceInterfacePackageEvent {
    pub shared_objects: Vec<SharedObjectRef>,
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

/// Partial terminal `_err_eval` record shape often found inside nested trace
/// payloads before event metadata is attached.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TerminalErrEvalRecordedSummary {
    /// Which vertex produced the terminal `_err_eval`.
    pub vertex: RuntimeVertex,
    /// How the workflow classified the failure.
    pub failure_class: WorkflowFailureClass,
    /// Which post-failure action was resolved onchain.
    pub outcome: PostFailureAction,
    /// The sanitized terminal reason string recorded onchain.
    pub reason: String,
    /// Whether this event reflects a duplicate submission converging on the
    /// existing terminal record.
    pub duplicate: bool,
}

/// Comparison-oriented terminal `_err_eval` details used by higher-level
/// lineage reconstruction over nested event JSON payloads.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TerminalErrEvalEventDetails {
    pub execution: String,
    pub walk_index: u64,
    pub vertex: RuntimeVertex,
    pub leader: String,
    pub failure_class: WorkflowFailureClass,
    pub outcome: PostFailureAction,
    pub reason: String,
    pub err_eval_hash_hex: String,
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

/// Comparison-oriented verification-verdict details used by higher-level test
/// helpers over nested event JSON payloads.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct VerificationVerdictEventDetails {
    pub dag: String,
    pub execution: String,
    pub walk_index: u64,
    pub vertex: RuntimeVertex,
    pub leader: String,
    pub submission_kind: VerificationSubmissionKind,
    pub failure_evidence_kind: FailureEvidenceKind,
    pub leader_verifier_mode: VerifierMode,
    pub leader_verifier_method: String,
    pub tool_verifier_mode: VerifierMode,
    pub tool_verifier_method: String,
    pub checked_leader_kid: Option<u64>,
    pub checked_tool_kid: Option<u64>,
    pub payload_or_reason_hash_hex: String,
    pub submission_role: VerificationSubmissionRole,
    pub checked_identity_hex: String,
    pub policy_mode: VerifierMode,
    pub verdict_reference_hex: String,
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

/// Comparison-oriented submission-failure details used by higher-level
/// lineage reconstruction over nested event JSON payloads.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SubmissionFailureEvidenceEventDetails {
    pub execution: String,
    pub walk_index: u64,
    pub vertex: RuntimeVertex,
    pub failed_leader: String,
    pub winning_leader: Option<String>,
    pub reason: String,
    pub err_eval_hash_hex: String,
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
    pub cleared_occurrences: u64,
    pub had_periodic: bool,
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
    /// Address of the relevant slashing cap.
    pub slashing_cap: sui::types::Address,
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
}
