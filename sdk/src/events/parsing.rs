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
            parse_published_move_enum_value,
            parse_runtime_vertex_value,
            parse_string_value,
            parse_u64_value,
            workflow,
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

        let event_name = normalize_event_name(event_type, objects)?;

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
) -> anyhow::Result<Option<crate::types::workflow::execution_events::TerminalErrEvalRecordedEvent>>
{
    parse_nested_event_value(value, try_parse_terminal_err_eval_recorded_event)
}

/// Parse a nested Move-JSON payload into submission-failure evidence.
pub fn parse_submission_failure_evidence_recorded_event(
    value: serde_json::Value,
) -> anyhow::Result<
    Option<crate::types::workflow::execution_events::SubmissionFailureEvidenceRecordedEvent>,
> {
    parse_nested_event_value(value, try_parse_submission_failure_evidence_recorded_event)
}

/// Parse a nested Move-JSON payload into a verification-verdict event.
pub fn parse_verification_verdict_event(
    value: serde_json::Value,
) -> anyhow::Result<Option<crate::types::workflow::execution_events::VerificationVerdictEvent>> {
    parse_nested_event_value(value, try_parse_verification_verdict_event)
}

fn normalize_event_name(
    event_type: &sui::types::StructTag,
    objects: &NexusObjects,
) -> anyhow::Result<String> {
    let name = event_type.name().as_str();

    if name != "RequestScheduledExecution" {
        return Ok(name.to_string());
    }

    if *event_type.address() != objects.interface_pkg_id
        || event_type.module().as_str() != "scheduled_request"
    {
        bail!(
            "RequestScheduledExecution does not come from Nexus scheduled_request, found '{}::{}'",
            event_type.address(),
            event_type.module()
        );
    }

    let Some(type_tag) = event_type.type_params().first() else {
        bail!("RequestScheduledExecution is missing a type parameter");
    };

    let sui::types::TypeTag::Struct(struct_tag) = type_tag else {
        bail!("RequestScheduledExecution expects a struct type parameter");
    };

    let normalized = match struct_tag.name().as_str() {
        "OccurrenceScheduledEvent"
            if objects.is_scheduler_package(*struct_tag.address())
                && struct_tag.module().as_str() == "scheduler" =>
        {
            "RequestScheduledOccurrenceEvent"
        }
        "OccurrenceScheduledEvent" => {
            bail!(
                "RequestScheduledExecution occurrence payload does not come from Nexus scheduler, found '{}::{}'",
                struct_tag.address(),
                struct_tag.module()
            )
        }
        "RequestWalkExecutionEvent"
            if objects.is_workflow_package(*struct_tag.address())
                && struct_tag.module().as_str() == "execution_events" =>
        {
            "RequestWalkExecutionEvent"
        }
        "RequestWalkExecutionEvent" => {
            bail!(
                "RequestScheduledExecution walk payload does not come from Nexus workflow, found '{}::{}'",
                struct_tag.address(),
                struct_tag.module()
            )
        }
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
) -> anyhow::Result<Option<crate::types::workflow::execution_events::TerminalErrEvalRecordedEvent>>
{
    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    for key in [
        "dag",
        "execution",
        "walk_index",
        "vertex",
        "leader",
        "failure_class",
        "outcome",
        "reason",
        "err_eval_hash",
        "duplicate",
    ] {
        if !object.contains_key(key) {
            return Ok(None);
        }
    }

    let Some(raw) =
        parse_move_event::<workflow::execution_events::TerminalErrEvalRecordedEvent>(value)?
    else {
        return Ok(None);
    };

    Ok(Some(raw.into_public()))
}

fn try_parse_submission_failure_evidence_recorded_event(
    value: &serde_json::Value,
) -> anyhow::Result<
    Option<crate::types::workflow::execution_events::SubmissionFailureEvidenceRecordedEvent>,
> {
    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    if !object.contains_key("execution") {
        return Ok(None);
    }
    if !object.contains_key("walk_index") {
        return Ok(None);
    }
    if !object.contains_key("vertex") {
        return Ok(None);
    }
    if !object.contains_key("failed_leader") {
        return Ok(None);
    }
    if !object.contains_key("winning_leader") {
        return Ok(None);
    }
    if !object.contains_key("reason") {
        return Ok(None);
    }
    if !object.contains_key("err_eval_hash") {
        return Ok(None);
    }

    let value = event_value_with_default_dag(value);
    let Some(raw) = parse_move_event::<
        workflow::execution_events::SubmissionFailureEvidenceRecordedEvent,
    >(&value)?
    else {
        return Ok(None);
    };

    Ok(Some(raw.into_public()))
}

fn event_value_with_default_dag(value: &serde_json::Value) -> serde_json::Value {
    let serde_json::Value::Object(mut object) = value.clone() else {
        return value.clone();
    };
    object
        .entry("dag".to_string())
        .or_insert_with(|| serde_json::Value::String(sui::types::Address::ZERO.to_string()));
    serde_json::Value::Object(object)
}

fn try_parse_verification_verdict_event(
    value: &serde_json::Value,
) -> anyhow::Result<Option<crate::types::workflow::execution_events::VerificationVerdictEvent>> {
    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    for key in [
        "execution",
        "walk_index",
        "vertex",
        "leader",
        "submission_kind",
        "failure_evidence_kind",
        "leader_verifier_mode",
        "leader_verifier_method",
        "tool_verifier_mode",
        "tool_verifier_method",
        "checked_leader_kid",
        "checked_tool_kid",
        "payload_or_reason_hash",
        "checked_identity",
        "verdict_reference",
        "verdict",
    ] {
        if !object.contains_key(key) {
            return Ok(None);
        }
    }

    let value = event_value_with_default_dag(value);
    let Some(raw) =
        parse_move_event::<workflow::execution_events::VerificationVerdictEvent>(&value)?
    else {
        return Ok(None);
    };

    Ok(Some(raw.into_public()))
}

trait IntoPublicEvent {
    type Public;

    fn into_public(self) -> Self::Public;
}

impl IntoPublicEvent for workflow::execution_events::TerminalErrEvalRecordedEvent {
    type Public = crate::types::workflow::execution_events::TerminalErrEvalRecordedEvent;

    fn into_public(self) -> Self::Public {
        self
    }
}

impl IntoPublicEvent for workflow::execution_events::SubmissionFailureEvidenceRecordedEvent {
    type Public = crate::types::workflow::execution_events::SubmissionFailureEvidenceRecordedEvent;

    fn into_public(self) -> Self::Public {
        self
    }
}

impl IntoPublicEvent for workflow::execution_events::VerificationVerdictEvent {
    type Public = crate::types::workflow::execution_events::VerificationVerdictEvent;

    fn into_public(self) -> Self::Public {
        self
    }
}

fn parse_move_event<T>(value: &serde_json::Value) -> anyhow::Result<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    let normalized = serde_json::Value::Object(
        object
            .iter()
            .map(|(key, value)| Ok((key.clone(), normalize_move_event_field(key, value)?)))
            .collect::<anyhow::Result<_>>()?,
    );

    Ok(Some(serde_json::from_value(normalized)?))
}

fn normalize_move_event_field(
    key: &str,
    value: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value = match key {
        "dag" | "execution" => id_json(value)?,
        "leader" | "failed_leader" => parse_address_value(value)?
            .map(|address| serde_json::Value::String(address.to_string()))
            .unwrap_or_else(|| value.clone()),
        "winning_leader" => value.clone(),
        "checked_leader_kid" | "checked_tool_kid" => optional_u64_json(value)?,
        "walk_index" => parse_u64_value(value)?
            .map(serde_json::Value::from)
            .unwrap_or_else(|| value.clone()),
        "vertex" => {
            serde_json::to_value(parse_runtime_vertex_value(value)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse generated event vertex: {value}")
            })?)?
        }
        "failure_class" => workflow_failure_class_json(value)?,
        "outcome"
        | "submission_kind"
        | "failure_evidence_kind"
        | "leader_verifier_mode"
        | "tool_verifier_mode"
        | "verdict" => normalize_move_enum_json(value)?,
        "reason" | "leader_verifier_method" | "tool_verifier_method" => serde_json::Value::String(
            normalize_json_string(parse_string_value(value)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse generated event string field {key}: {value}")
            })?),
        ),
        "err_eval_hash" | "payload_or_reason_hash" | "checked_identity" | "verdict_reference" => {
            serde_json::to_value(parse_byte_vector_value(value)?.ok_or_else(|| {
                anyhow::anyhow!("Could not parse generated event byte-vector field {key}: {value}")
            })?)?
        }
        "duplicate" => parse_bool_value(value)?
            .map(serde_json::Value::from)
            .unwrap_or_else(|| value.clone()),
        _ => value.clone(),
    };
    Ok(value)
}

fn optional_u64_json(value: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    if value.is_null() {
        return Ok(value.clone());
    }

    if let Some(parsed) = parse_u64_value(value)? {
        return Ok(serde_json::Value::from(parsed));
    }

    let serde_json::Value::Object(object) = value else {
        return Ok(value.clone());
    };

    if let Some(fields) = object.get("fields") {
        let normalized = optional_u64_json(fields)?;
        if &normalized != fields {
            let mut object = object.clone();
            object.insert("fields".to_string(), normalized);
            return Ok(serde_json::Value::Object(object));
        }
    }

    if let Some(vec) = object.get("vec").and_then(serde_json::Value::as_array) {
        let normalized = vec
            .iter()
            .map(optional_u64_json)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mut object = object.clone();
        object.insert("vec".to_string(), serde_json::Value::Array(normalized));
        return Ok(serde_json::Value::Object(object));
    }

    Ok(value.clone())
}

fn id_json(value: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let address = parse_address_value(value)?
        .ok_or_else(|| anyhow::anyhow!("Could not parse generated event object id: {value}"))?;
    Ok(serde_json::json!({ "bytes": address }))
}

fn normalize_move_enum_json(value: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let parsed = parse_published_move_enum_value::<String>(value)?;
    Ok(parsed
        .map(serde_json::Value::String)
        .unwrap_or_else(|| value.clone()))
}

fn workflow_failure_class_json(value: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let parsed = parse_published_move_enum_value::<crate::types::WorkflowFailureClass>(value)?
        .ok_or_else(|| anyhow::anyhow!("Could not parse workflow failure class: {value}"))?;
    let variant = match parsed {
        crate::types::WorkflowFailureClass::Retryable => "Retryable",
        crate::types::WorkflowFailureClass::TerminalToolFailure => "TerminalToolFailure",
        crate::types::WorkflowFailureClass::TerminalSubmissionFailure => {
            "TerminalSubmissionFailure"
        }
    };
    Ok(serde_json::Value::String(variant.to_string()))
}

/// Helper function to determine whether the given address is one of the Nexus
/// package addresses.
fn is_nexus_package(address: sui::types::Address, objects: &NexusObjects) -> bool {
    address == objects.primitives_pkg_id
        || address == objects.interface_pkg_id
        || address == objects.registry_pkg_id
        || objects.is_scheduler_package(address)
        || objects.is_workflow_package(address)
}

/// Helper function to determine whether the event name may be emitted while
/// Sui records a non-Nexus caller package as `event.package_id`.
fn allows_foreign_emitter(event_name: &str) -> bool {
    matches!(
        event_name,
        "AgentSkillExecutionRequestedEvent"
            | "AgentSkillPaymentCreatedEvent"
            | "AgentVertexAuthorizationRequiredEvent"
            | "PaymentLockUpdateEvent"
            | "RequestScheduledOccurrenceEvent"
            | "RequestWalkExecutionEvent"
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
mod direct_event_tests {
    use {
        super::*,
        crate::{
            events::{parse_bcs, NexusEventKind},
            idents::primitives,
            sui,
            test_utils::sui_mocks,
            types::{
                interface::{
                    agent::{self as agent_types, *},
                    dag::*,
                    payment::{self as payment_types, *},
                    scheduled_request,
                    version,
                },
                primitives::policy::Symbol as PolicySymbol,
                registry::{agent_registry::*, leader::*, leader_cap::*, tool_registry::*},
                scheduler::scheduler::*,
                sui_framework::{
                    object::ID,
                    vec_map::{Entry as VecMapEntry, VecMap as MoveVecMap},
                },
                workflow::{execution_events::*, gas::*},
                MoveOption,
                MoveString,
                PostFailureAction,
                RuntimeVertex,
                TypeName,
                WorkflowFailureClass,
            },
        },
        serde::Serialize,
        serde_json::json,
    };

    type RequestScheduledOccurrenceEvent =
        scheduled_request::RequestScheduledExecution<OccurrenceScheduledEvent>;

    #[derive(Serialize)]
    struct Wrapper<T> {
        event: T,
    }

    #[derive(Serialize)]
    struct DistributedWrapper<T> {
        event: T,
        deadline_ms: u64,
        requested_at_ms: u64,
        task_id: sui::types::Address,
        leaders: Vec<sui::types::Address>,
    }

    fn id(bytes: sui::types::Address) -> ID {
        ID { bytes }
    }

    fn addr(byte: u8) -> sui::types::Address {
        sui::types::Address::from([byte; 32])
    }

    fn interface_version(inner: u64) -> version::InterfaceVersion {
        version::InterfaceVersion { inner }
    }

    fn generator() -> PolicySymbol {
        PolicySymbol::witness(TypeName::new("0xa5::scheduler::QueueGeneratorWitness"))
    }

    fn skill_dag_binding() -> agent_types::SkillDagBinding {
        agent_types::SkillDagBinding::Pinned { dag_id: addr(0xd1) }
    }

    fn skill_requirements() -> agent_types::SkillRequirement {
        agent_types::SkillRequirement {
            input_commitment: vec![1, 2, 3],
            payment_policy: payment_types::SkillPaymentPolicy::UserFunded,
            schedule_policy: agent_types::SkillSchedulePolicy {
                recurrence: agent_types::SkillRecurrenceKind::Once,
                allow_recursive: false,
            },
            fixed_tools: vec![],
        }
    }

    fn payment_source() -> payment_types::PaymentSourceKind {
        payment_types::PaymentSourceKind::UserFunded { user: addr(0xf0) }
    }

    fn empty_ports_data() -> MoveVecMap<
        crate::types::interface::graph::OutputPort,
        crate::types::primitives::data::NexusData,
    > {
        MoveVecMap { contents: vec![] }
    }

    fn assert_move_event_parses<T>(
        objects: &crate::types::NexusObjects,
        event_name: &str,
        inner: sui::types::StructTag,
        event: T,
        expected_name: &str,
        index: u64,
    ) -> NexusEventKind
    where
        T: Serialize,
    {
        let bytes = bcs::to_bytes(&Wrapper { event }).unwrap();
        let (parsed, distribution) = parse_bcs(event_name, &bytes).unwrap();

        assert!(
            distribution.is_none(),
            "{event_name} unexpectedly parsed as distributed"
        );
        assert_eq!(parsed.name(), expected_name);

        let emitter_package = *inner.address();
        let wrapped = sui::types::Event {
            package_id: emitter_package,
            module: primitives::Event::EVENT_WRAPPER.module,
            sender: addr(0xee),
            type_: sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            ),
            contents: bytes,
        };

        let parsed_grpc =
            NexusEvent::from_sui_grpc_event(index, sui::types::Digest::ZERO, &wrapped, objects)
                .unwrap();

        assert_eq!(parsed_grpc.id, (sui::types::Digest::ZERO, index));
        assert!(parsed_grpc.distribution.is_none());
        assert_eq!(parsed_grpc.data.name(), expected_name);

        parsed
    }

    fn inner_tag(
        package: sui::types::Address,
        module: &'static str,
        name: &'static str,
        type_params: Vec<sui::types::TypeTag>,
    ) -> sui::types::StructTag {
        sui::types::StructTag::new(
            package,
            sui::types::Identifier::from_static(module),
            sui::types::Identifier::from_static(name),
            type_params,
        )
    }

    fn wrapped_event<T: Serialize>(
        objects: &crate::types::NexusObjects,
        emitter_package: sui::types::Address,
        inner: sui::types::StructTag,
        event: T,
    ) -> sui::types::Event {
        sui::types::Event {
            package_id: emitter_package,
            module: primitives::Event::EVENT_WRAPPER.module,
            sender: addr(0xee),
            type_: sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            ),
            contents: bcs::to_bytes(&Wrapper { event }).unwrap(),
        }
    }

    #[test]
    fn parse_bcs_uses_direct_dag_created_event() {
        let dag = id(sui::types::Address::from_static("0xabc"));
        let bytes = bcs::to_bytes(&Wrapper {
            event: DAGCreatedEvent { dag: dag.clone() },
        })
        .unwrap();

        let (event, distribution) = parse_bcs("DAGCreatedEvent", &bytes).unwrap();

        assert!(distribution.is_none());
        assert!(
            matches!(event, NexusEventKind::DAGCreated(DAGCreatedEvent { dag: parsed }) if parsed == dag)
        );
    }

    #[test]
    fn parse_bcs_uses_direct_request_walk_event() {
        let agent_id = id(sui::types::Address::from_static("0x1"));
        let bytes = bcs::to_bytes(&Wrapper {
            event: RequestWalkExecutionEvent {
                dag: id(sui::types::Address::from_static("0xa")),
                execution: id(sui::types::Address::from_static("0xb")),
                invoker: sui::types::Address::from_static("0xc"),
                walk_index: 1,
                next_vertex: RuntimeVertex::plain("vertex"),
                evaluations: id(sui::types::Address::from_static("0xd")),
                agent_id: agent_id.clone(),
                skill_id: 2,
                interface_version: version::InterfaceVersion { inner: 3 },
                scheduled_task_id: MoveOption(None),
                scheduled_occurrence_index: MoveOption(None),
            },
        })
        .unwrap();

        let (event, distribution) = parse_bcs("RequestWalkExecutionEvent", &bytes).unwrap();

        assert!(distribution.is_none());
        match event {
            NexusEventKind::RequestWalkExecution(event) => {
                assert_eq!(event.agent_id, agent_id);
                let context = event.to_context().unwrap().unwrap();
                assert_eq!(context.agent_id, sui::types::Address::from_static("0x1"));
                assert_eq!(context.skill_id, 2);
                assert_eq!(context.interface_revision.inner, 3);
            }
            _ => panic!("expected RequestWalkExecution event"),
        }
    }

    #[test]
    fn from_sui_grpc_event_unwraps_distributed_scheduled_request_walk_event() {
        let objects = sui_mocks::mock_nexus_objects();
        let execution = id(addr(0xb1));
        let task_id = addr(0x44);
        let leaders = vec![addr(0x45), addr(0x46)];
        let request = RequestWalkExecutionEvent {
            dag: id(addr(0xb0)),
            execution: execution.clone(),
            invoker: addr(0xb2),
            walk_index: 12,
            next_vertex: RuntimeVertex::plain("scheduled_walk"),
            evaluations: id(addr(0xb3)),
            agent_id: id(addr(0xb4)),
            skill_id: 13,
            interface_version: interface_version(14),
            scheduled_task_id: MoveOption(None),
            scheduled_occurrence_index: MoveOption(None),
        };
        let scheduled = scheduled_request::RequestScheduledExecution {
            request,
            priority: 15,
            request_ms: 16,
            start_ms: 17,
            deadline_ms: 18,
        };
        let inner = inner_tag(
            objects.interface_pkg_id,
            "scheduled_request",
            "RequestScheduledExecution",
            vec![sui::types::TypeTag::Struct(Box::new(inner_tag(
                objects.workflow_pkg_id,
                "execution_events",
                "RequestWalkExecutionEvent",
                vec![],
            )))],
        );
        let event = sui::types::Event {
            package_id: objects.workflow_pkg_id,
            module: primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module,
            sender: addr(0xee),
            type_: sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.name,
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            ),
            contents: bcs::to_bytes(&DistributedWrapper {
                event: scheduled,
                deadline_ms: 19,
                requested_at_ms: 20,
                task_id,
                leaders: leaders.clone(),
            })
            .unwrap(),
        };

        let parsed =
            NexusEvent::from_sui_grpc_event(7, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::RequestWalkExecution(RequestWalkExecutionEvent { execution: parsed_execution, walk_index: 12, .. })
                if parsed_execution == execution
        ));
        let distribution = parsed
            .distribution
            .expect("scheduled request carries distribution metadata");
        assert_eq!(distribution.task_id, task_id);
        assert_eq!(distribution.leaders, leaders);
    }

    #[test]
    fn nested_terminal_err_eval_json_parses_direct_event() {
        let parsed = parse_terminal_err_eval_recorded_event_value(json!({
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
                    "failure_class": "TerminalSubmissionFailure",
                    "outcome": "Terminate",
                    "reason": "\"timeout\"",
                    "err_eval_hash": [9, 8, 7],
                    "duplicate": false
                }
            }
        }))
        .unwrap();

        assert_eq!(
            parsed,
            Some(TerminalErrEvalRecordedEvent {
                dag: id(sui::types::Address::from_static("0x1")),
                execution: id(sui::types::Address::TWO),
                walk_index: 4,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
                outcome: MoveOption(Some(PostFailureAction::Terminate)),
                reason: MoveString::from("timeout"),
                err_eval_hash: vec![9, 8, 7],
                duplicate: false,
            })
        );
    }

    #[test]
    fn from_sui_grpc_event_rejects_non_wrapper_event() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = sui::types::Event {
            package_id: objects.interface_pkg_id,
            module: sui::types::Identifier::from_static("dag"),
            sender: addr(0xef),
            type_: inner_tag(objects.interface_pkg_id, "dag", "DAGCreatedEvent", vec![]),
            contents: bcs::to_bytes(&Wrapper {
                event: DAGCreatedEvent {
                    dag: id(addr(0xa3)),
                },
            })
            .unwrap(),
        };

        let err = NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects)
            .unwrap_err();

        assert!(err.to_string().contains("Event is not wrapped"));
    }

    #[test]
    fn from_sui_grpc_event_rejects_foreign_inner_event() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_event(
            &objects,
            objects.primitives_pkg_id,
            inner_tag(addr(0xf1), "dag", "DAGCreatedEvent", vec![]),
            DAGCreatedEvent {
                dag: id(addr(0xa4)),
            },
        );

        let err = NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects)
            .unwrap_err();

        assert!(err
            .to_string()
            .contains("Inner event does not come from a Nexus package"));
    }

    #[test]
    fn from_sui_grpc_event_rejects_foreign_scheduled_request_payload() {
        let objects = sui_mocks::mock_nexus_objects();
        let scheduled_walk = scheduled_request::RequestScheduledExecution {
            request: RequestWalkExecutionEvent {
                dag: id(addr(0xb0)),
                execution: id(addr(0xb1)),
                invoker: addr(0xb2),
                walk_index: 12,
                next_vertex: RuntimeVertex::plain("foreign_scheduled_walk"),
                evaluations: id(addr(0xb3)),
                agent_id: id(addr(0xb4)),
                skill_id: 13,
                interface_version: interface_version(14),
                scheduled_task_id: MoveOption(None),
                scheduled_occurrence_index: MoveOption(None),
            },
            priority: 15,
            request_ms: 16,
            start_ms: 17,
            deadline_ms: 18,
        };
        let foreign_walk_payload = inner_tag(
            addr(0xf4),
            "execution_events",
            "RequestWalkExecutionEvent",
            vec![],
        );
        let walk_event = sui::types::Event {
            package_id: addr(0xf5),
            module: primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module,
            sender: addr(0xee),
            type_: sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.name,
                vec![sui::types::TypeTag::Struct(Box::new(inner_tag(
                    objects.interface_pkg_id,
                    "scheduled_request",
                    "RequestScheduledExecution",
                    vec![sui::types::TypeTag::Struct(Box::new(foreign_walk_payload))],
                )))],
            ),
            contents: bcs::to_bytes(&DistributedWrapper {
                event: scheduled_walk,
                deadline_ms: 19,
                requested_at_ms: 20,
                task_id: addr(0x44),
                leaders: vec![addr(0x45), addr(0x46)],
            })
            .unwrap(),
        };

        let err =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &walk_event, &objects)
                .unwrap_err();

        assert!(err
            .to_string()
            .contains("walk payload does not come from Nexus workflow"));

        let scheduled_occurrence = RequestScheduledOccurrenceEvent {
            request: OccurrenceScheduledEvent {
                task: id(addr(0xc0)),
                generator: generator(),
            },
            priority: 21,
            request_ms: 22,
            start_ms: 23,
            deadline_ms: 24,
        };
        let foreign_occurrence_payload =
            inner_tag(addr(0xf6), "scheduler", "OccurrenceScheduledEvent", vec![]);
        let occurrence_event = sui::types::Event {
            package_id: addr(0xf7),
            module: primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module,
            sender: addr(0xee),
            type_: sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.module,
                primitives::DistributedEvent::DISTRIBUTED_EVENT_WRAPPER.name,
                vec![sui::types::TypeTag::Struct(Box::new(inner_tag(
                    objects.interface_pkg_id,
                    "scheduled_request",
                    "RequestScheduledExecution",
                    vec![sui::types::TypeTag::Struct(Box::new(
                        foreign_occurrence_payload,
                    ))],
                )))],
            ),
            contents: bcs::to_bytes(&DistributedWrapper {
                event: scheduled_occurrence,
                deadline_ms: 25,
                requested_at_ms: 26,
                task_id: addr(0x47),
                leaders: vec![addr(0x48), addr(0x49)],
            })
            .unwrap(),
        };

        let err = NexusEvent::from_sui_grpc_event(
            1,
            sui::types::Digest::ZERO,
            &occurrence_event,
            &objects,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("occurrence payload does not come from Nexus scheduler"));
    }

    #[test]
    fn from_sui_grpc_event_allows_foreign_emitter_for_known_extension_events() {
        let objects = sui_mocks::mock_nexus_objects();
        let execution = id(addr(0xa5));
        let event = wrapped_event(
            &objects,
            addr(0xf2),
            inner_tag(
                objects.workflow_pkg_id,
                "execution_events",
                "RequestWalkExecutionEvent",
                vec![],
            ),
            RequestWalkExecutionEvent {
                dag: id(addr(0xa6)),
                execution: execution.clone(),
                invoker: addr(0xa7),
                walk_index: 8,
                next_vertex: RuntimeVertex::plain("foreign_emitter"),
                evaluations: id(addr(0xa8)),
                agent_id: id(addr(0xa9)),
                skill_id: 10,
                interface_version: interface_version(11),
                scheduled_task_id: MoveOption(None),
                scheduled_occurrence_index: MoveOption(None),
            },
        );

        let parsed =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::RequestWalkExecution(RequestWalkExecutionEvent { execution: parsed_execution, .. })
                if parsed_execution == execution
        ));
    }

    #[test]
    fn from_sui_grpc_event_rejects_foreign_emitter_for_regular_events() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_event(
            &objects,
            addr(0xf3),
            inner_tag(objects.interface_pkg_id, "dag", "DAGCreatedEvent", vec![]),
            DAGCreatedEvent {
                dag: id(addr(0xaa)),
            },
        );

        let err = NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects)
            .unwrap_err();

        assert!(err
            .to_string()
            .contains("Event does not come from a Nexus package"));
    }

    #[test]
    fn direct_event_wrappers_parse_for_every_exposed_event_kind() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut parsed = 0usize;

        macro_rules! check {
            (@tag $package:expr, $module:literal, $name:expr) => {
                inner_tag($package, $module, $name, vec![])
            };
            (@scheduled_occurrence_tag) => {{
                let occurrence_tag = inner_tag(
                    objects.scheduler_pkg_id,
                    "scheduler",
                    "OccurrenceScheduledEvent",
                    vec![],
                );
                inner_tag(
                    objects.interface_pkg_id,
                    "scheduled_request",
                    "RequestScheduledExecution",
                    vec![sui::types::TypeTag::Struct(Box::new(occurrence_tag))],
                )
            }};
            ($name:expr, $tag:expr, $event:expr) => {{
                assert_move_event_parses(&objects, $name, $tag, $event, $name, parsed as u64);
                parsed += 1;
            }};
        }

        check!(
            "RequestScheduledOccurrenceEvent",
            check!(@scheduled_occurrence_tag),
            RequestScheduledOccurrenceEvent {
                request: OccurrenceScheduledEvent {
                    task: id(addr(0x01)),
                    generator: generator(),
                },
                priority: 1,
                request_ms: 2,
                start_ms: 3,
                deadline_ms: 4,
            }
        );
        check!(
            "OccurrenceScheduledEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "OccurrenceScheduledEvent"),
            OccurrenceScheduledEvent {
                task: id(addr(0x02)),
                generator: generator(),
            }
        );
        check!(
            "RequestWalkExecutionEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "RequestWalkExecutionEvent"),
            RequestWalkExecutionEvent {
                dag: id(addr(0x03)),
                execution: id(addr(0x04)),
                invoker: addr(0x05),
                walk_index: 6,
                next_vertex: RuntimeVertex::plain("entry"),
                evaluations: id(addr(0x07)),
                agent_id: id(addr(0x08)),
                skill_id: 9,
                interface_version: interface_version(10),
                scheduled_task_id: MoveOption(Some(id(addr(0x0b)))),
                scheduled_occurrence_index: MoveOption(Some(12)),
            }
        );
        check!(
            "AgentCreatedEvent",
            check!(@tag objects.interface_pkg_id, "agent", "AgentCreatedEvent"),
            AgentCreatedEvent {
                agent_id: id(addr(0x0d)),
                vault_id: addr(0x0e),
            }
        );
        check!(
            "SkillRegisteredEvent",
            check!(@tag objects.registry_pkg_id, "agent_registry", "SkillRegisteredEvent"),
            SkillRegisteredEvent {
                agent_id: id(addr(0x0f)),
                skill_id: 16,
                dag_id: addr(0x10),
                dag_binding: skill_dag_binding(),
            }
        );
        check!(
            "SkillContractRevisionedEvent",
            check!(@tag objects.registry_pkg_id, "agent_registry", "SkillContractRevisionedEvent"),
            SkillContractRevisionedEvent {
                agent_id: id(addr(0x11)),
                skill_id: 18,
                current_interface_revision: interface_version(19),
                dag_binding: skill_dag_binding(),
                requirements: skill_requirements(),
            }
        );
        check!(
            "DefaultDagExecutorUpdatedEvent",
            check!(@tag objects.registry_pkg_id, "agent_registry", "DefaultDagExecutorUpdatedEvent"),
            DefaultDagExecutorUpdatedEvent {
                agent_id: id(addr(0x12)),
                skill_id: 20,
            }
        );
        check!(
            "AgentSkillExecutionRequestedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "AgentSkillExecutionRequestedEvent"),
            AgentSkillExecutionRequestedEvent {
                execution_id: addr(0x13),
                agent_id: id(addr(0x14)),
                skill_id: 21,
                interface_revision: interface_version(22),
                payment_id: addr(0x15),
            }
        );
        check!(
            "AgentVertexAuthorizationRequiredEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "AgentVertexAuthorizationRequiredEvent"),
            AgentVertexAuthorizationRequiredEvent {
                dag: id(addr(0x16)),
                execution: id(addr(0x17)),
                walk_index: 23,
                vertex: RuntimeVertex::plain("auth"),
                tool_fqn: MoveString::from("demo::tool::run"),
                agent_id: MoveOption(Some(id(addr(0x18)))),
                skill_id: MoveOption(Some(24)),
            }
        );
        check!(
            "AgentSkillPaymentCreatedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "AgentSkillPaymentCreatedEvent"),
            AgentSkillPaymentCreatedEvent {
                payment_id: addr(0x19),
                execution_id: addr(0x1a),
                agent_id: id(addr(0x1b)),
                skill_id: 25,
                interface_revision: interface_version(26),
                payment_policy: payment_types::SkillPaymentPolicy::UserFunded,
                source_kind: payment_source(),
                max_budget: 27,
                locked_budget: 28,
            }
        );
        check!(
            "ExecutionPaymentReceiptCreatedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionPaymentReceiptCreatedEvent"),
            ExecutionPaymentReceiptCreatedEvent {
                receipt_id: addr(0x1c),
                execution_id: addr(0x1d),
                payment_id: addr(0x1e),
                agent_id: id(addr(0x1f)),
                skill_id: 29,
                source_kind: payment_source(),
                stored_under_agent: true,
            }
        );
        check!(
            "ExecutionPaymentReceiptResolvedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionPaymentReceiptResolvedEvent"),
            ExecutionPaymentReceiptResolvedEvent {
                receipt_id: addr(0x20),
                execution_id: addr(0x21),
                payment_id: addr(0x22),
                agent_id: id(addr(0x23)),
                final_state: payment_types::ExecutionPaymentFinalState::Accomplished,
            }
        );
        check!(
            "ScheduledPaymentReserveReceiptCreatedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ScheduledPaymentReserveReceiptCreatedEvent"),
            ScheduledPaymentReserveReceiptCreatedEvent {
                receipt_id: addr(0x24),
                scheduled_task_id: addr(0x25),
                reserve_id: addr(0x26),
                agent_id: id(addr(0x27)),
                skill_id: 30,
                interface_version: interface_version(31),
                source_kind: payment_source(),
                prepaid_amount: 32,
                occurrence_budget: 33,
                stored_under_agent: false,
            }
        );
        check!(
            "GasPaymentConsumedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "GasPaymentConsumedEvent"),
            GasPaymentConsumedEvent {
                payment_id: addr(0x28),
                execution_id: addr(0x29),
                agent_id: id(addr(0x2a)),
                skill_id: 34,
                interface_revision: interface_version(35),
                amount: 36,
                consumed_total: 37,
            }
        );
        check!(
            "ExecutionAccomplishedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionAccomplishedEvent"),
            ExecutionAccomplishedEvent {
                execution_id: addr(0x2b),
                payment_id: addr(0x2c),
                agent_id: id(addr(0x2d)),
                skill_id: 38,
                interface_revision: interface_version(39),
            }
        );
        check!(
            "ExecutionRefundedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionRefundedEvent"),
            ExecutionRefundedEvent {
                execution_id: addr(0x2e),
                payment_id: addr(0x2f),
                agent_id: id(addr(0x30)),
                skill_id: 40,
                interface_revision: interface_version(41),
                refund_reason: b"refund".to_vec(),
            }
        );
        check!(
            "ScheduledSkillExecutionCreatedEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "ScheduledSkillExecutionCreatedEvent"),
            ScheduledSkillExecutionCreatedEvent {
                task: id(addr(0x31)),
                owner: addr(0x32),
            }
        );
        check!(
            "ScheduledSkillExecutionPausedEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "ScheduledSkillExecutionPausedEvent"),
            ScheduledSkillExecutionPausedEvent {
                task: id(addr(0x33)),
            }
        );
        check!(
            "ScheduledSkillExecutionResumedEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "ScheduledSkillExecutionResumedEvent"),
            ScheduledSkillExecutionResumedEvent {
                task: id(addr(0x34)),
            }
        );
        check!(
            "ScheduledSkillExecutionCanceledEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "ScheduledSkillExecutionCanceledEvent"),
            ScheduledSkillExecutionCanceledEvent {
                task: id(addr(0x35)),
            }
        );
        check!(
            "ScheduledSkillPaymentRefilledEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ScheduledSkillPaymentRefilledEvent"),
            ScheduledSkillPaymentRefilledEvent {
                scheduled_task_id: addr(0x36),
                reserve_id: addr(0x37),
                agent_id: id(addr(0x38)),
                skill_id: 42,
                interface_version: interface_version(43),
                source_kind: payment_source(),
                refill_amount: 44,
                occurrence_budget: 45,
                remaining_funds: 46,
            }
        );
        check!(
            "ScheduledOccurrencePaymentCreatedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ScheduledOccurrencePaymentCreatedEvent"),
            ScheduledOccurrencePaymentCreatedEvent {
                scheduled_task_id: addr(0x39),
                reserve_id: addr(0x3a),
                occurrence_index: 47,
                execution_id: addr(0x3b),
                payment_id: addr(0x3c),
                agent_id: id(addr(0x3d)),
                skill_id: 48,
                interface_version: interface_version(49),
                source_kind: payment_source(),
                budget: 50,
                remaining_funds: 51,
            }
        );
        check!(
            "ScheduledSkillPaymentCanceledEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ScheduledSkillPaymentCanceledEvent"),
            ScheduledSkillPaymentCanceledEvent {
                scheduled_task_id: addr(0x3e),
                reserve_id: addr(0x3f),
                agent_id: id(addr(0x40)),
                skill_id: 52,
                interface_version: interface_version(53),
                source_kind: payment_source(),
                refunded_amount: 54,
                remaining_funds: 55,
            }
        );
        check!(
            "ScheduledOccurrencePaymentFinalizedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ScheduledOccurrencePaymentFinalizedEvent"),
            ScheduledOccurrencePaymentFinalizedEvent {
                scheduled_task_id: addr(0x41),
                reserve_id: addr(0x42),
                occurrence_index: 56,
                execution_id: addr(0x43),
                payment_id: addr(0x44),
                agent_id: id(addr(0x45)),
                skill_id: 57,
                interface_version: interface_version(58),
                final_state: payment_types::ScheduledOccurrenceFinalState::Accomplished,
                remaining_funds: 59,
            }
        );
        check!(
            "ToolRegisteredEvent",
            check!(@tag objects.registry_pkg_id, "tool_registry", "ToolRegisteredEvent"),
            ToolRegisteredEvent {
                tool: id(addr(0x46)),
                fqn: MoveString::from("demo::tool::registered"),
            }
        );
        check!(
            "ToolUnregisteredEvent",
            check!(@tag objects.registry_pkg_id, "tool_registry", "ToolUnregisteredEvent"),
            ToolUnregisteredEvent {
                tool: id(addr(0x47)),
                fqn: MoveString::from("demo::tool::unregistered"),
            }
        );
        check!(
            "CommittedToolResultEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "CommittedToolResultEvent"),
            CommittedToolResultEvent {
                dag: id(addr(0x48)),
                execution: id(addr(0x49)),
                walk_index: 60,
                vertex: RuntimeVertex::plain("commit"),
                leader: id(addr(0x4a)),
                has_primary_failure_evidence: false,
                has_secondary_failure_evidence: true,
            }
        );
        check!(
            "WalkAdvancedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "WalkAdvancedEvent"),
            WalkAdvancedEvent {
                dag: id(addr(0x4b)),
                execution: id(addr(0x4c)),
                walk_index: 61,
                vertex: RuntimeVertex::plain("advanced"),
                variant: crate::types::interface::graph::OutputVariant {
                    name: MoveString::from("ok"),
                },
                variant_ports_to_data: empty_ports_data(),
            }
        );
        check!(
            "WalkFailedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "WalkFailedEvent"),
            WalkFailedEvent {
                dag: id(addr(0x4d)),
                execution: id(addr(0x4e)),
                walk_index: 62,
                vertex: RuntimeVertex::plain("failed"),
                reason: MoveString::from("failed"),
            }
        );
        check!(
            "TerminalErrEvalRecordedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "TerminalErrEvalRecordedEvent"),
            TerminalErrEvalRecordedEvent {
                dag: id(addr(0x4f)),
                execution: id(addr(0x50)),
                walk_index: 63,
                vertex: RuntimeVertex::plain("terminal"),
                leader: addr(0x51),
                failure_class: WorkflowFailureClass::TerminalToolFailure,
                outcome: MoveOption(Some(PostFailureAction::Terminate)),
                reason: MoveString::from("terminal"),
                err_eval_hash: vec![1, 2, 3],
                duplicate: false,
            }
        );
        check!(
            "VerificationVerdictEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "VerificationVerdictEvent"),
            VerificationVerdictEvent {
                dag: id(addr(0x52)),
                execution: id(addr(0x53)),
                walk_index: 64,
                vertex: RuntimeVertex::plain("verified"),
                leader: addr(0x54),
                submission_kind:
                    crate::types::interface::verifier::VerificationSubmissionKind::Success,
                failure_evidence_kind:
                    crate::types::interface::verifier::FailureEvidenceKind::ToolEvidence,
                leader_verifier_mode:
                    crate::types::interface::verifier::VerifierMode::LeaderRegisteredKey,
                leader_verifier_method: MoveString::from("leader"),
                tool_verifier_mode:
                    crate::types::interface::verifier::VerifierMode::ToolVerifierContract,
                tool_verifier_method: MoveString::from("tool"),
                checked_leader_kid: MoveOption(Some(65)),
                checked_tool_kid: MoveOption(Some(66)),
                payload_or_reason_hash: vec![4, 5],
                checked_identity: vec![6, 7],
                verdict_reference: vec![8, 9],
                verdict: crate::types::interface::verifier::VerificationVerdict::Accepted,
            }
        );
        check!(
            "WalkAbortedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "WalkAbortedEvent"),
            WalkAbortedEvent {
                dag: id(addr(0x55)),
                execution: id(addr(0x56)),
                walk_index: 67,
                vertex: RuntimeVertex::plain("aborted"),
            }
        );
        check!(
            "WalkCancelledEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "WalkCancelledEvent"),
            WalkCancelledEvent {
                dag: id(addr(0x57)),
                execution: id(addr(0x58)),
                walk_index: 68,
                vertex: RuntimeVertex::plain("cancelled"),
            }
        );
        check!(
            "EndStateReachedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "EndStateReachedEvent"),
            EndStateReachedEvent {
                dag: id(addr(0x59)),
                execution: id(addr(0x5a)),
                walk_index: 69,
                vertex: RuntimeVertex::plain("end"),
                variant: crate::types::interface::graph::OutputVariant {
                    name: MoveString::from("ok"),
                },
                variant_ports_to_data: MoveVecMap {
                    contents: vec![VecMapEntry {
                        key: crate::types::interface::graph::OutputPort {
                            name: MoveString::from("answer"),
                        },
                        value: crate::types::primitives::data::NexusData {
                            storage: b"inline".to_vec(),
                            one: serde_json::to_vec(&json!(42)).unwrap(),
                            many: vec![],
                        },
                    }],
                },
            }
        );
        check!(
            "ExecutionFinishedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "ExecutionFinishedEvent"),
            ExecutionFinishedEvent {
                dag: id(addr(0x5b)),
                execution: id(addr(0x5c)),
                has_any_walk_failed: false,
                has_any_walk_succeeded: true,
                was_aborted: false,
            }
        );
        check!(
            "MissedOccurrenceEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "MissedOccurrenceEvent"),
            MissedOccurrenceEvent {
                task: id(addr(0x5d)),
                start_time_ms: 70,
                deadline_ms: MoveOption(Some(71)),
                pruned_at: 72,
                priority_fee_per_gas_unit: 73,
                generator: generator(),
            }
        );
        check!(
            "OccurrenceConsumedEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "OccurrenceConsumedEvent"),
            OccurrenceConsumedEvent {
                task: id(addr(0x5e)),
                start_time_ms: 74,
                deadline_ms: MoveOption(None),
                priority_fee_per_gas_unit: 75,
                generator: generator(),
                executed_at: 76,
            }
        );
        check!(
            "PeriodicScheduleConfiguredEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "PeriodicScheduleConfiguredEvent"),
            PeriodicScheduleConfiguredEvent {
                task: id(addr(0x5f)),
                period_ms: MoveOption(Some(77)),
                deadline_offset_ms: MoveOption(Some(78)),
                max_iterations: MoveOption(Some(79)),
                generated: MoveOption(Some(80)),
                priority_fee_per_gas_unit: 81,
                last_generated_start_ms: MoveOption(Some(82)),
            }
        );
        check!(
            "FoundingLeaderCapCreatedEvent",
            check!(@tag objects.registry_pkg_id, "leader_cap", "FoundingLeaderCapCreatedEvent"),
            FoundingLeaderCapCreatedEvent {
                leader_cap: id(addr(0x60)),
                network: id(addr(0x61)),
            }
        );
        check!(
            "LeaderCapIssuedEvent",
            check!(@tag objects.registry_pkg_id, "leader", "LeaderCapIssuedEvent"),
            LeaderCapIssuedEvent {
                registry: id(addr(0x62)),
                leader_cap_id: id(addr(0x63)),
                network: id(addr(0x64)),
                leader: addr(0x65),
            }
        );
        check!(
            "LeaderClaimedEvent",
            check!(@tag objects.registry_pkg_id, "leader", "LeaderClaimedEvent"),
            LeaderClaimedEvent {
                registry: id(addr(0x66)),
                leader_cap_id: id(addr(0x67)),
                claim_token: b"claim".to_vec(),
            }
        );
        check!(
            "PaymentInsufficientGasEvent",
            check!(@tag objects.workflow_pkg_id, "gas", "PaymentInsufficientGasEvent"),
            PaymentInsufficientGasEvent {
                execution: id(addr(0x68)),
                vertex: RuntimeVertex::plain("gas"),
                tool_fqn: MoveString::from("demo::tool::gas"),
                required_tool_fee: 83,
                available_gas: 84,
            }
        );
        check!(
            "PaymentLockUpdateEvent",
            check!(@tag objects.workflow_pkg_id, "gas", "PaymentLockUpdateEvent"),
            PaymentLockUpdateEvent {
                execution: id(addr(0x69)),
                vertex: RuntimeVertex::plain("lock"),
                tool_fqn: MoveString::from("demo::tool::lock"),
                was_locked: true,
            }
        );
        check!(
            "PaymentUnlockUpdateEvent",
            check!(@tag objects.workflow_pkg_id, "gas", "PaymentUnlockUpdateEvent"),
            PaymentUnlockUpdateEvent {
                execution: id(addr(0x6a)),
                vertex: RuntimeVertex::plain("unlock"),
                tool_fqn: MoveString::from("demo::tool::unlock"),
                was_refunded: false,
            }
        );
        check!(
            "DAGCreatedEvent",
            check!(@tag objects.interface_pkg_id, "dag", "DAGCreatedEvent"),
            DAGCreatedEvent {
                dag: id(addr(0x6b))
            }
        );
        check!(
            "ToolRegistryCreatedEvent",
            check!(@tag objects.registry_pkg_id, "tool_registry", "ToolRegistryCreatedEvent"),
            ToolRegistryCreatedEvent {
                registry: id(addr(0x6c)),
            }
        );

        assert_eq!(parsed, 46);
    }
}
