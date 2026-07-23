//! This module defines transformers from various Sui types to Nexus event types.
//! Namely we support:
//! - Parsing GRPC [`sui::types::Event`]

use {
    crate::{
        events::{parse_bcs, supports_event, NexusEvent},
        move_bindings::primitives::{
            data::NexusData as MoveNexusData,
            distributed_event as distributed_event_move,
            event as event_move,
        },
        sui,
        types::NexusObjects,
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

pub(super) enum NexusEventClassification<'a> {
    Decode {
        event_type: &'a sui::types::StructTag,
        event_name: String,
    },
    Ignore(String),
}

impl FromSuiGrpcEvent for NexusEvent {
    fn from_sui_grpc_event(
        index: u64,
        digest: sui::types::Digest,
        event: &sui::types::Event,
        objects: &NexusObjects,
    ) -> anyhow::Result<NexusEvent> {
        match classify_nexus_event(&event.type_, objects)? {
            NexusEventClassification::Decode {
                event_type,
                event_name,
            } => decode_nexus_event(index, digest, &event.contents, event_type, &event_name),
            NexusEventClassification::Ignore(reason) => Err(anyhow::Error::msg(reason)),
        }
    }
}

pub(super) fn classify_nexus_event<'a>(
    wrapper_type: &'a sui::types::StructTag,
    objects: &NexusObjects,
) -> anyhow::Result<NexusEventClassification<'a>> {
    if !is_event_wrapper(wrapper_type, objects) {
        return Ok(NexusEventClassification::Ignore(format!(
            "Event is not wrapped in '{}::event::EventWrapper', found type: \
             '{:?}'",
            objects.primitives_pkg_id, wrapper_type
        )));
    }

    let Some(event_type) = wrapper_type
        .type_params()
        .first()
        .and_then(|tag| match tag {
            sui::types::TypeTag::Struct(struct_tag) => Some(struct_tag),
            _ => None,
        })
    else {
        bail!("EventWrapper does not have a valid event type parameter");
    };

    if !is_nexus_package(*event_type.address(), objects) {
        return Ok(NexusEventClassification::Ignore(format!(
            "Inner event does not come from a Nexus package, it comes from \
             '{}' instead",
            event_type.address()
        )));
    }

    let event_name = normalize_event_name(event_type, objects)?;
    if !supports_event(&event_name) {
        return Ok(NexusEventClassification::Ignore(format!(
            "Unknown event: {event_name}"
        )));
    }

    Ok(NexusEventClassification::Decode {
        event_type,
        event_name,
    })
}

pub(super) fn decode_nexus_event(
    index: u64,
    digest: sui::types::Digest,
    contents: &[u8],
    event_type: &sui::types::StructTag,
    event_name: &str,
) -> anyhow::Result<NexusEvent> {
    let (data, distribution) = parse_bcs(event_name, contents)?;

    Ok(NexusEvent {
        id: (digest, index),
        generics: event_type.type_params().to_vec(),
        data,
        distribution,
    })
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

/// Helper function to determine whether the given address is one of the Nexus
/// package addresses.
fn is_nexus_package(address: sui::types::Address, objects: &NexusObjects) -> bool {
    address == objects.primitives_pkg_id
        || address == objects.interface_pkg_id
        || address == objects.registry_pkg_id
        || objects.is_scheduler_package(address)
        || objects.is_workflow_package(address)
}

/// Helper function to determine whether the provided struct tag corresponds to
/// `nexus_primitives::event::EventWrapper`.
pub(crate) fn is_event_wrapper(tag: &sui::types::StructTag, objects: &NexusObjects) -> bool {
    crate::move_bindings::struct_tag_matches::<event_move::EventWrapper<MoveNexusData>>(
        objects, tag,
    ) || crate::move_bindings::struct_tag_matches::<
        distributed_event_move::DistributedEventWrapper<MoveNexusData>,
    >(objects, tag)
}

#[cfg(all(test, feature = "test_utils"))]
mod direct_event_tests {
    use {
        super::*,
        crate::{
            events::{parse_bcs, NexusEventKind, NexusEventQuery},
            move_bindings::{
                interface::{
                    agent::{self as agent_types, *},
                    dag::*,
                    graph::{PostFailureAction, RuntimeVertex},
                    payment::{self as payment_types, *},
                    scheduled_request,
                    version,
                },
                move_std::{
                    ascii::String as MoveString,
                    option::Option as MoveOption,
                    type_name::TypeName,
                },
                primitives::policy::Symbol as PolicySymbol,
                registry::{
                    agent_registry::*,
                    leader::*,
                    leader_cap::*,
                    priority_fee_vault::*,
                    tool_registry::*,
                },
                scheduler::scheduler::*,
                sui_framework::{
                    object::ID,
                    vec_map::{Entry as VecMapEntry, VecMap as MoveVecMap},
                },
                workflow::{execution_events::*, execution_failure::WorkflowFailureClass, gas::*},
            },
            sui::{self, events::EventQuery as _},
            test_utils::sui_mocks,
        },
        serde::Serialize,
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
        crate::move_bindings::interface::graph::OutputPort,
        crate::move_bindings::primitives::data::NexusData,
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

        let caller_package = *inner.address();
        let type_ = wrapper_tag(objects, inner);
        let wrapped = sui::types::Event {
            package_id: caller_package,
            module: type_.module().clone(),
            sender: addr(0xee),
            type_,
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

    fn wrapper_tag(
        objects: &crate::types::NexusObjects,
        inner: sui::types::StructTag,
    ) -> sui::types::StructTag {
        let wrapper =
            crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(objects);
        sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        )
    }

    fn distributed_wrapper_tag(
        objects: &crate::types::NexusObjects,
        inner: sui::types::StructTag,
    ) -> sui::types::StructTag {
        let wrapper = crate::move_bindings::struct_tag::<
            distributed_event_move::DistributedEventWrapper<MoveNexusData>,
        >(objects);
        sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        )
    }

    fn wrapped_event<T: Serialize>(
        objects: &crate::types::NexusObjects,
        caller_package: sui::types::Address,
        inner: sui::types::StructTag,
        event: T,
    ) -> sui::types::Event {
        let type_ = wrapper_tag(objects, inner);
        sui::types::Event {
            package_id: caller_package,
            module: type_.module().clone(),
            sender: addr(0xee),
            type_,
            contents: bcs::to_bytes(&Wrapper { event }).unwrap(),
        }
    }

    #[test]
    fn nexus_event_query_decodes_a_known_wrapper_event() {
        let objects = std::sync::Arc::new(sui_mocks::mock_nexus_objects());
        let dag = id(addr(0xd1));
        let inner = inner_tag(objects.interface_pkg_id, "dag", "DAGCreatedEvent", vec![]);
        let event = wrapped_event(
            &objects,
            objects.interface_pkg_id,
            inner,
            DAGCreatedEvent { dag },
        );
        let mut event = sui::grpc::Event::from(event);
        event.set_checkpoint(7);
        event.set_transaction_digest(sui::types::Digest::ZERO);
        event.set_event_index(4);

        let query = NexusEventQuery::new(std::sync::Arc::clone(&objects));
        let decoded = query.decode(event).unwrap().unwrap();

        assert_eq!(decoded.id, (sui::types::Digest::ZERO, 4));
        assert!(matches!(
            decoded.data,
            NexusEventKind::DAGCreated(DAGCreatedEvent { dag: parsed })
                if parsed == dag
        ));
        let read_mask = query.read_mask();
        assert_eq!(read_mask.paths.len(), 2);
        for path in ["event_type", "contents"] {
            assert!(read_mask.paths.iter().any(|candidate| candidate == path));
        }
        assert_eq!(query.filter().terms.len(), 2);
    }

    #[test]
    fn nexus_event_query_ignores_an_unknown_nexus_event() {
        let objects = std::sync::Arc::new(sui_mocks::mock_nexus_objects());
        let event = wrapped_event(
            &objects,
            objects.interface_pkg_id,
            inner_tag(objects.interface_pkg_id, "dag", "FutureEvent", vec![]),
            vec![1_u8, 2, 3],
        );
        let mut event = sui::grpc::Event::from(event);
        event.set_checkpoint(7);
        event.set_transaction_digest(sui::types::Digest::ZERO);
        event.set_event_index(4);

        let query = NexusEventQuery::new(std::sync::Arc::clone(&objects));

        assert!(query.decode(event).unwrap().is_none());
    }

    #[test]
    fn nexus_event_query_rejects_invalid_known_event_contents() {
        let objects = std::sync::Arc::new(sui_mocks::mock_nexus_objects());
        let inner = inner_tag(objects.interface_pkg_id, "dag", "DAGCreatedEvent", vec![]);
        let mut event = wrapped_event(
            &objects,
            objects.interface_pkg_id,
            inner,
            DAGCreatedEvent {
                dag: id(addr(0xd1)),
            },
        );
        event.contents = vec![0xff];
        let mut event = sui::grpc::Event::from(event);
        event.set_checkpoint(7);
        event.set_transaction_digest(sui::types::Digest::ZERO);
        event.set_event_index(4);

        let query = NexusEventQuery::new(std::sync::Arc::clone(&objects));

        assert!(query.decode(event).is_err());
    }

    #[test]
    fn parse_bcs_uses_direct_dag_created_event() {
        let dag = id(sui::types::Address::from_static("0xabc"));
        let bytes = bcs::to_bytes(&Wrapper {
            event: DAGCreatedEvent { dag },
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
                agent_id,
                skill_id: 2,
                interface_version: version::InterfaceVersion { inner: 3 },
                scheduled_task_id: MoveOption::from_option(None),
                scheduled_occurrence_index: MoveOption::from_option(None),
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
            execution,
            invoker: addr(0xb2),
            walk_index: 12,
            next_vertex: RuntimeVertex::plain("scheduled_walk"),
            evaluations: id(addr(0xb3)),
            agent_id: id(addr(0xb4)),
            skill_id: 13,
            interface_version: interface_version(14),
            scheduled_task_id: MoveOption::from_option(None),
            scheduled_occurrence_index: MoveOption::from_option(None),
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
        let type_ = distributed_wrapper_tag(&objects, inner);
        let event = sui::types::Event {
            package_id: objects.workflow_pkg_id,
            module: type_.module().clone(),
            sender: addr(0xee),
            type_,
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
                scheduled_task_id: MoveOption::from_option(None),
                scheduled_occurrence_index: MoveOption::from_option(None),
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
        let walk_type = distributed_wrapper_tag(
            &objects,
            inner_tag(
                objects.interface_pkg_id,
                "scheduled_request",
                "RequestScheduledExecution",
                vec![sui::types::TypeTag::Struct(Box::new(foreign_walk_payload))],
            ),
        );
        let walk_event = sui::types::Event {
            package_id: addr(0xf5),
            module: walk_type.module().clone(),
            sender: addr(0xee),
            type_: walk_type,
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
        let occurrence_type = distributed_wrapper_tag(
            &objects,
            inner_tag(
                objects.interface_pkg_id,
                "scheduled_request",
                "RequestScheduledExecution",
                vec![sui::types::TypeTag::Struct(Box::new(
                    foreign_occurrence_payload,
                ))],
            ),
        );
        let occurrence_event = sui::types::Event {
            package_id: addr(0xf7),
            module: occurrence_type.module().clone(),
            sender: addr(0xee),
            type_: occurrence_type,
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
    fn from_sui_grpc_event_allows_foreign_caller_for_known_extension_events() {
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
                execution,
                invoker: addr(0xa7),
                walk_index: 8,
                next_vertex: RuntimeVertex::plain("foreign_emitter"),
                evaluations: id(addr(0xa8)),
                agent_id: id(addr(0xa9)),
                skill_id: 10,
                interface_version: interface_version(11),
                scheduled_task_id: MoveOption::from_option(None),
                scheduled_occurrence_index: MoveOption::from_option(None),
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
    fn from_sui_grpc_event_allows_foreign_caller_for_skill_registered() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_event(
            &objects,
            addr(0xf3),
            inner_tag(
                objects.registry_pkg_id,
                "agent_registry",
                "SkillRegisteredEvent",
                vec![],
            ),
            SkillRegisteredEvent {
                agent_id: id(addr(0xaa)),
                skill_id: 11,
                dag_id: addr(0xab),
                dag_binding: skill_dag_binding(),
            },
        );

        let parsed =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::SkillRegistered(SkillRegisteredEvent { skill_id: 11, .. })
        ));
    }

    #[test]
    fn from_sui_grpc_event_allows_foreign_caller_for_skill_contract_revisioned() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_event(
            &objects,
            addr(0xf4),
            inner_tag(
                objects.registry_pkg_id,
                "agent_registry",
                "SkillContractRevisionedEvent",
                vec![],
            ),
            SkillContractRevisionedEvent {
                agent_id: id(addr(0xac)),
                skill_id: 12,
                current_interface_revision: interface_version(13),
                dag_binding: skill_dag_binding(),
                requirements: skill_requirements(),
            },
        );

        let parsed =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::SkillContractRevisioned(SkillContractRevisionedEvent {
                skill_id: 12,
                ..
            })
        ));
    }

    #[test]
    fn from_sui_grpc_event_allows_foreign_caller_for_execution_payment_receipt_created() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_event(
            &objects,
            addr(0xf5),
            inner_tag(
                objects.interface_pkg_id,
                "payment",
                "ExecutionPaymentReceiptCreatedEvent",
                vec![],
            ),
            ExecutionPaymentReceiptCreatedEvent {
                receipt_id: addr(0xad),
                execution_id: addr(0xae),
                payment_id: addr(0xaf),
                agent_id: id(addr(0xb0)),
                skill_id: 14,
                source_kind: payment_source(),
                stored_under_agent: true,
            },
        );

        let parsed =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::ExecutionPaymentReceiptCreated(ExecutionPaymentReceiptCreatedEvent {
                skill_id: 14,
                ..
            })
        ));
    }

    #[test]
    fn from_sui_grpc_event_allows_foreign_caller_for_execution_payment_tool_cost_snapshotted() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_event(
            &objects,
            addr(0xf6),
            inner_tag(
                objects.interface_pkg_id,
                "payment",
                "ExecutionPaymentToolCostSnapshottedEvent",
                vec![],
            ),
            ExecutionPaymentToolCostSnapshottedEvent {
                payment_id: addr(0xb1),
                execution_id: addr(0xb2),
                agent_id: id(addr(0xb3)),
                tool_fqn: b"demo::tool".to_vec(),
                cost: 15,
            },
        );

        let parsed =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::ExecutionPaymentToolCostSnapshotted(
                ExecutionPaymentToolCostSnapshottedEvent { cost: 15, .. }
            )
        ));
    }

    #[test]
    fn from_sui_grpc_event_allows_foreign_caller_for_execution_payment_vertex_locked() {
        let objects = sui_mocks::mock_nexus_objects();
        let event = wrapped_event(
            &objects,
            addr(0xf7),
            inner_tag(
                objects.interface_pkg_id,
                "payment",
                "ExecutionPaymentVertexLockedEvent",
                vec![],
            ),
            ExecutionPaymentVertexLockedEvent {
                payment_id: addr(0xb4),
                execution_id: addr(0xb5),
                agent_id: id(addr(0xb6)),
                vertex_key: b"vertex".to_vec(),
                tool_fqn: b"demo::tool".to_vec(),
                amount: 16,
                settlement_kind: payment_types::VertexExecutionPaymentSettlementKind::Paid,
            },
        );

        let parsed =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::ExecutionPaymentVertexLocked(ExecutionPaymentVertexLockedEvent {
                amount: 16,
                ..
            })
        ));
    }

    #[test]
    fn from_sui_grpc_event_trusts_the_inner_event_type() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = id(addr(0xaa));
        let event = wrapped_event(
            &objects,
            addr(0xf3),
            inner_tag(objects.interface_pkg_id, "dag", "DAGCreatedEvent", vec![]),
            DAGCreatedEvent { dag },
        );

        let parsed =
            NexusEvent::from_sui_grpc_event(0, sui::types::Digest::ZERO, &event, &objects).unwrap();

        assert!(matches!(
            parsed.data,
            NexusEventKind::DAGCreated(DAGCreatedEvent { dag: parsed })
                if parsed == dag
        ));
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
                scheduled_task_id: MoveOption::from_option(Some(id(addr(0x0b)))),
                scheduled_occurrence_index: MoveOption::from_option(Some(12)),
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
                agent_id: MoveOption::from_option(Some(id(addr(0x18)))),
                skill_id: MoveOption::from_option(Some(24)),
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
                max_budget_mist: 27,
                gas_budget_mist: 23,
                priority_fee_reserve_mist: 4,
                locked_budget_mist: 28,
                priority_fee_percentage: 20,
            }
        );
        check!(
            "ExecutionPaymentFeesRecordedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionPaymentFeesRecordedEvent"),
            ExecutionPaymentFeesRecordedEvent {
                payment_id: addr(0xc7),
                execution_id: addr(0xc8),
                agent_id: id(addr(0xc9)),
                skill_id: 44,
                gas_fee_mist: 45,
                tool_fee_mist: 46,
                priority_fee_mist: 47,
                priority_fee_percentage: 20,
            }
        );
        check!(
            "ExecutionPaymentToolCostSnapshottedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionPaymentToolCostSnapshottedEvent"),
            ExecutionPaymentToolCostSnapshottedEvent {
                payment_id: addr(0xc1),
                execution_id: addr(0xc2),
                agent_id: id(addr(0xc3)),
                tool_fqn: b"demo::tool".to_vec(),
                cost: 42,
            }
        );
        check!(
            "ExecutionPaymentVertexLockedEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionPaymentVertexLockedEvent"),
            ExecutionPaymentVertexLockedEvent {
                payment_id: addr(0xca),
                execution_id: addr(0xcb),
                agent_id: id(addr(0xcc)),
                vertex_key: b"vertex".to_vec(),
                tool_fqn: b"demo::tool".to_vec(),
                amount: 48,
                settlement_kind: payment_types::VertexExecutionPaymentSettlementKind::Ticket,
            }
        );
        check!(
            "ExecutionPaymentVertexSettledEvent",
            check!(@tag objects.interface_pkg_id, "payment", "ExecutionPaymentVertexSettledEvent"),
            ExecutionPaymentVertexSettledEvent {
                payment_id: addr(0xc4),
                execution_id: addr(0xc5),
                agent_id: id(addr(0xc6)),
                vertex_key: b"vertex".to_vec(),
                tool_fqn: b"demo::tool".to_vec(),
                amount: 43,
                settlement_kind: payment_types::VertexExecutionPaymentSettlementKind::Paid,
                was_refunded: false,
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
                prepaid_amount_mist: 32,
                occurrence_budget_mist: 33,
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
                occurrence_budget_mist: 45,
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
                variant: crate::move_bindings::interface::graph::OutputVariant {
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
            "SubmissionFailureEvidenceRecordedEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "SubmissionFailureEvidenceRecordedEvent"),
            SubmissionFailureEvidenceRecordedEvent {
                dag: id(addr(0xcf)),
                execution: id(addr(0xd0)),
                walk_index: 77,
                vertex: RuntimeVertex::plain("submission_failure"),
                failed_leader: addr(0xd1),
                winning_leader: MoveOption::from_option(Some(addr(0xd2))),
                reason: MoveString::from("invalid evidence"),
                err_eval_hash: vec![10, 11, 12],
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
                outcome: MoveOption::from_option(Some(PostFailureAction::Terminate)),
                reason: MoveString::from("terminal"),
                err_eval_hash: vec![1, 2, 3],
                duplicate: false,
            }
        );
        check!(
            "ToolVerificationResolved",
            check!(@tag objects.workflow_pkg_id, "execution_events", "ToolVerificationResolved"),
            ToolVerificationResolved {
                dag: id(addr(0x52)),
                execution: id(addr(0x53)),
                walk_index: 64,
                vertex: RuntimeVertex::plain("verified"),
                leader_cap_id: id(addr(0x54)),
                tool_id: id(addr(0x55)),
                verifier_kind:
                    crate::move_bindings::interface::verifier::ToolVerifierMode::External,
                verifier_witness_id: MoveOption::from_option(Some(id(addr(0x56)))),
                decision: crate::move_bindings::interface::verifier::VerifierDecision::Accept,
            }
        );
        check!(
            "WalkPendingAbortEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "WalkPendingAbortEvent"),
            WalkPendingAbortEvent {
                dag: id(addr(0xd3)),
                execution: id(addr(0xd4)),
                walk_index: 78,
                vertex: RuntimeVertex::plain("pending_abort"),
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
                variant: crate::move_bindings::interface::graph::OutputVariant {
                    name: MoveString::from("ok"),
                },
                variant_ports_to_data: MoveVecMap {
                    contents: vec![VecMapEntry {
                        key: crate::move_bindings::interface::graph::OutputPort {
                            name: MoveString::from("answer"),
                        },
                        value: crate::move_bindings::primitives::data::NexusData {
                            storage: b"inline".to_vec(),
                            one: b"42".to_vec(),
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
            "ExecutionPaymentRefilledEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "ExecutionPaymentRefilledEvent"),
            ExecutionPaymentRefilledEvent {
                execution_id: addr(0xd5),
                payment_id: addr(0xd6),
                source: addr(0xd7),
                refill_amount: 79,
            }
        );
        check!(
            "ExecutionPaymentInsufficientSettlementEvent",
            check!(@tag objects.workflow_pkg_id, "execution_events", "ExecutionPaymentInsufficientSettlementEvent"),
            ExecutionPaymentInsufficientSettlementEvent {
                execution: id(addr(0x5d)),
                walk_index: 70,
                required_shortfall: 71,
            }
        );
        check!(
            "MissedOccurrenceEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "MissedOccurrenceEvent"),
            MissedOccurrenceEvent {
                task: id(addr(0x5d)),
                start_time_ms: 70,
                deadline_ms: MoveOption::from_option(Some(71)),
                pruned_at: 72,
                priority_fee_percentage: 73,
                generator: generator(),
            }
        );
        check!(
            "OccurrenceConsumedEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "OccurrenceConsumedEvent"),
            OccurrenceConsumedEvent {
                task: id(addr(0x5e)),
                start_time_ms: 74,
                deadline_ms: MoveOption::from_option(None),
                priority_fee_percentage: 75,
                generator: generator(),
                executed_at: 76,
            }
        );
        check!(
            "PeriodicScheduleConfiguredEvent",
            check!(@tag objects.scheduler_pkg_id, "scheduler", "PeriodicScheduleConfiguredEvent"),
            PeriodicScheduleConfiguredEvent {
                task: id(addr(0x5f)),
                period_ms: MoveOption::from_option(Some(77)),
                deadline_offset_ms: MoveOption::from_option(Some(78)),
                max_iterations: MoveOption::from_option(Some(79)),
                generated: MoveOption::from_option(Some(80)),
                priority_fee_percentage: 81,
                last_generated_start_ms: MoveOption::from_option(Some(82)),
            }
        );
        check!(
            "PriorityFeeSwapEvent",
            check!(@tag objects.registry_pkg_id, "priority_fee_vault", "PriorityFeeSwapEvent"),
            PriorityFeeSwapEvent {
                vault: id(addr(0xda)),
                us_in: 83,
                us_refunded: 84,
                sui_out: 85,
            }
        );
        check!(
            "PriorityFeeDepositEvent",
            check!(@tag objects.registry_pkg_id, "priority_fee_vault", "PriorityFeeDepositEvent"),
            PriorityFeeDepositEvent {
                vault: id(addr(0xd8)),
                leader_cap_id: id(addr(0xd9)),
                amount: 80,
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

        assert_eq!(parsed, 56);
    }
}
