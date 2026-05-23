//! This module defines transformers from various Sui types to Nexus event types.
//! Namely we support:
//! - Parsing GRPC [`sui::types::Event`]

use {
    crate::{
        events::{parse_bcs, NexusEvent},
        idents::primitives,
        sui,
        types::{
            normalize_json_string,
            parse_address_value,
            parse_bool_value,
            parse_byte_vector_value,
            parse_optional_string_value,
            parse_published_move_enum_value,
            parse_runtime_vertex_value,
            parse_string_value,
            parse_u64_value,
            MoveOption,
            NexusObjects,
        },
    },
    anyhow::bail,
};

/// [`sui::types::Event`] -> [`NexusEvent`]
pub trait FromSuiGrpcEvent {
    /// Parse a Sui GRPC event into a Nexus event.
    fn from_sui_grpc_event(
        index: u64,
        digest: sui::types::Digest,
        event: &sui::types::Event,
        objects: &NexusObjects,
    ) -> anyhow::Result<NexusEvent>;
}

impl FromSuiGrpcEvent for NexusEvent {
    fn from_sui_grpc_event(
        index: u64,
        digest: sui::types::Digest,
        event: &sui::types::Event,
        objects: &NexusObjects,
    ) -> anyhow::Result<NexusEvent> {
        // Only accept events that are wrapped in `nexus_primitives::event::EventWrapper`.
        if !is_event_wrapper(&event.type_, objects) {
            bail!(
                "Event is not wrapped in '{}::event::EventWrapper', found type: '{:?}'",
                objects.primitives_pkg_id,
                event.type_
            );
        }

        // Extract the name of the event we want to parse into.
        let Some(event_type) = event.type_.type_params().first().and_then(|tag| match tag {
            sui::types::TypeTag::Struct(struct_tag) => Some(struct_tag),
            _ => None,
        }) else {
            bail!("EventWrapper does not have a valid event type parameter");
        };

        let event_name = normalize_event_name(event_type)?;

        // Only accept inner events that come from Nexus packages.
        if !is_nexus_package(*event_type.address(), objects) {
            bail!(
                "Inner event does not come from a Nexus package, it comes from '{}' instead",
                event_type.address()
            );
        }

        // Public Nexus entrypoints can be invoked from extension packages, in
        // which case Sui records the caller package as `event.package_id`.
        // The wrapper package and inner Nexus event type checks above remain
        // the trust boundary for those event names.
        if !is_nexus_package(event.package_id, objects) && !allows_foreign_emitter(&event_name) {
            bail!(
                "Event does not come from a Nexus package, it comes from '{}' instead",
                event.package_id
            );
        }

        let (data, distribution) = parse_bcs(&event_name, &event.contents)?;

        Ok(NexusEvent {
            id: (digest, index),
            generics: event_type.type_params().to_vec(),
            data,
            distribution,
        })
    }
}

/// Parse a nested Move-JSON payload into a full terminal `_err_eval` event.
pub fn parse_terminal_err_eval_recorded_event_value(
    value: serde_json::Value,
) -> anyhow::Result<Option<crate::events::TerminalErrEvalRecordedEvent>> {
    parse_nested_event_value(value, try_parse_terminal_err_eval_recorded_event)
}

/// Parse a nested Move-JSON payload into submission-failure evidence.
pub fn parse_submission_failure_evidence_recorded_event(
    value: serde_json::Value,
) -> anyhow::Result<Option<crate::events::SubmissionFailureEvidenceRecordedEvent>> {
    parse_nested_event_value(value, try_parse_submission_failure_evidence_recorded_event)
}

/// Parse a nested Move-JSON payload into a verification-verdict event.
pub fn parse_verification_verdict_event(
    value: serde_json::Value,
) -> anyhow::Result<Option<crate::events::VerificationVerdictEvent>> {
    parse_nested_event_value(value, try_parse_verification_verdict_event)
}

fn normalize_event_name(event_type: &sui::types::StructTag) -> anyhow::Result<String> {
    let name = event_type.name().as_str();

    if name != "RequestScheduledExecution" {
        return Ok(name.to_string());
    }

    let Some(type_tag) = event_type.type_params().first() else {
        bail!("RequestScheduledExecution is missing a type parameter");
    };

    let sui::types::TypeTag::Struct(struct_tag) = type_tag else {
        bail!("RequestScheduledExecution expects a struct type parameter");
    };

    let normalized = match struct_tag.name().as_str() {
        "OccurrenceScheduledEvent" => "RequestScheduledOccurrenceEvent",
        "RequestWalkExecutionEvent" => "RequestScheduledWalkEvent",
        other => bail!("Unsupported RequestScheduledExecution payload: {other}"),
    };

    Ok(normalized.to_string())
}

pub(crate) fn parse_nested_event_value<T>(
    value: serde_json::Value,
    try_parse: fn(&serde_json::Value) -> anyhow::Result<Option<T>>,
) -> anyhow::Result<Option<T>> {
    const KEYS: [&str; 7] = [
        "event",
        "parsedJson",
        "parsed_json",
        "fields",
        "value",
        "data",
        "contents",
    ];
    if let Some(parsed) = try_parse(&value)? {
        return Ok(Some(parsed));
    }

    match value {
        serde_json::Value::Object(object) => {
            for key in KEYS {
                if let Some(nested) = object.get(key) {
                    if let Some(parsed) = parse_nested_event_value(nested.clone(), try_parse)? {
                        return Ok(Some(parsed));
                    }
                }
            }

            for (key, nested) in object.into_iter() {
                if !KEYS.contains(&key.as_str()) {
                    if let Some(parsed) = parse_nested_event_value(nested, try_parse)? {
                        return Ok(Some(parsed));
                    }
                }
            }
        }
        serde_json::Value::Array(values) => {
            for nested in values {
                if let Some(parsed) = parse_nested_event_value(nested, try_parse)? {
                    return Ok(Some(parsed));
                }
            }
        }
        _ => {}
    }

    Ok(None)
}

fn try_parse_terminal_err_eval_recorded_event(
    value: &serde_json::Value,
) -> anyhow::Result<Option<crate::events::TerminalErrEvalRecordedEvent>> {
    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    let Some(dag) = object.get("dag") else {
        return Ok(None);
    };
    let Some(execution) = object.get("execution") else {
        return Ok(None);
    };
    let Some(walk_index) = object.get("walk_index") else {
        return Ok(None);
    };
    let Some(vertex) = object.get("vertex") else {
        return Ok(None);
    };
    let Some(leader) = object.get("leader") else {
        return Ok(None);
    };
    let Some(failure_class) = object.get("failure_class") else {
        return Ok(None);
    };
    let Some(outcome) = object.get("outcome") else {
        return Ok(None);
    };
    let Some(reason) = object.get("reason") else {
        return Ok(None);
    };
    let Some(err_eval_hash) = object.get("err_eval_hash") else {
        return Ok(None);
    };
    let Some(duplicate) = object.get("duplicate") else {
        return Ok(None);
    };

    Ok(Some(crate::events::TerminalErrEvalRecordedEvent {
        dag: parse_address_value(dag)?
            .ok_or_else(|| anyhow::anyhow!("Could not parse terminal err_eval dag: {dag}"))?,
        execution: parse_address_value(execution)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse terminal err_eval execution: {execution}")
        })?,
        walk_index: parse_u64_value(walk_index)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse terminal err_eval walk_index: {walk_index}")
        })?,
        vertex: parse_runtime_vertex_value(vertex)?
            .ok_or_else(|| anyhow::anyhow!("Could not parse terminal err_eval vertex: {vertex}"))?,
        leader: parse_address_value(leader)?
            .ok_or_else(|| anyhow::anyhow!("Could not parse terminal err_eval leader: {leader}"))?,
        failure_class: parse_published_move_enum_value(failure_class)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse terminal err_eval failure_class: {failure_class}")
        })?,
        outcome: parse_published_move_enum_value(outcome)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse terminal err_eval outcome: {outcome}")
        })?,
        reason: normalize_json_string(parse_string_value(reason)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse terminal err_eval reason: {reason}")
        })?),
        err_eval_hash: parse_byte_vector_value(err_eval_hash)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse terminal err_eval err_eval_hash: {err_eval_hash}")
        })?,
        duplicate: parse_bool_value(duplicate)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse terminal err_eval duplicate: {duplicate}")
        })?,
    }))
}

fn try_parse_submission_failure_evidence_recorded_event(
    value: &serde_json::Value,
) -> anyhow::Result<Option<crate::events::SubmissionFailureEvidenceRecordedEvent>> {
    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    let Some(execution) = object.get("execution") else {
        return Ok(None);
    };
    let Some(walk_index) = object.get("walk_index") else {
        return Ok(None);
    };
    let Some(vertex) = object.get("vertex") else {
        return Ok(None);
    };
    let Some(failed_leader) = object.get("failed_leader") else {
        return Ok(None);
    };
    let Some(winning_leader) = object.get("winning_leader") else {
        return Ok(None);
    };
    let Some(reason) = object.get("reason") else {
        return Ok(None);
    };
    let Some(err_eval_hash) = object.get("err_eval_hash") else {
        return Ok(None);
    };

    Ok(Some(
        crate::events::SubmissionFailureEvidenceRecordedEvent {
            execution: parse_address_value(execution)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse submission failure execution: {execution}")
            })?,
            walk_index: parse_u64_value(walk_index)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse submission failure walk_index: {walk_index}")
            })?,
            vertex: parse_runtime_vertex_value(vertex)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse submission failure vertex: {vertex}")
            })?,
            failed_leader: parse_address_value(failed_leader)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse submission failure failed_leader: {failed_leader}")
            })?,
            winning_leader: match parse_optional_string_value(winning_leader)? {
                Some(Some(value)) => {
                    Some(serde_json::from_value(serde_json::Value::String(value))?)
                }
                Some(None) => None,
                None => {
                    return Err(anyhow::anyhow!(
                        "Could not parse submission failure winning_leader: {winning_leader}"
                    ))
                }
            },
            reason: normalize_json_string(parse_string_value(reason)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse submission failure reason: {reason}")
            })?),
            err_eval_hash: parse_byte_vector_value(err_eval_hash)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse submission failure err_eval_hash: {err_eval_hash}")
            })?,
        },
    ))
}

fn try_parse_verification_verdict_event(
    value: &serde_json::Value,
) -> anyhow::Result<Option<crate::events::VerificationVerdictEvent>> {
    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    let Some(dag) = object.get("dag") else {
        return Ok(None);
    };
    let Some(execution) = object.get("execution") else {
        return Ok(None);
    };
    let Some(walk_index) = object.get("walk_index") else {
        return Ok(None);
    };
    let Some(vertex) = object.get("vertex") else {
        return Ok(None);
    };
    let Some(leader) = object.get("leader") else {
        return Ok(None);
    };
    let Some(submission_kind) = object.get("submission_kind") else {
        return Ok(None);
    };
    let Some(failure_evidence_kind) = object.get("failure_evidence_kind") else {
        return Ok(None);
    };
    let Some(leader_verifier_mode) = object.get("leader_verifier_mode") else {
        return Ok(None);
    };
    let Some(leader_verifier_method) = object.get("leader_verifier_method") else {
        return Ok(None);
    };
    let Some(tool_verifier_mode) = object.get("tool_verifier_mode") else {
        return Ok(None);
    };
    let Some(tool_verifier_method) = object.get("tool_verifier_method") else {
        return Ok(None);
    };
    let Some(checked_leader_kid) = object.get("checked_leader_kid") else {
        return Ok(None);
    };
    let Some(checked_tool_kid) = object.get("checked_tool_kid") else {
        return Ok(None);
    };
    let Some(payload_or_reason_hash) = object.get("payload_or_reason_hash") else {
        return Ok(None);
    };
    let Some(submission_role) = object.get("submission_role") else {
        return Ok(None);
    };
    let Some(checked_identity) = object.get("checked_identity") else {
        return Ok(None);
    };
    let Some(policy_mode) = object.get("policy_mode") else {
        return Ok(None);
    };
    let Some(verdict_reference) = object.get("verdict_reference") else {
        return Ok(None);
    };
    let Some(verdict) = object.get("verdict") else {
        return Ok(None);
    };

    Ok(Some(crate::events::VerificationVerdictEvent {
        dag: parse_address_value(dag)?
            .ok_or_else(|| anyhow::anyhow!("Could not parse verification verdict dag: {dag}"))?,
        execution: parse_address_value(execution)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse verification verdict execution: {execution}")
        })?,
        walk_index: parse_u64_value(walk_index)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse verification verdict walk_index: {walk_index}")
        })?,
        vertex: parse_runtime_vertex_value(vertex)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse verification verdict vertex: {vertex}")
        })?,
        leader: parse_address_value(leader)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse verification verdict leader: {leader}")
        })?,
        submission_kind: parse_published_move_enum_value(submission_kind)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Could not parse verification verdict submission_kind: {submission_kind}"
            )
        })?,
        failure_evidence_kind: parse_published_move_enum_value(failure_evidence_kind)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Could not parse verification verdict failure_evidence_kind: {failure_evidence_kind}"
                )
            })?,
        leader_verifier_mode: parse_published_move_enum_value(leader_verifier_mode)?.ok_or_else(
            || {
                anyhow::anyhow!(
                    "Could not parse verification verdict leader_verifier_mode: {leader_verifier_mode}"
                )
            },
        )?,
        leader_verifier_method: parse_string_value(leader_verifier_method)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Could not parse verification verdict leader_verifier_method: {leader_verifier_method}"
            )
        })?,
        tool_verifier_mode: parse_published_move_enum_value(tool_verifier_mode)?.ok_or_else(
            || {
                anyhow::anyhow!(
                    "Could not parse verification verdict tool_verifier_mode: {tool_verifier_mode}"
                )
            },
        )?,
        tool_verifier_method: parse_string_value(tool_verifier_method)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Could not parse verification verdict tool_verifier_method: {tool_verifier_method}"
            )
        })?,
        checked_leader_kid: parse_optional_u64_value(checked_leader_kid).map_err(|source| {
            anyhow::anyhow!(
                "Could not parse verification verdict checked_leader_kid: {checked_leader_kid}: {source}"
            )
        })?,
        checked_tool_kid: parse_optional_u64_value(checked_tool_kid).map_err(|source| {
            anyhow::anyhow!(
                "Could not parse verification verdict checked_tool_kid: {checked_tool_kid}: {source}"
            )
        })?,
        payload_or_reason_hash: parse_byte_vector_value(payload_or_reason_hash)?.ok_or_else(
            || {
                anyhow::anyhow!(
                    "Could not parse verification verdict payload_or_reason_hash: {payload_or_reason_hash}"
                )
            },
        )?,
        submission_role: parse_published_move_enum_value(submission_role)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Could not parse verification verdict submission_role: {submission_role}"
            )
        })?,
        checked_identity: parse_byte_vector_value(checked_identity)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Could not parse verification verdict checked_identity: {checked_identity}"
            )
        })?,
        policy_mode: parse_published_move_enum_value(policy_mode)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse verification verdict policy_mode: {policy_mode}")
        })?,
        verdict_reference: parse_byte_vector_value(verdict_reference)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Could not parse verification verdict verdict_reference: {verdict_reference}"
            )
        })?,
        verdict: parse_published_move_enum_value(verdict)?.ok_or_else(|| {
            anyhow::anyhow!("Could not parse verification verdict verdict: {verdict}")
        })?,
    }))
}

fn parse_optional_u64_value(value: &serde_json::Value) -> anyhow::Result<Option<u64>> {
    if value.is_null() {
        return Ok(None);
    }

    if let Some(parsed) = parse_u64_value(value)? {
        return Ok(Some(parsed));
    }

    Ok(serde_json::from_value::<MoveOption<u64>>(value.clone())?.0)
}

/// Helper function to determine whether the given address is one of the Nexus
/// package addresses.
fn is_nexus_package(address: sui::types::Address, objects: &NexusObjects) -> bool {
    address == objects.primitives_pkg_id
        || address == objects.interface_pkg_id
        || address == objects.registry_pkg_id()
        || objects.is_scheduler_package(address)
        || objects.is_workflow_package(address)
}

/// Helper function to determine whether the event name may be emitted while
/// Sui records a non-Nexus caller package as `event.package_id`.
fn allows_foreign_emitter(event_name: &str) -> bool {
    matches!(
        event_name,
        // Historical V1 interface announcements remain decodable for replay
        // and migration tooling.
        "AnnounceInterfacePackageEvent"
            // Standard TAP and workflow grant events can be emitted by public
            // Nexus functions invoked from a package-owned PTB.
            | "AgentSkillExecutionRequestedEvent"
            | "AgentSkillPaymentCreatedEvent"
            | "PaymentLockUpdateEvent"
            | "RequestScheduledWalkEvent"
            | "RequestWalkExecutionEvent"
            | "ScheduledAuthorizationGrantCreatedEvent"
            | "ScheduledAuthorizationGrantMaterializedEvent"
            | "VertexAuthorizationGrantCreatedEvent"
            | "VertexAuthorizationGrantRequiredEvent"
    )
}

/// Helper function to determine whether the provided struct tag corresponds to
/// `nexus_primitives::event::EventWrapper`.
fn is_event_wrapper(tag: &sui::types::StructTag, objects: &NexusObjects) -> bool {
    *tag.address() == objects.primitives_pkg_id
        && (*tag.module() == primitives::Event::EVENT_WRAPPER.module
            || *tag.module() == primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module)
        && (*tag.name() == primitives::Event::EVENT_WRAPPER.name
            || *tag.name() == primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.name)
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            events::*,
            idents::primitives,
            test_utils::sui_mocks,
            types::{
                AgentId,
                InterfaceRevision,
                PostFailureAction,
                RuntimeVertex,
                SharedObjectRef,
                SkillId,
                TapDagBinding,
                TapSharedObjectRef,
                TapSkillRequirements,
                TapVertexAuthorizationPlanEntry,
                TypeName,
                WorkflowFailureClass,
            },
        },
        serde::{Deserialize, Serialize},
        serde_json::json,
    };

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct Wrapper<T> {
        event: T,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct DistributedWrapperBcs<T> {
        event: T,
        deadline_ms: u64,
        requested_at_ms: u64,
        task_id: sui::types::Address,
        leaders: Vec<sui::types::Address>,
    }

    #[derive(Clone, Debug, Serialize)]
    struct MoveOptionBcs<T> {
        vec: Vec<T>,
    }

    impl<T> From<Option<T>> for MoveOptionBcs<T> {
        fn from(value: Option<T>) -> Self {
            Self {
                vec: value.into_iter().collect(),
            }
        }
    }

    fn wrapped_nexus_event<T: Serialize>(
        objects: &NexusObjects,
        emitter_package: sui::types::Address,
        inner_package: sui::types::Address,
        inner_module: &str,
        event_name: &str,
        event: T,
    ) -> sui::types::Event {
        let event_type = sui::types::StructTag::new(
            inner_package,
            sui::types::Identifier::new(inner_module).unwrap(),
            sui::types::Identifier::new(event_name).unwrap(),
            vec![],
        );
        wrapped_nexus_struct_event(objects, emitter_package, event_type, event)
    }

    fn wrapped_nexus_struct_event<T: Serialize>(
        objects: &NexusObjects,
        emitter_package: sui::types::Address,
        event_type: sui::types::StructTag,
        event: T,
    ) -> sui::types::Event {
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type));
        let bcs = bcs::to_bytes(&Wrapper { event }).expect("wrapped event should serialize");

        sui_mocks::mock_sui_event(
            emitter_package,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            bcs,
        )
    }

    fn distributed_nexus_struct_event<T: Serialize>(
        objects: &NexusObjects,
        emitter_package: sui::types::Address,
        event_type: sui::types::StructTag,
        event: T,
    ) -> sui::types::Event {
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type));
        let bcs = bcs::to_bytes(&DistributedWrapperBcs {
            event,
            deadline_ms: 30,
            requested_at_ms: 1500,
            task_id: sui::types::Address::from_static("0x51"),
            leaders: vec![
                sui::types::Address::from_static("0x52"),
                sui::types::Address::from_static("0x53"),
            ],
        })
        .expect("distributed wrapped event should serialize");

        sui_mocks::mock_sui_event(
            emitter_package,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            bcs,
        )
    }

    #[derive(Clone, Debug, Serialize)]
    struct RequestWalkExecutionEventBcs {
        dag: sui::types::Address,
        execution: sui::types::Address,
        invoker: sui::types::Address,
        walk_index: u64,
        next_vertex: RuntimeVertex,
        evaluations: sui::types::Address,
        worksheet_from_type: TypeName,
        worksheet_from_uid: sui::types::Address,
        tap_agent_id: MoveOptionBcs<AgentId>,
        tap_skill_id: MoveOptionBcs<SkillId>,
        tap_interface_revision: MoveOptionBcs<InterfaceRevision>,
        tap_endpoint_object_id: MoveOptionBcs<sui::types::Address>,
        tap_payment_id: MoveOptionBcs<sui::types::Address>,
        tap_selected_dag_id: MoveOptionBcs<sui::types::Address>,
        tap_authorization_plan_commitment: MoveOptionBcs<Vec<u8>>,
        tap_authorization_plan: Vec<TapVertexAuthorizationPlanEntry>,
        tap_scheduled_task_id: MoveOptionBcs<sui::types::Address>,
        tap_scheduled_occurrence_index: MoveOptionBcs<u64>,
    }

    #[derive(Clone, Debug, Serialize)]
    struct RequestScheduledWalkEventBcs {
        request: RequestWalkExecutionEventBcs,
        priority: u64,
        request_ms: u64,
        start_ms: u64,
        deadline_ms: u64,
    }

    #[derive(Clone, Debug, Serialize)]
    struct AgentSkillExecutionRequestedEventBcs {
        execution_id: sui::types::Address,
        agent_id: AgentId,
        skill_id: SkillId,
        interface_revision: InterfaceRevision,
        payment_id: sui::types::Address,
    }

    #[test]
    fn test_parse_from_grpc_valid_nexus_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("DAGCreatedEvent").unwrap(),
            vec![],
        );

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));
        // Manually craft a valid event kind and serialize as BCS
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: DAGCreatedEvent { dag: dag_addr },
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");

        let event = sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type.clone()],
            ),
            bcs,
        );

        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(result.is_ok(), "Should parse valid Nexus event");
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.generics, vec![]);
        assert_eq!(nexus_event.id, (digest, index));
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_addr
        ));
    }

    #[test]
    fn test_parse_from_grpc_valid_leader_cap_issued_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("leader").unwrap(),
            sui::types::Identifier::new("LeaderCapIssuedEvent").unwrap(),
            vec![],
        );

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));

        let registry = sui::types::Address::generate(&mut rng);
        let leader_cap_id = sui::types::Address::generate(&mut rng);
        let network = sui::types::Address::generate(&mut rng);
        let leader = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: LeaderCapIssuedEvent {
                registry,
                leader_cap_id,
                network,
                leader,
            },
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");

        let event = sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type.clone()],
            ),
            bcs,
        );

        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(result.is_ok(), "Should parse valid Nexus event");
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.generics, vec![]);
        assert_eq!(nexus_event.id, (digest, index));
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::LeaderCapIssued(LeaderCapIssuedEvent {
                registry: r,
                leader_cap_id: l,
                network: n,
                leader: lead,
            }) if r == registry && l == leader_cap_id && n == network && lead == leader
        ));
    }

    #[test]
    fn test_parse_from_grpc_valid_terminal_err_eval_recorded_event() {
        let index = 1u64;
        let digest = sui::types::Digest::generate(rand::thread_rng());
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("TerminalErrEvalRecordedEvent").unwrap(),
            vec![],
        );

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type));
        let data = Wrapper {
            event: TerminalErrEvalRecordedEvent {
                dag: sui::types::Address::ZERO,
                execution: sui::types::Address::TWO,
                walk_index: 4,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
                outcome: PostFailureAction::Terminate,
                reason: "timeout".to_string(),
                err_eval_hash: vec![9, 8, 7],
                duplicate: false,
            },
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");

        let event = sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            bcs,
        );

        let nexus_event = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects).unwrap();

        assert_eq!(nexus_event.generics, vec![]);
        assert_eq!(nexus_event.id, (digest, index));
        match nexus_event.data {
            NexusEventKind::TerminalErrEvalRecorded(parsed) => {
                assert_eq!(parsed.walk_index, 4);
                assert_eq!(
                    parsed.failure_class,
                    WorkflowFailureClass::TerminalSubmissionFailure
                );
                assert_eq!(parsed.outcome, PostFailureAction::Terminate);
                assert_eq!(parsed.reason, "timeout");
                assert_eq!(parsed.err_eval_hash, vec![9, 8, 7]);
                assert!(!parsed.duplicate);
            }
            _ => panic!("Expected TerminalErrEvalRecorded event"),
        }
    }

    #[test]
    fn test_parse_from_grpc_valid_payment_lock_update_event() {
        let index = 2u64;
        let digest = sui::types::Digest::generate(rand::thread_rng());
        let objects = sui_mocks::mock_nexus_objects();
        let execution = sui::types::Address::from_static("0xee");
        let vertex = RuntimeVertex::plain("payable");
        let tool_fqn = crate::fqn!("xyz.taluslabs.payable@1");
        let event_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("gas").unwrap(),
            sui::types::Identifier::new("PaymentLockUpdateEvent").unwrap(),
            vec![],
        );

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type));
        let data = Wrapper {
            event: PaymentLockUpdateEvent {
                execution,
                vertex: vertex.clone(),
                tool_fqn: tool_fqn.clone(),
                was_locked: true,
            },
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");

        let event = sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            bcs,
        );

        let nexus_event = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects).unwrap();
        match nexus_event.data {
            NexusEventKind::PaymentLockUpdate(parsed) => {
                assert_eq!(parsed.execution, execution);
                assert_eq!(parsed.vertex, vertex);
                assert_eq!(parsed.tool_fqn, tool_fqn);
                assert!(parsed.was_locked);
            }
            _ => panic!("Expected PaymentLockUpdate event"),
        }
    }

    #[test]
    fn test_parse_from_grpc_valid_payment_insufficient_gas_event() {
        let index = 3u64;
        let digest = sui::types::Digest::generate(rand::thread_rng());
        let objects = sui_mocks::mock_nexus_objects();
        let execution = sui::types::Address::from_static("0xee");
        let vertex = RuntimeVertex::plain("payable");
        let tool_fqn = crate::fqn!("xyz.taluslabs.payable@1");
        let event_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("gas").unwrap(),
            sui::types::Identifier::new("PaymentInsufficientGasEvent").unwrap(),
            vec![],
        );

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type));
        let data = Wrapper {
            event: PaymentInsufficientGasEvent {
                execution,
                vertex: vertex.clone(),
                tool_fqn: tool_fqn.clone(),
                required_tool_fee: 12,
                available_gas: 10,
            },
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");

        let event = sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            bcs,
        );

        let nexus_event = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects).unwrap();
        match nexus_event.data {
            NexusEventKind::PaymentInsufficientGas(parsed) => {
                assert_eq!(parsed.execution, execution);
                assert_eq!(parsed.vertex, vertex);
                assert_eq!(parsed.tool_fqn, tool_fqn);
                assert_eq!(parsed.required_tool_fee, 12);
                assert_eq!(parsed.available_gas, 10);
            }
            _ => panic!("Expected PaymentInsufficientGas event"),
        }
    }

    #[test]
    fn test_parse_from_grpc_valid_distributed_nexus_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("DAGCreatedEvent").unwrap(),
            vec![],
        );

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));
        // Manually craft a valid event kind and serialize as BCS
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = DistributedWrapperBcs {
            event: DAGCreatedEvent { dag: dag_addr },
            deadline_ms: 30,
            requested_at_ms: 1500,
            leaders: vec![sui::types::Address::ZERO],
            task_id: sui::types::Address::ZERO,
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");

        let event = sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type.clone()],
            ),
            bcs,
        );

        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(result.is_ok(), "Should parse valid Nexus event");
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.generics, vec![]);
        assert_eq!(nexus_event.id, (digest, index));
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_addr
        ));
        let distribution = nexus_event
            .distribution
            .as_ref()
            .expect("Distribution should be present");
        assert_eq!(distribution.deadline, chrono::Duration::milliseconds(30));
        assert_eq!(
            distribution.requested_at,
            chrono::DateTime::<chrono::Utc>::from_timestamp(1, 500_000_000).unwrap()
        );
        assert_eq!(distribution.leaders.len(), 1);
        assert_eq!(distribution.task_id, sui::types::Address::ZERO);
    }

    #[test]
    fn test_parse_from_grpc_non_nexus_package_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            sui::types::Address::generate(&mut rng),
            sui::types::Identifier::new("module").unwrap(),
            sui::types::Identifier::new("EventName").unwrap(),
            vec![],
        );
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));

        let event = sui_mocks::mock_sui_event(
            sui::types::Address::generate(&mut rng),
            sui::types::StructTag::new(
                sui::types::Address::generate(&mut rng),
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            vec![1, 2, 3],
        );

        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(result.is_err(), "Should fail for non-Nexus package event");
    }

    #[test]
    fn test_parse_from_grpc_non_nexus_package_inner_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            sui::types::Address::generate(&mut rng),
            sui::types::Identifier::new("module").unwrap(),
            sui::types::Identifier::new("EventName").unwrap(),
            vec![],
        );
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));

        let event = sui_mocks::mock_sui_event(
            objects.workflow_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            vec![1, 2, 3],
        );

        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(
            result.is_err(),
            "Should fail for non-Nexus package inner event"
        );
    }

    #[test]
    fn test_parse_from_grpc_non_nexus_package_event_foreign_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            objects.interface_pkg_id,
            sui::types::Identifier::from_static("v1"),
            sui::types::Identifier::from_static("AnnounceInterfacePackageEvent"),
            vec![sui::types::TypeTag::Address],
        );
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));

        let data = Wrapper {
            event: AnnounceInterfacePackageEvent {
                shared_objects: vec![SharedObjectRef::new_imm(sui::types::Address::generate(
                    &mut rng,
                ))],
            },
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");
        let event = sui_mocks::mock_sui_event(
            sui::types::Address::generate(&mut rng),
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type],
            ),
            bcs,
        );

        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects)
            .expect("Should parse foreign event from non-Nexus package");

        assert_eq!(result.id, (digest, index));
        assert!(matches!(
            result.data,
            crate::events::NexusEventKind::AnnounceInterfacePackage(
                AnnounceInterfacePackageEvent { shared_objects }
            ) if shared_objects == data.event.shared_objects
        ));
    }

    #[test]
    fn test_parse_from_grpc_public_tap_events_from_foreign_emitter_package() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let emitter_package = sui::types::Address::generate(&mut rng);

        let requested_event = wrapped_nexus_event(
            &objects,
            emitter_package,
            objects.interface_pkg_id,
            "tap",
            "AgentSkillExecutionRequestedEvent",
            AgentSkillExecutionRequestedEventBcs {
                execution_id: sui::types::Address::from_static("0x22"),
                agent_id: sui::types::Address::from_static("0x1"),
                skill_id: 2,
                interface_revision: InterfaceRevision(9),
                payment_id: sui::types::Address::from_static("0x23"),
            },
        );
        let parsed =
            NexusEvent::from_sui_grpc_event(0, digest, &requested_event, &objects).unwrap();
        assert!(matches!(
            parsed.data,
            NexusEventKind::AgentSkillExecutionRequested(AgentSkillExecutionRequestedEvent {
                payment_id,
                ..
            }) if payment_id == sui::types::Address::from_static("0x23")
        ));

        let payment_event = wrapped_nexus_event(
            &objects,
            emitter_package,
            objects.interface_pkg_id,
            "tap",
            "AgentSkillPaymentCreatedEvent",
            AgentSkillPaymentCreatedEvent {
                payment_id: sui::types::Address::from_static("0x36"),
                execution_id: sui::types::Address::from_static("0x37"),
                agent_id: sui::types::Address::from_static("0x1"),
                skill_id: 2,
                interface_revision: InterfaceRevision(9),
                payer: sui::types::Address::from_static("0x38"),
                source_kind: TapPaymentSourceKind::AgentVault,
                source_identity: sui::types::Address::from_static("0x1"),
                max_budget: 10_000,
                locked_budget: 10_000,
            },
        );
        let parsed = NexusEvent::from_sui_grpc_event(1, digest, &payment_event, &objects).unwrap();
        assert!(matches!(
            parsed.data,
            NexusEventKind::AgentSkillPaymentCreated(AgentSkillPaymentCreatedEvent {
                source_kind: TapPaymentSourceKind::AgentVault,
                locked_budget: 10_000,
                ..
            })
        ));

        let grant_event = wrapped_nexus_event(
            &objects,
            emitter_package,
            objects.workflow_pkg_id,
            "dag",
            "VertexAuthorizationGrantCreatedEvent",
            VertexAuthorizationGrantCreatedEvent {
                grant_id: sui::types::Address::from_static("0x44"),
                execution_id: sui::types::Address::from_static("0x45"),
                vertex: RuntimeVertex::plain("transfer"),
            },
        );
        let parsed = NexusEvent::from_sui_grpc_event(2, digest, &grant_event, &objects).unwrap();
        assert!(matches!(
            parsed.data,
            NexusEventKind::VertexAuthorizationGrantCreated(VertexAuthorizationGrantCreatedEvent {
                vertex,
                ..
            }) if vertex == RuntimeVertex::plain("transfer")
        ));

        let request_event = distributed_nexus_struct_event(
            &objects,
            emitter_package,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                sui::types::Identifier::new("scheduled_request").unwrap(),
                sui::types::Identifier::new("RequestScheduledExecution").unwrap(),
                vec![sui::types::TypeTag::Struct(Box::new(
                    sui::types::StructTag::new(
                        objects.workflow_pkg_id,
                        sui::types::Identifier::new("dag").unwrap(),
                        sui::types::Identifier::new("RequestWalkExecutionEvent").unwrap(),
                        vec![],
                    ),
                ))],
            ),
            RequestScheduledWalkEventBcs {
                request: RequestWalkExecutionEventBcs {
                    dag: sui::types::Address::from_static("0xa"),
                    execution: sui::types::Address::from_static("0xb"),
                    invoker: sui::types::Address::from_static("0xc"),
                    walk_index: 1,
                    next_vertex: RuntimeVertex::plain("transfer"),
                    evaluations: sui::types::Address::from_static("0xd"),
                    worksheet_from_type: TypeName::from("demo::tap::Witness"),
                    worksheet_from_uid: sui::types::Address::from_static("0xe"),
                    tap_agent_id: Some(sui::types::Address::from_static("0x1")).into(),
                    tap_skill_id: Some(2).into(),
                    tap_interface_revision: Some(InterfaceRevision(3)).into(),
                    tap_endpoint_object_id: Some(sui::types::Address::from_static("0xf")).into(),
                    tap_payment_id: Some(sui::types::Address::from_static("0x10")).into(),
                    tap_selected_dag_id: Some(sui::types::Address::from_static("0x11")).into(),
                    tap_authorization_plan_commitment: Some(vec![1, 2, 3]).into(),
                    tap_authorization_plan: Vec::<TapVertexAuthorizationPlanEntry>::new(),
                    tap_scheduled_task_id: None.into(),
                    tap_scheduled_occurrence_index: None.into(),
                },
                priority: 1,
                request_ms: 2,
                start_ms: 3,
                deadline_ms: 4,
            },
        );
        let parsed = NexusEvent::from_sui_grpc_event(3, digest, &request_event, &objects).unwrap();
        assert!(parsed.distribution.is_some());
        assert!(matches!(
            parsed.data,
            NexusEventKind::RequestScheduledWalk(RequestScheduledWalkEvent {
                request,
                ..
            }) if request.next_vertex == RuntimeVertex::plain("transfer")
        ));

        let lock_event = wrapped_nexus_event(
            &objects,
            emitter_package,
            objects.workflow_pkg_id,
            "gas",
            "PaymentLockUpdateEvent",
            PaymentLockUpdateEvent {
                execution: sui::types::Address::from_static("0x56"),
                vertex: RuntimeVertex::plain("transfer"),
                tool_fqn: crate::fqn!("demo.taluslabs.demo_onchain_vertex@1"),
                was_locked: true,
            },
        );
        let parsed = NexusEvent::from_sui_grpc_event(4, digest, &lock_event, &objects).unwrap();
        assert!(matches!(
            parsed.data,
            NexusEventKind::PaymentLockUpdate(PaymentLockUpdateEvent {
                vertex,
                was_locked: true,
                ..
            }) if vertex == RuntimeVertex::plain("transfer")
        ));
    }

    #[test]
    fn test_parse_from_grpc_foreign_emitter_rejects_unlisted_nexus_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_nexus_event(
            &objects,
            sui::types::Address::generate(&mut rng),
            objects.workflow_pkg_id,
            "dag",
            "DAGCreatedEvent",
            DAGCreatedEvent {
                dag: sui::types::Address::from_static("0xda6"),
            },
        );

        let result = NexusEvent::from_sui_grpc_event(0, digest, &event, &objects);
        assert!(
            result.is_err(),
            "foreign package emitters remain rejected for non-public TAP events"
        );
    }

    #[test]
    fn test_parse_from_grpc_non_event_wrapper_type() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let wrong_tag = sui::types::StructTag::new(
            objects.primitives_pkg_id,
            sui::types::Identifier::new("wrong_module").unwrap(),
            sui::types::Identifier::new("wrong_name").unwrap(),
            vec![],
        );
        let event = sui_mocks::mock_sui_event(objects.primitives_pkg_id, wrong_tag, vec![1, 2, 3]);
        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(result.is_err(), "Should fail for non-EventWrapper type");
    }

    #[test]
    fn test_parse_from_grpc_event_wrapper_missing_type_param() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let wrapper_tag = sui::types::StructTag::new(
            objects.primitives_pkg_id,
            sui::types::Identifier::new(primitives::Event::EVENT_WRAPPER.module.as_str()).unwrap(),
            sui::types::Identifier::new(primitives::Event::EVENT_WRAPPER.name.as_str()).unwrap(),
            vec![],
        );
        let event =
            sui_mocks::mock_sui_event(objects.primitives_pkg_id, wrapper_tag, vec![1, 2, 3]);
        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(result.is_err(), "Should fail for missing type param");
    }

    #[test]
    fn test_parse_from_grpc_valid_nexus_event_with_generics() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let event_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("DAGCreatedEvent").unwrap(),
            vec![sui::types::TypeTag::U64],
        );

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));
        // Manually craft a valid event kind and serialize as BCS
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: DAGCreatedEvent { dag: dag_addr },
        };
        let bcs = bcs::to_bytes(&data).expect("BCS serialization should succeed");

        let event = sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![wrapper_type.clone()],
            ),
            bcs,
        );

        let result = NexusEvent::from_sui_grpc_event(index, digest, &event, &objects);
        assert!(result.is_ok(), "Should parse valid Nexus event");
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.id, (digest, index));
        assert_eq!(nexus_event.generics, vec![sui::types::TypeTag::U64]);
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_addr
        ));
    }

    #[test]
    fn test_parse_sample_events() {
        for (name, bytes) in sample_events() {
            let result = parse_bcs(name, &bytes);

            assert!(result.is_ok(), "'{name}' event failed to parse: {result:?}")
        }
    }

    #[test]
    fn standard_tap_samples_cover_current_move_option_layouts() {
        let samples = current_standard_tap_samples();
        let names = samples.iter().map(|(name, _)| *name).collect::<Vec<_>>();

        assert!(names.contains(&"RequestWalkExecutionEvent"));
        assert!(names.contains(&"RequestScheduledWalkEvent"));
        assert!(names.contains(&"AgentSkillExecutionRequestedEvent"));

        for (name, bytes) in samples {
            let (event, distribution) =
                parse_bcs(name, &bytes).unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert!(
                distribution.is_none(),
                "{name} sample should use the direct event wrapper"
            );

            match event {
                NexusEventKind::RequestWalkExecution(event) => {
                    let context = event
                        .standard_tap_context()
                        .expect("standard TAP context should validate")
                        .expect("standard TAP context should be present");
                    assert_eq!(context.agent_id, sui::types::Address::from_static("0x1"));
                    assert_eq!(context.interface_revision, InterfaceRevision(3));
                    assert_eq!(context.authorization_plan_commitment, Some(vec![1, 2, 3]));
                }
                NexusEventKind::RequestScheduledWalk(event) => {
                    let context = event
                        .request
                        .standard_tap_context()
                        .expect("scheduled standard TAP context should validate")
                        .expect("scheduled standard TAP context should be present");
                    assert_eq!(context.agent_id, sui::types::Address::from_static("0xa"));
                    assert_eq!(context.interface_revision, InterfaceRevision(1));
                    assert_eq!(
                        context.authorization_plan_commitment,
                        Some(vec![1, 2, 3, 4])
                    );
                    assert_eq!(
                        context.scheduled_task_id,
                        Some(sui::types::Address::from_static("0x12"))
                    );
                    assert_eq!(context.scheduled_occurrence_index, Some(7));
                }
                NexusEventKind::AgentSkillExecutionRequested(event) => {
                    assert_eq!(event.payment_id, sui::types::Address::from_static("0x23"));
                }
                _ => {}
            }
        }
    }

    fn current_standard_tap_samples() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            (
                "AgentCreatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: AgentCreatedEvent {
                        agent_id: sui::types::Address::from_static("0x1"),
                        vault_id: sui::types::Address::from_static("0x1a"),
                        owner: sui::types::Address::from_static("0x18"),
                        operator: sui::types::Address::from_static("0x19"),
                    },
                })
                .expect("AgentCreatedEvent sample serializes"),
            ),
            (
                "SkillRegisteredEvent",
                bcs::to_bytes(&Wrapper {
                    event: SkillRegisteredEvent {
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        dag_id: sui::types::Address::from_static("0x1a"),
                        dag_binding: TapDagBinding::pinned(sui::types::Address::from_static("0x1a")),
                        workflow_commitment: vec![1],
                        requirements_commitment: vec![2],
                        capability_schema_commitment: vec![5],
                    },
                })
                .expect("SkillRegisteredEvent sample serializes"),
            ),
            (
                "DefaultDagExecutorUpdatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: DefaultDagExecutorUpdatedEvent {
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                    },
                })
                .expect("DefaultDagExecutorUpdatedEvent sample serializes"),
            ),
            (
                "EndpointRevisionAnnouncedEvent",
                bcs::to_bytes(&Wrapper {
                    event: EndpointRevisionAnnouncedEvent {
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        endpoint_object_id: sui::types::Address::from_static("0x1d"),
                        endpoint_object_version: 4,
                        endpoint_object_digest: vec![7; 32],
                        shared_objects: vec![TapSharedObjectRef::mutable(
                            sui::types::Address::from_static("0x1e"),
                        )],
                        requirements: TapSkillRequirements::default(),
                        config_digest: vec![8, 8],
                        active_for_new_executions: true,
                    },
                })
                .expect("EndpointRevisionAnnouncedEvent sample serializes"),
            ),
            (
                "EndpointRevisionActivatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: EndpointRevisionActivatedEvent {
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        active_for_new_executions: true,
                    },
                })
                .expect("EndpointRevisionActivatedEvent sample serializes"),
            ),
            (
                "WorksheetResolvedEvent",
                bcs::to_bytes(&Wrapper {
                    event: WorksheetResolvedEvent {
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        endpoint_object_id: sui::types::Address::from_static("0x1f"),
                        execution_id: sui::types::Address::from_static("0x20"),
                        worksheet_id: sui::types::Address::from_static("0x21"),
                    },
                })
                .expect("WorksheetResolvedEvent sample serializes"),
            ),
            (
                "AgentSkillExecutionRequestedEvent",
                bcs::to_bytes(&Wrapper {
                    event: AgentSkillExecutionRequestedEventBcs {
                        execution_id: sui::types::Address::from_static("0x22"),
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        payment_id: sui::types::Address::from_static("0x23"),
                    },
                })
                .expect("AgentSkillExecutionRequestedEvent sample serializes"),
            ),
            (
                "AgentSkillPaymentCreatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: AgentSkillPaymentCreatedEvent {
                        payment_id: sui::types::Address::from_static("0x36"),
                        execution_id: sui::types::Address::from_static("0x37"),
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        payer: sui::types::Address::from_static("0x38"),
                        source_kind: TapPaymentSourceKind::Invoker,
                        source_identity: sui::types::Address::from_static("0x38"),
                        max_budget: 10_000,
                        locked_budget: 10_000,
                    },
                })
                .expect("AgentSkillPaymentCreatedEvent sample serializes"),
            ),
            (
                "GasPaymentConsumedEvent",
                bcs::to_bytes(&Wrapper {
                    event: GasPaymentConsumedEvent {
                        payment_id: sui::types::Address::from_static("0x39"),
                        execution_id: sui::types::Address::from_static("0x3a"),
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        amount: 500,
                        consumed_total: 900,
                    },
                })
                .expect("GasPaymentConsumedEvent sample serializes"),
            ),
            (
                "ExecutionAccomplishedEvent",
                bcs::to_bytes(&Wrapper {
                    event: ExecutionAccomplishedEvent {
                        execution_id: sui::types::Address::from_static("0x3b"),
                        payment_id: sui::types::Address::from_static("0x3c"),
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                    },
                })
                .expect("ExecutionAccomplishedEvent sample serializes"),
            ),
            (
                "ExecutionRefundedEvent",
                bcs::to_bytes(&Wrapper {
                    event: ExecutionRefundedEvent {
                        execution_id: sui::types::Address::from_static("0x3d"),
                        payment_id: sui::types::Address::from_static("0x3e"),
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        refund_reason: vec![7, 8],
                    },
                })
                .expect("ExecutionRefundedEvent sample serializes"),
            ),
            (
                "ScheduledSkillExecutionCreatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: ScheduledSkillExecutionCreatedEvent {
                        scheduled_task_id: sui::types::Address::from_static("0x3f"),
                        scheduler_task_id: sui::types::Address::from_static("0x44"),
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        long_term_gas_coin_id: sui::types::Address::from_static("0x40"),
                        schedule_entries_commitment: vec![1, 4, 9],
                        first_after_ms: 1000,
                        max_occurrences: 5,
                        source_kind: TapPaymentSourceKind::Invoker,
                        source_identity: sui::types::Address::from_static("0x40"),
                        prepaid_amount: 50,
                        occurrence_budget: 10,
                        refund_mode: 0,
                    },
                })
                .expect("ScheduledSkillExecutionCreatedEvent sample serializes"),
            ),
            (
                "ScheduledSkillExecutionTriggeredEvent",
                bcs::to_bytes(&Wrapper {
                    event: ScheduledSkillExecutionTriggeredEvent {
                        scheduled_task_id: sui::types::Address::from_static("0x41"),
                        execution_id: sui::types::Address::from_static("0x42"),
                        agent_id: sui::types::Address::from_static("0x1"),
                        skill_id: 2,
                        interface_revision: InterfaceRevision(9),
                        occurrence_index: 2,
                    },
                })
                .expect("ScheduledSkillExecutionTriggeredEvent sample serializes"),
            ),
            (
                "ScheduledSkillExecutionCompletedEvent",
                bcs::to_bytes(&Wrapper {
                    event: ScheduledSkillExecutionCompletedEvent {
                        scheduled_task_id: sui::types::Address::from_static("0x43"),
                        execution_id: sui::types::Address::from_static("0x44"),
                        continue_recurring: true,
                        next_after_ms: 2000,
                    },
                })
                .expect("ScheduledSkillExecutionCompletedEvent sample serializes"),
            ),
            (
                "RequestWalkExecutionEvent",
                bcs::to_bytes(&Wrapper {
                    event: RequestWalkExecutionEventBcs {
                        dag: sui::types::Address::from_static("0xa"),
                        execution: sui::types::Address::from_static("0xb"),
                        invoker: sui::types::Address::from_static("0xc"),
                        walk_index: 1,
                        next_vertex: RuntimeVertex::plain("vertex"),
                        evaluations: sui::types::Address::from_static("0xd"),
                        worksheet_from_type: TypeName::from(
                            "d000000000000000000000000000000000000000000000000000000000000000::dag::LeaderRegistryWorkflowWitness",
                        ),
                        worksheet_from_uid: sui::types::Address::from_static("0xe"),
                        tap_agent_id: Some(sui::types::Address::from_static("0x1")).into(),
                        tap_skill_id: Some(2).into(),
                        tap_interface_revision: Some(InterfaceRevision(3)).into(),
                        tap_endpoint_object_id: Some(sui::types::Address::from_static("0xf")).into(),
                        tap_payment_id: Some(sui::types::Address::from_static("0x10")).into(),
                        tap_selected_dag_id: Some(sui::types::Address::from_static("0x11")).into(),
                        tap_authorization_plan_commitment: Some(vec![1, 2, 3]).into(),
                        tap_authorization_plan: Vec::<TapVertexAuthorizationPlanEntry>::new(),
                        tap_scheduled_task_id: None.into(),
                        tap_scheduled_occurrence_index: None.into(),
                    },
                })
                .expect("RequestWalkExecutionEvent sample serializes"),
            ),
            (
                "RequestScheduledWalkEvent",
                bcs::to_bytes(&Wrapper {
                    event: RequestScheduledWalkEventBcs {
                        request: RequestWalkExecutionEventBcs {
                            dag: sui::types::Address::from_static("0x1"),
                            execution: sui::types::Address::from_static("0x2"),
                            invoker: sui::types::Address::from_static("0x3"),
                            walk_index: 0,
                            next_vertex: RuntimeVertex::plain("dummy"),
                            evaluations: sui::types::Address::from_static("0x4"),
                            worksheet_from_type: TypeName::from("legacy::compat::Witness"),
                            worksheet_from_uid: sui::types::Address::from_static("0x5"),
                            tap_agent_id: Some(sui::types::Address::from_static("0xa"))
                                .into(),
                            tap_skill_id: Some(11)
                                .into(),
                            tap_interface_revision: Some(InterfaceRevision(1)).into(),
                            tap_endpoint_object_id: Some(sui::types::Address::from_static("0xc"))
                                .into(),
                            tap_payment_id: Some(sui::types::Address::from_static("0xd")).into(),
                            tap_selected_dag_id: Some(sui::types::Address::from_static("0xe"))
                                .into(),
                            tap_authorization_plan_commitment: Some(vec![1, 2, 3, 4]).into(),
                            tap_authorization_plan: Vec::new(),
                            tap_scheduled_task_id: Some(sui::types::Address::from_static("0x12"))
                                .into(),
                            tap_scheduled_occurrence_index: Some(7).into(),
                        },
                        priority: 1,
                        request_ms: 2,
                        start_ms: 3,
                        deadline_ms: 4,
                    },
                })
                .expect("RequestScheduledWalkEvent sample serializes"),
            ),
            (
                "PaymentInsufficientGasEvent",
                bcs::to_bytes(&Wrapper {
                    event: PaymentInsufficientGasEvent {
                        execution: sui::types::Address::from_static("0x70"),
                        vertex: RuntimeVertex::plain("payable"),
                        tool_fqn: crate::fqn!("xyz.taluslabs.payable@1"),
                        required_tool_fee: 12,
                        available_gas: 10,
                    },
                })
                .expect("PaymentInsufficientGasEvent sample serializes"),
            ),
        ]
    }

    /// Return deterministic BCS samples emitted by the matching Move event types.
    fn sample_events() -> Vec<(&'static str, Vec<u8>)> {
        let mut samples = vec![
            (
                "DAGCreatedEvent",
                vec![
                    172, 45, 232, 250, 15, 55, 177, 42, 63, 139, 114, 186, 218, 6, 79, 233, 155,
                    245, 118, 65, 38, 9, 194, 133, 80, 214, 234, 139, 42, 249, 215, 254,
                ],
            ),
            (
                "PaymentLockUpdateEvent",
                vec![
                    137, 85, 88, 251, 70, 35, 154, 244, 157, 83, 95, 160, 229, 41, 235, 87, 49, 34,
                    108, 227, 130, 217, 34, 60, 63, 1, 217, 168, 78, 221, 225, 177, 1, 5, 100, 117,
                    109, 109, 121, 16, 120, 121, 122, 46, 100, 117, 109, 109, 121, 46, 116, 111,
                    111, 108, 64, 49, 1,
                ],
            ),
            (
                "ToolRegisteredEvent",
                vec![
                    53, 118, 162, 75, 202, 80, 114, 229, 20, 139, 102, 88, 41, 247, 106, 81, 231,
                    122, 179, 18, 162, 131, 113, 77, 191, 203, 73, 146, 208, 212, 185, 171, 28,
                    120, 121, 122, 46, 116, 97, 108, 117, 115, 108, 97, 98, 115, 46, 109, 97, 116,
                    104, 46, 105, 54, 52, 46, 115, 117, 109, 64, 49,
                ],
            ),
            (
                "AnnounceInterfacePackageEvent",
                vec![
                    1, 30, 119, 100, 18, 153, 38, 229, 238, 194, 76, 38, 173, 14, 59, 134, 129, 97,
                    127, 227, 222, 102, 203, 227, 137, 8, 168, 65, 31, 190, 45, 0, 151, 0,
                ],
            ),
            (
                "FoundingLeaderCapCreatedEvent",
                vec![
                    220, 77, 44, 250, 39, 146, 163, 254, 224, 253, 94, 74, 105, 99, 64, 142, 187,
                    76, 70, 202, 207, 69, 223, 66, 20, 104, 0, 21, 159, 182, 106, 170, 7, 147, 201,
                    4, 107, 90, 177, 234, 233, 159, 79, 235, 110, 104, 9, 97, 134, 200, 7, 65, 153,
                    183, 255, 82, 32, 55, 192, 14, 111, 197, 5, 247,
                ],
            ),
            (
                "ExecutionFinishedEvent",
                vec![
                    76, 145, 234, 176, 46, 104, 79, 149, 7, 4, 155, 4, 34, 47, 112, 132, 107, 166,
                    75, 155, 168, 106, 231, 169, 17, 231, 42, 55, 254, 13, 32, 182, 12, 64, 190,
                    126, 42, 153, 71, 21, 43, 93, 197, 119, 139, 178, 53, 131, 225, 154, 24, 101,
                    138, 228, 101, 237, 112, 225, 252, 204, 192, 102, 88, 49, 0, 1, 0,
                ],
            ),
            (
                "EndStateReachedEvent",
                vec![
                    76, 145, 234, 176, 46, 104, 79, 149, 7, 4, 155, 4, 34, 47, 112, 132, 107, 166,
                    75, 155, 168, 106, 231, 169, 17, 231, 42, 55, 254, 13, 32, 182, 12, 64, 190,
                    126, 42, 153, 71, 21, 43, 93, 197, 119, 139, 178, 53, 131, 225, 154, 24, 101,
                    138, 228, 101, 237, 112, 225, 252, 204, 192, 102, 88, 49, 0, 0, 0, 0, 0, 0, 0,
                    0, 1, 5, 100, 117, 109, 109, 121, 2, 111, 107, 1, 7, 109, 101, 115, 115, 97,
                    103, 101, 6, 105, 110, 108, 105, 110, 101, 24, 34, 89, 111, 117, 32, 115, 97,
                    105, 100, 58, 32, 72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 33, 34,
                    0,
                ],
            ),
            (
                "RequestScheduledOccurrenceEvent",
                vec![
                    234, 49, 197, 185, 6, 194, 12, 9, 9, 187, 27, 164, 244, 58, 29, 51, 14, 42, 79,
                    10, 177, 123, 69, 28, 27, 131, 12, 131, 102, 182, 151, 83, 0, 98, 99, 50, 48,
                    49, 54, 53, 51, 56, 51, 99, 56, 48, 101, 102, 48, 51, 57, 49, 54, 101, 51, 51,
                    99, 48, 52, 99, 101, 49, 98, 54, 101, 55, 99, 98, 98, 56, 100, 97, 48, 50, 48,
                    53, 48, 98, 50, 49, 101, 101, 49, 101, 100, 55, 50, 101, 52, 99, 97, 55, 57,
                    53, 99, 101, 55, 49, 58, 58, 115, 99, 104, 101, 100, 117, 108, 101, 114, 58,
                    58, 81, 117, 101, 117, 101, 71, 101, 110, 101, 114, 97, 116, 111, 114, 87, 105,
                    116, 110, 101, 115, 115, 1, 0, 0, 0, 0, 0, 0, 0, 27, 208, 108, 195, 156, 1, 0,
                    0, 71, 209, 108, 195, 156, 1, 0, 0, 135, 240, 108, 195, 156, 1, 0, 0, 48, 117,
                    0, 0, 0, 0, 0, 0, 71, 209, 108, 195, 156, 1, 0, 0, 28, 106, 230, 75, 241, 192,
                    93, 183, 209, 11, 222, 12, 98, 199, 206, 166, 195, 132, 112, 190, 13, 133, 140,
                    121, 192, 39, 92, 217, 2, 190, 93, 179, 2, 157, 22, 199, 54, 48, 18, 169, 158,
                    216, 68, 111, 79, 42, 245, 75, 45, 204, 1, 239, 67, 252, 89, 220, 243, 127, 29,
                    130, 3, 144, 9, 81, 223, 70, 239, 6, 15, 239, 195, 145, 34, 90, 230, 52, 78,
                    245, 173, 196, 178, 236, 75, 142, 174, 7, 76, 106, 189, 66, 229, 139, 43, 142,
                    105, 152, 182,
                ],
            ),
            (
                "OccurrenceConsumedEvent",
                vec![
                    234, 49, 197, 185, 6, 194, 12, 9, 9, 187, 27, 164, 244, 58, 29, 51, 14, 42, 79,
                    10, 177, 123, 69, 28, 27, 131, 12, 131, 102, 182, 151, 83, 71, 209, 108, 195,
                    156, 1, 0, 0, 1, 135, 240, 108, 195, 156, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
                    98, 99, 50, 48, 49, 54, 53, 51, 56, 51, 99, 56, 48, 101, 102, 48, 51, 57, 49,
                    54, 101, 51, 51, 99, 48, 52, 99, 101, 49, 98, 54, 101, 55, 99, 98, 98, 56, 100,
                    97, 48, 50, 48, 53, 48, 98, 50, 49, 101, 101, 49, 101, 100, 55, 50, 101, 52,
                    99, 97, 55, 57, 53, 99, 101, 55, 49, 58, 58, 115, 99, 104, 101, 100, 117, 108,
                    101, 114, 58, 58, 81, 117, 101, 117, 101, 71, 101, 110, 101, 114, 97, 116, 111,
                    114, 87, 105, 116, 110, 101, 115, 115, 134, 211, 108, 195, 156, 1, 0, 0,
                ],
            ),
            (
                "WalkAdvancedEvent",
                vec![
                    25, 13, 140, 141, 215, 138, 116, 155, 39, 47, 68, 22, 144, 0, 154, 167, 99,
                    115, 183, 30, 10, 144, 218, 96, 19, 136, 161, 170, 121, 189, 179, 24, 75, 66,
                    44, 41, 248, 78, 49, 235, 213, 109, 239, 122, 242, 143, 7, 85, 166, 51, 204, 9,
                    167, 127, 186, 225, 193, 81, 236, 140, 132, 134, 167, 51, 0, 0, 0, 0, 0, 0, 0,
                    0, 1, 11, 105, 115, 95, 110, 101, 103, 97, 116, 105, 118, 101, 2, 108, 116, 1,
                    1, 97, 6, 105, 110, 108, 105, 110, 101, 2, 45, 50, 0,
                ],
            ),
            (
                // Iterator vertex.
                "WalkAdvancedEvent",
                vec![
                    167, 109, 87, 152, 85, 138, 214, 135, 181, 142, 46, 148, 154, 181, 45, 55, 50,
                    112, 158, 51, 29, 193, 65, 180, 24, 31, 58, 96, 164, 229, 24, 241, 121, 206,
                    166, 237, 250, 255, 104, 46, 58, 104, 195, 74, 63, 218, 252, 132, 50, 98, 158,
                    114, 103, 48, 159, 76, 253, 3, 41, 39, 116, 241, 134, 70, 0, 0, 0, 0, 0, 0, 0,
                    0, 1, 10, 99, 114, 101, 97, 116, 101, 95, 118, 101, 99, 2, 111, 107, 1, 6, 114,
                    101, 115, 117, 108, 116, 6, 105, 110, 108, 105, 110, 101, 0, 3, 1, 49, 1, 50,
                    1, 51,
                ],
            ),
            (
                "LeaderCapIssuedEvent",
                vec![
                    205, 19, 59, 181, 227, 175, 174, 63, 109, 25, 51, 51, 242, 35, 41, 91, 77, 200,
                    127, 205, 231, 244, 143, 137, 215, 215, 6, 177, 184, 68, 172, 140, 43, 59, 169,
                    207, 177, 188, 84, 54, 147, 44, 93, 140, 42, 177, 128, 69, 212, 56, 135, 113,
                    85, 146, 111, 58, 159, 33, 38, 214, 146, 18, 236, 17, 145, 173, 70, 152, 47,
                    201, 1, 29, 239, 119, 79, 143, 4, 102, 8, 181, 255, 163, 194, 79, 158, 155, 5,
                    220, 76, 145, 127, 10, 190, 156, 156, 79, 230, 125, 33, 187, 163, 211, 146,
                    144, 156, 249, 196, 219, 221, 2, 159, 23, 145, 102, 193, 115, 199, 38, 49, 145,
                    44, 100, 109, 189, 198, 0, 29, 25,
                ],
            ),
            (
                "AgentCreatedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 25, 4, 1, 2, 3, 4, 1,
                ],
            ),
            (
                "SkillRegisteredEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 26, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 26, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 27, 1, 1, 1, 2, 1, 3, 1, 4, 1, 5,
                ],
            ),
            (
                "EndpointRevisionAnnouncedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 29, 4, 0, 0, 0, 0, 0, 0, 0, 32, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
                    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 1, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 30, 5, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 1, 3, 0, 16, 39, 0, 0, 0, 0, 0,
                    0, 2, 1, 2, 2, 3, 4, 111, 110, 99, 101, 10, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0,
                    0, 0, 0, 1, 1, 4, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 48, 4, 116, 111, 111, 108, 3, 114, 117,
                    110, 1, 5, 1, 2, 8, 8, 1,
                ],
            ),
            (
                "EndpointRevisionActivatedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0, 0, 0, 0, 0, 0, 0, 1,
                ],
            ),
            (
                "WorksheetResolvedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33,
                ],
            ),
            (
                "AgentSkillExecutionRequestedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 35,
                ],
            ),
            (
                "AgentSkillPaymentCreatedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 56, 16, 39, 0, 0, 0, 0, 0, 0, 2,
                ],
            ),
            (
                "GasPaymentConsumedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 57, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 58, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0,
                    0, 0, 0, 0, 0, 0, 244, 1, 0, 0, 0, 0, 0, 0, 132, 3, 0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                "ExecutionAccomplishedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0,
                    0, 0, 0, 0, 0, 0, 3, 4, 5, 6,
                ],
            ),
            (
                "ExecutionRefundedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 61, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 62, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0,
                    0, 0, 0, 0, 0, 0, 2, 7, 8,
                ],
            ),
            (
                "ScheduledSkillExecutionCreatedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 3, 1,
                    4, 9, 232, 3, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                "ScheduledSkillExecutionTriggeredEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 65, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 0,
                    0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                "ScheduledSkillExecutionCompletedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 67, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 68, 1, 208, 7, 0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                "RequestWalkExecutionEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 12, 1, 0, 0, 0, 0, 0, 0, 0,
                    1, 6, 118, 101, 114, 116, 101, 120, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 13, 100, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 58,
                    58, 100, 97, 103, 58, 58, 76, 101, 97, 100, 101, 114, 82, 101, 103, 105, 115,
                    116, 114, 121, 87, 111, 114, 107, 102, 108, 111, 119, 87, 105, 116, 110, 101,
                    115, 115, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 14, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 3, 0,
                    0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16, 1, 3, 1, 2, 3,
                ],
            ),
            (
                "OccurrenceScheduledEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 33, 0, 98, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 58, 58, 115, 99, 104, 101, 100,
                    117, 108, 101, 114, 58, 58, 81, 117, 101, 117, 101, 71, 101, 110, 101, 114, 97,
                    116, 111, 114, 87, 105, 116, 110, 101, 115, 115,
                ],
            ),
            (
                "ToolUnregisteredEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 70, 27, 120, 121, 122, 46, 116, 97, 108, 117, 115, 108, 97, 98,
                    115, 46, 115, 97, 109, 112, 108, 101, 95, 116, 111, 111, 108, 64, 49,
                ],
            ),
            (
                "WalkFailedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 73, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 74, 4, 0, 0, 0, 0, 0, 0, 0, 0, 8, 102, 97,
                    105, 108, 97, 98, 108, 101, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 14,
                    115, 97, 109, 112, 108, 101, 32, 102, 97, 105, 108, 117, 114, 101,
                ],
            ),
            (
                "TerminalErrEvalRecordedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 75, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 76, 5, 0, 0, 0, 0, 0, 0, 0, 1, 8, 116, 101,
                    114, 109, 105, 110, 97, 108, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 77, 1, 0, 22, 111, 117, 116, 112,
                    117, 116, 32, 115, 99, 104, 101, 109, 97, 32, 109, 105, 115, 109, 97, 116, 99,
                    104, 3, 8, 7, 6, 0,
                ],
            ),
            (
                "VerificationVerdictEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 78, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 79, 6, 0, 0, 0, 0, 0, 0, 0, 1, 8, 118, 101,
                    114, 105, 102, 105, 101, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 80, 0, 0, 1, 14, 115, 105, 103,
                    110, 101, 100, 95, 104, 116, 116, 112, 95, 118, 49, 0, 0, 1, 11, 0, 0, 0, 0, 0,
                    0, 0, 0, 4, 1, 2, 3, 4, 1, 2, 5, 6, 1, 2, 7, 8, 0,
                ],
            ),
            (
                "WalkAbortedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 81, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 82, 7, 0, 0, 0, 0, 0, 0, 0, 1, 5, 97, 98, 111,
                    114, 116,
                ],
            ),
            (
                "WalkCancelledEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 83, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 84, 8, 0, 0, 0, 0, 0, 0, 0, 1, 6, 99, 97, 110,
                    99, 101, 108,
                ],
            ),
            (
                "MissedOccurrenceEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 89, 232, 3, 0, 0, 0, 0, 0, 0, 1, 76, 4, 0, 0, 0, 0, 0, 0, 176,
                    4, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 0, 0, 0, 0, 98, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48,
                    48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 48, 58, 58,
                    115, 99, 104, 101, 100, 117, 108, 101, 114, 58, 58, 81, 117, 101, 117, 101, 71,
                    101, 110, 101, 114, 97, 116, 111, 114, 87, 105, 116, 110, 101, 115, 115,
                ],
            ),
            (
                "TaskCreatedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 90, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 91,
                ],
            ),
            (
                "TaskPausedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 92,
                ],
            ),
            (
                "TaskResumedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 93,
                ],
            ),
            (
                "TaskCanceledEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 94,
                ],
            ),
            (
                "PeriodicScheduleConfiguredEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 96, 1, 136, 19, 0, 0, 0, 0, 0, 0, 1, 244, 1, 0, 0, 0, 0, 0, 0,
                    1, 12, 0, 0, 0, 0, 0, 0, 0, 1, 3, 0, 0, 0, 0, 0, 0, 0, 99, 0, 0, 0, 0, 0, 0, 0,
                    1, 16, 39, 0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                "ToolRegistryCreatedEvent",
                vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 105,
                ],
            ),
        ];
        samples.retain(|(name, _)| {
            !matches!(
                *name,
                "AgentCreatedEvent"
                    | "SkillRegisteredEvent"
                    | "DefaultDagExecutorUpdatedEvent"
                    | "EndpointRevisionAnnouncedEvent"
                    | "EndpointRevisionActivatedEvent"
                    | "WorksheetResolvedEvent"
                    | "AgentSkillExecutionRequestedEvent"
                    | "AgentSkillPaymentCreatedEvent"
                    | "GasPaymentConsumedEvent"
                    | "ExecutionAccomplishedEvent"
                    | "ExecutionRefundedEvent"
                    | "ScheduledSkillExecutionCreatedEvent"
                    | "ScheduledSkillExecutionTriggeredEvent"
                    | "ScheduledSkillExecutionCompletedEvent"
                    | "RequestWalkExecutionEvent"
                    | "RequestScheduledWalkEvent"
            )
        });
        samples.extend(current_standard_tap_samples());
        samples
    }

    #[test]
    fn test_parse_terminal_err_eval_recorded_event_value_nested_trace_shape() {
        let parsed = parse_terminal_err_eval_recorded_event_value(json!({
            "trace": [{
                "event": {
                    "fields": {
                        "dag": "0x1",
                        "execution": "0x2",
                        "walk_index": { "number": "4" },
                        "vertex": {
                            "fields": {
                                "_variant_name": "WithIterator",
                                "vertex": "failable",
                                "iteration": "1",
                                "out_of": 3
                            }
                        },
                        "leader": { "value": "0x3" },
                        "failure_class": "terminal_submission_failure",
                        "outcome": "terminate",
                        "reason": "\"timeout\"",
                        "err_eval_hash": { "bytes": [9, 8, 7] },
                        "duplicate": false
                    }
                }
            }]
        }))
        .expect("event should parse");

        assert_eq!(
            parsed,
            Some(TerminalErrEvalRecordedEvent {
                dag: sui::types::Address::from_static("0x1"),
                execution: sui::types::Address::TWO,
                walk_index: 4,
                vertex: RuntimeVertex::with_iterator("failable", 1, 3),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
                outcome: PostFailureAction::Terminate,
                reason: "timeout".to_string(),
                err_eval_hash: vec![9, 8, 7],
                duplicate: false,
            })
        );
    }

    #[test]
    fn test_parse_terminal_err_eval_recorded_event_value_parsed_json_wrapper_shape() {
        let parsed = parse_terminal_err_eval_recorded_event_value(json!({
            "parsed_json": {
                "event": {
                    "fields": {
                        "dag": "0x1",
                        "execution": "0x2",
                        "walk_index": "4",
                        "vertex": {
                            "_variant_name": "Plain",
                            "vertex": { "name": "failable" }
                        },
                        "leader": "0x3",
                        "failure_class": "terminal_submission_failure",
                        "outcome": "terminate",
                        "reason": "\"timeout\"",
                        "err_eval_hash": { "bytes": [9, 8, 7] },
                        "duplicate": false
                    }
                }
            }
        }))
        .expect("parsed_json wrapper should parse");

        assert_eq!(
            parsed,
            Some(TerminalErrEvalRecordedEvent {
                dag: sui::types::Address::from_static("0x1"),
                execution: sui::types::Address::TWO,
                walk_index: 4,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
                outcome: PostFailureAction::Terminate,
                reason: "timeout".to_string(),
                err_eval_hash: vec![9, 8, 7],
                duplicate: false,
            })
        );
    }

    #[test]
    fn test_parse_submission_failure_evidence_recorded_event_nested_option_shape() {
        let parsed = parse_submission_failure_evidence_recorded_event(json!({
            "wrapper": {
                "event": {
                    "fields": {
                        "execution": "0x2",
                        "walk_index": "6",
                        "vertex": {
                            "_variant_name": "Plain",
                            "vertex": { "name": "failable" }
                        },
                        "failed_leader": "0x3",
                        "winning_leader": {
                            "fields": {
                                "_variant_name": "Some",
                                "value": "0x4"
                            }
                        },
                        "reason": "\"rpc timeout\"",
                        "err_eval_hash": { "bytes": "0x0102ff" }
                    }
                }
            }
        }))
        .expect("submission failure should parse");

        assert_eq!(
            parsed,
            Some(SubmissionFailureEvidenceRecordedEvent {
                execution: sui::types::Address::TWO,
                walk_index: 6,
                vertex: RuntimeVertex::plain("failable"),
                failed_leader: sui::types::Address::THREE,
                winning_leader: Some(sui::types::Address::from_static("0x4")),
                reason: "rpc timeout".to_string(),
                err_eval_hash: vec![1, 2, 255],
            })
        );
    }
}
