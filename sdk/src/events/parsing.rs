//! This module defines transformers from various Sui types to Nexus event types.
//! Namely we support:
//! - Parsing GRPC [`sui_sdk_types::Event`]
//! - Parsing GQL response type

use {
    crate::{
        events::{
            events_query::events_query::EventsQueryEventsNodesContents,
            parse_bcs,
            NexusEvent,
        },
        idents::primitives,
        sui,
        types::NexusObjects,
    },
    anyhow::bail,
    serde_json::json,
};

/// [`sui_sdk_types::Event`] -> [`NexusEvent`]
pub trait FromSuiGrpcEvent {
    /// Parse a Sui GRPC event into a Nexus event.
    fn from_sui_grpc_event(
        index: u64,
        digest: sui::types::Digest,
        event: &sui::types::Event,
        objects: &NexusObjects,
    ) -> anyhow::Result<NexusEvent>;
}

/// [`EventsQueryEventsNodes`] -> [`NexusEvent`]
pub trait FromSuiGqlEvent {
    /// Parse a Sui GQL event into a Nexus event.
    fn from_sui_gql_event(
        index: u64,
        digest: sui::types::Digest,
        package_id: sui::types::Address,
        event: &EventsQueryEventsNodesContents,
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
        // Only accept events that come from the Nexus packages.
        if !is_nexus_package(event.package_id, objects) {
            bail!(
                "Event does not come from a Nexus package, it comes from '{}' instead",
                event.package_id
            );
        }

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

        Ok(NexusEvent {
            id: (digest, index),
            generics: event_type.type_params().to_vec(),
            data: parse_bcs(&event_name, &event.contents)?,
        })
    }
}

impl FromSuiGqlEvent for NexusEvent {
    fn from_sui_gql_event(
        index: u64,
        digest: sui::types::Digest,
        package_id: sui::types::Address,
        event: &EventsQueryEventsNodesContents,
        objects: &NexusObjects,
    ) -> anyhow::Result<NexusEvent> {
        // Only accept events that come from the Nexus packages.
        if !is_nexus_package(package_id, objects) {
            bail!("Event does not come from a Nexus package, it comes from '{package_id}' instead");
        }

        let struct_tag: sui::types::StructTag = event
            .type_
            .as_ref()
            .and_then(|t| t.repr.parse().ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to parse event type '{:?}' into StructTag",
                    event.type_
                )
            })?;

        // Only accept events that are wrapped in `nexus_primitives::event::EventWrapper`.
        if !is_event_wrapper(&struct_tag, objects) {
            bail!(
                "Event is not wrapped in '{}::event::EventWrapper', found type: '{:?}'",
                objects.primitives_pkg_id,
                event.type_
            );
        }

        // Extract the name of the event we want to parse into.
        let Some(event_type) = struct_tag.type_params().first().and_then(|tag| match tag {
            sui::types::TypeTag::Struct(struct_tag) => Some(struct_tag),
            _ => None,
        }) else {
            bail!("EventWrapper does not have a valid event type parameter");
        };

        let event_name = normalize_event_name(event_type)?;

        let Some(json) = event
            .json
            .as_ref()
            .and_then(|j| j.as_object())
            .and_then(|obj| obj.get("event"))
        else {
            bail!("Event contents missing JSON data");
        };

        let data = json!({
            "_nexus_event_type": event_name,
            "event": json
        });

        Ok(NexusEvent {
            id: (digest, index),
            generics: event_type.type_params().to_vec(),
            data: serde_json::from_value(data)?,
        })
    }
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

/// Helper function to determine whether the given address is one of the Nexus
/// package addresses.
fn is_nexus_package(address: sui::types::Address, objects: &NexusObjects) -> bool {
    address == objects.primitives_pkg_id
        || address == objects.interface_pkg_id
        || address == objects.workflow_pkg_id
}

/// Helper function to determine whether the provided struct tag corresponds to
/// `nexus_primitives::event::EventWrapper`.
fn is_event_wrapper(tag: &sui::types::StructTag, objects: &NexusObjects) -> bool {
    *tag.address() == objects.primitives_pkg_id
        && *tag.module() == primitives::Event::EVENT_WRAPPER.module
        && *tag.name() == primitives::Event::EVENT_WRAPPER.name
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            events::{
                events_query::events_query::EventsQueryEventsNodesContentsType,
                AnnounceInterfacePackageEvent,
                DAGCreatedEvent,
                EndStateReachedEvent,
                ExecutionFinishedEvent,
                FoundingLeaderCapCreatedEvent,
                GasSettlementUpdateEvent,
                MissedOccurrenceEvent,
                NexusEventKind,
                OccurrenceConsumedEvent,
                OccurrenceScheduledEvent,
                PeriodicScheduleConfiguredEvent,
                PreKeyAssociatedEvent,
                PreKeyFulfilledEvent,
                PreKeyRequestedEvent,
                PreKeyVaultCreatedEvent,
                RequestScheduledOccurrenceEvent,
                RequestScheduledWalkEvent,
                RequestWalkExecutionEvent,
                RuntimeVertex,
                TaskCanceledEvent,
                TaskCreatedEvent,
                TaskPausedEvent,
                TaskResumedEvent,
                ToolRegisteredEvent,
                ToolUnregisteredEvent,
                TypeName,
                WalkAdvancedEvent,
                WalkFailedEvent,
            },
            fqn,
            idents::primitives,
            test_utils::sui_mocks,
            types::{NexusData, PolicySymbol, PortsData, SharedObjectRef},
            ToolFqn,
        },
        serde::{Deserialize, Serialize},
        std::sync::Arc,
    };

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct Wrapper<T> {
        event: T,
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
    fn test_parse_from_gql_request_scheduled_occurrence_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();

        let inner_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("scheduler").unwrap(),
            sui::types::Identifier::new("OccurrenceScheduledEvent").unwrap(),
            vec![],
        );

        let scheduled_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("scheduler").unwrap(),
            sui::types::Identifier::new("RequestScheduledExecution").unwrap(),
            vec![sui::types::TypeTag::Struct(Box::new(inner_type))],
        );

        let wrapper_type = sui::types::StructTag::new(
            objects.primitives_pkg_id,
            primitives::Event::EVENT_WRAPPER.module,
            primitives::Event::EVENT_WRAPPER.name,
            vec![sui::types::TypeTag::Struct(Box::new(scheduled_type))],
        );

        let generator_uid = sui::types::Address::generate(&mut rng);
        let occurrence = OccurrenceScheduledEvent {
            task: sui::types::Address::generate(&mut rng),
            generator: crate::types::PolicySymbol::Uid(generator_uid),
        };

        let scheduled = RequestScheduledOccurrenceEvent {
            request: occurrence.clone(),
            priority: 5,
            request_ms: 10,
            start_ms: 20,
            deadline_ms: 30,
        };

        let gql_event = EventsQueryEventsNodesContents {
            json: Some(serde_json::json!({
                "event": {
                    "request": {
                        "task": scheduled.request.task,
                        "generator": {
                            "variant": "Uid",
                            "fields": { "pos0": generator_uid }
                        }
                    },
                    "priority": scheduled.priority.to_string(),
                    "request_ms": scheduled.request_ms.to_string(),
                    "start_ms": scheduled.start_ms.to_string(),
                    "deadline_ms": scheduled.deadline_ms.to_string(),
                }
            })),
            type_: Some(EventsQueryEventsNodesContentsType {
                repr: wrapper_type.to_string(),
            }),
        };

        let result = NexusEvent::from_sui_gql_event(
            index,
            digest,
            objects.primitives_pkg_id,
            &gql_event,
            &objects,
        )
        .expect("Should parse scheduled occurrence event");

        assert!(matches!(
            result.data,
            NexusEventKind::RequestScheduledOccurrence(env)
                if env.request.task == scheduled.request.task
                    && env.priority == scheduled.priority
                    && env.start_ms == scheduled.start_ms
        ));
    }

    #[test]
    fn test_parse_from_gql_request_scheduled_walk_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();

        let inner_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("scheduler").unwrap(),
            sui::types::Identifier::new("RequestWalkExecutionEvent").unwrap(),
            vec![],
        );

        let scheduled_type = sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::new("scheduler").unwrap(),
            sui::types::Identifier::new("RequestScheduledExecution").unwrap(),
            vec![sui::types::TypeTag::Struct(Box::new(inner_type))],
        );

        let wrapper_type = sui::types::StructTag::new(
            objects.primitives_pkg_id,
            primitives::Event::EVENT_WRAPPER.module,
            primitives::Event::EVENT_WRAPPER.name,
            vec![sui::types::TypeTag::Struct(Box::new(scheduled_type))],
        );

        let walk = RequestWalkExecutionEvent {
            dag: sui::types::Address::generate(&mut rng),
            execution: sui::types::Address::generate(&mut rng),
            invoker: sui::types::Address::generate(&mut rng),
            walk_index: 1,
            next_vertex: RuntimeVertex::plain("v"),
            evaluations: sui::types::Address::generate(&mut rng),
            worksheet_from_type: TypeName::new("worksheet"),
        };

        let scheduled = RequestScheduledWalkEvent {
            request: walk.clone(),
            priority: 1,
            request_ms: 2,
            start_ms: 3,
            deadline_ms: 4,
        };

        let gql_event = EventsQueryEventsNodesContents {
            json: Some(
                serde_json::to_value(&Wrapper {
                    event: scheduled.clone(),
                })
                .unwrap(),
            ),
            type_: Some(EventsQueryEventsNodesContentsType {
                repr: wrapper_type.to_string(),
            }),
        };

        let result = NexusEvent::from_sui_gql_event(
            index,
            digest,
            objects.primitives_pkg_id,
            &gql_event,
            &objects,
        )
        .expect("Should parse scheduled walk event");

        assert!(matches!(
            result.data,
            NexusEventKind::RequestScheduledWalk(env)
                if env.request.dag == scheduled.request.dag
                    && env.start_ms == scheduled.start_ms
                    && env.deadline_ms == scheduled.deadline_ms
        ));
    }

    #[test]
    fn test_parse_from_gql_valid_nexus_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let primitives_pkg_id = objects.primitives_pkg_id;
        let event_type = sui::types::StructTag::new(
            objects.primitives_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("DAGCreatedEvent").unwrap(),
            vec![],
        );
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: DAGCreatedEvent { dag: dag_addr },
        };
        let gql_event = EventsQueryEventsNodesContents {
            json: Some(serde_json::to_value(&data).unwrap()),
            type_: Some(EventsQueryEventsNodesContentsType {
                repr: sui::types::StructTag::new(
                    primitives_pkg_id,
                    primitives::Event::EVENT_WRAPPER.module,
                    primitives::Event::EVENT_WRAPPER.name,
                    vec![wrapper_type.clone()],
                )
                .to_string(),
            }),
        };

        let result =
            NexusEvent::from_sui_gql_event(index, digest, primitives_pkg_id, &gql_event, &objects);
        assert!(result.is_ok(), "Should parse valid Nexus GQL event");
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.generics, vec![]);
        assert_eq!(nexus_event.id, (digest, index));
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_addr
        ));
    }

    #[test]
    fn test_parse_from_gql_non_nexus_package_event() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let non_nexus_pkg_id = sui::types::Address::generate(&mut rng);
        let event_type = sui::types::StructTag::new(
            non_nexus_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("DAGCreatedEvent").unwrap(),
            vec![],
        );
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: DAGCreatedEvent { dag: dag_addr },
        };
        let gql_event = EventsQueryEventsNodesContents {
            json: Some(serde_json::to_value(&data).unwrap()),
            type_: Some(EventsQueryEventsNodesContentsType {
                repr: sui::types::StructTag::new(
                    non_nexus_pkg_id,
                    primitives::Event::EVENT_WRAPPER.module,
                    primitives::Event::EVENT_WRAPPER.name,
                    vec![wrapper_type.clone()],
                )
                .to_string(),
            }),
        };
        let result =
            NexusEvent::from_sui_gql_event(index, digest, non_nexus_pkg_id, &gql_event, &objects);
        assert!(
            result.is_err(),
            "Should fail for non-Nexus package GQL event"
        );
    }

    #[test]
    fn test_parse_from_gql_non_event_wrapper_gql_type() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let primitives_pkg_id = objects.primitives_pkg_id;
        let event_type = sui::types::StructTag::new(
            primitives_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("DAGCreatedEvent").unwrap(),
            vec![],
        );
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: DAGCreatedEvent { dag: dag_addr },
        };
        // Use a non-wrapper struct tag
        let non_wrapper_tag = sui::types::StructTag::new(
            primitives_pkg_id,
            sui::types::Identifier::new("not_wrapper").unwrap(),
            sui::types::Identifier::new("not_wrapper").unwrap(),
            vec![wrapper_type.clone()],
        );
        let gql_event = EventsQueryEventsNodesContents {
            json: Some(serde_json::to_value(&data).unwrap()),
            type_: Some(EventsQueryEventsNodesContentsType {
                repr: non_wrapper_tag.to_string(),
            }),
        };
        let result =
            NexusEvent::from_sui_gql_event(index, digest, primitives_pkg_id, &gql_event, &objects);
        assert!(result.is_err(), "Should fail for non-EventWrapper GQL type");
    }

    #[test]
    fn test_parse_from_gql_event_wrapper_missing_type_param() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let primitives_pkg_id = objects.primitives_pkg_id;
        // Wrapper tag with no type params
        let wrapper_tag = sui::types::StructTag::new(
            primitives_pkg_id,
            primitives::Event::EVENT_WRAPPER.module,
            primitives::Event::EVENT_WRAPPER.name,
            vec![],
        );
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: DAGCreatedEvent { dag: dag_addr },
        };
        let gql_event = EventsQueryEventsNodesContents {
            json: Some(serde_json::to_value(&data).unwrap()),
            type_: Some(EventsQueryEventsNodesContentsType {
                repr: wrapper_tag.to_string(),
            }),
        };
        let result =
            NexusEvent::from_sui_gql_event(index, digest, primitives_pkg_id, &gql_event, &objects);
        assert!(
            result.is_err(),
            "Should fail for missing type param in GQL event"
        );
    }

    #[test]
    fn test_parse_from_gql_valid_nexus_event_with_generics() {
        let mut rng = rand::thread_rng();
        let index = 0u64;
        let digest = sui::types::Digest::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();
        let primitives_pkg_id = objects.primitives_pkg_id;
        let event_type = sui::types::StructTag::new(
            primitives_pkg_id,
            sui::types::Identifier::new("dag").unwrap(),
            sui::types::Identifier::new("DAGCreatedEvent").unwrap(),
            vec![sui::types::TypeTag::U64],
        );
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(event_type.clone()));
        let dag_addr = sui::types::Address::generate(&mut rng);
        let data = Wrapper {
            event: DAGCreatedEvent { dag: dag_addr },
        };
        let gql_event = EventsQueryEventsNodesContents {
            json: Some(serde_json::to_value(&data).unwrap()),
            type_: Some(EventsQueryEventsNodesContentsType {
                repr: sui::types::StructTag::new(
                    primitives_pkg_id,
                    primitives::Event::EVENT_WRAPPER.module,
                    primitives::Event::EVENT_WRAPPER.name,
                    vec![wrapper_type.clone()],
                )
                .to_string(),
            }),
        };

        let result =
            NexusEvent::from_sui_gql_event(index, digest, primitives_pkg_id, &gql_event, &objects);
        assert!(
            result.is_ok(),
            "Should parse valid Nexus GQL event with generics"
        );
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.generics, vec![sui::types::TypeTag::U64]);
        assert_eq!(nexus_event.id, (digest, index));
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_addr
        ));
    }

    #[tokio::test]
    async fn test_parse_from_gql_all_events_roundtrip() {
        let mut server = mockito::Server::new_async().await;
        let objects = Arc::new(sui_mocks::mock_nexus_objects());
        let primitives_pkg_id = objects.primitives_pkg_id;

        let events = sample_events();

        let mock = sui_mocks::gql::mock_event_query(
            &mut server,
            primitives_pkg_id,
            events.clone(),
            None,
            Some("cursor"),
        );

        let fetcher = crate::events::EventFetcher::new(
            &format!("{}/graphql", &server.url()),
            objects.clone(),
        );

        let (_poller, mut receiver) = fetcher.poll_nexus_events(None, None);
        let page = receiver
            .recv()
            .await
            .expect("fetcher should yield a page of events")
            .expect("result should be Ok");

        assert_eq!(page.events.len(), events.len());

        for (expected, parsed) in events.iter().zip(page.events.iter()) {
            assert_eq!(expected.name(), parsed.data.name());
        }

        mock.assert_async().await;
    }

    fn sample_events() -> Vec<NexusEventKind> {
        let mut idx: u8 = 1;
        let mut addr = || {
            let hex = format!("0x{:02x}", idx);
            idx = idx.wrapping_add(1);
            hex.parse::<sui::types::Address>().unwrap()
        };
        let fqn: ToolFqn = fqn!("xyz.taluslabs.example@1");
        let vertex = RuntimeVertex::plain("v");
        let ports = PortsData::from_map(
            std::iter::once((
                "p".to_string(),
                NexusData::new_inline(serde_json::json!({ "k": 1 })),
            ))
            .collect(),
        );

        vec![
            NexusEventKind::RequestScheduledOccurrence(RequestScheduledOccurrenceEvent {
                request: OccurrenceScheduledEvent {
                    task: addr(),
                    generator: PolicySymbol::Uid(addr()),
                },
                priority: 1,
                request_ms: 2,
                start_ms: 3,
                deadline_ms: 4,
            }),
            NexusEventKind::RequestScheduledWalk(RequestScheduledWalkEvent {
                request: RequestWalkExecutionEvent {
                    dag: addr(),
                    execution: addr(),
                    invoker: addr(),
                    walk_index: 1,
                    next_vertex: vertex.clone(),
                    evaluations: addr(),
                    worksheet_from_type: TypeName::new("worksheet"),
                },
                priority: 1,
                request_ms: 2,
                start_ms: 3,
                deadline_ms: 4,
            }),
            NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
                task: addr(),
                generator: PolicySymbol::Uid(addr()),
            }),
            NexusEventKind::RequestWalkExecution(RequestWalkExecutionEvent {
                dag: addr(),
                execution: addr(),
                invoker: addr(),
                walk_index: 7,
                next_vertex: vertex.clone(),
                evaluations: addr(),
                worksheet_from_type: TypeName::new("worksheet"),
            }),
            NexusEventKind::AnnounceInterfacePackage(AnnounceInterfacePackageEvent {
                shared_objects: vec![SharedObjectRef::new_imm(addr())],
            }),
            NexusEventKind::ToolRegistered(ToolRegisteredEvent {
                tool: addr(),
                fqn: fqn.clone(),
            }),
            NexusEventKind::ToolUnregistered(ToolUnregisteredEvent {
                tool: addr(),
                fqn: fqn.clone(),
            }),
            NexusEventKind::WalkAdvanced(WalkAdvancedEvent {
                dag: addr(),
                execution: addr(),
                walk_index: 1,
                vertex: vertex.clone(),
                variant: TypeName::new("var"),
                variant_ports_to_data: ports.clone(),
            }),
            NexusEventKind::WalkFailed(WalkFailedEvent {
                dag: addr(),
                execution: addr(),
                walk_index: 2,
                vertex: vertex.clone(),
                reason: "fail".to_string(),
            }),
            NexusEventKind::EndStateReached(EndStateReachedEvent {
                dag: addr(),
                execution: addr(),
                walk_index: 3,
                vertex: vertex.clone(),
                variant: TypeName::new("end"),
                variant_ports_to_data: ports.clone(),
            }),
            NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
                dag: addr(),
                execution: addr(),
                has_any_walk_failed: true,
                has_any_walk_succeeded: true,
            }),
            NexusEventKind::MissedOccurrence(MissedOccurrenceEvent {
                task: addr(),
                start_time_ms: 1,
                deadline_ms: Some(2),
                pruned_at: 3,
                priority_fee_per_gas_unit: 4,
                generator: PolicySymbol::Uid(addr()),
            }),
            NexusEventKind::TaskCreated(TaskCreatedEvent {
                task: addr(),
                owner: addr(),
            }),
            NexusEventKind::TaskPaused(TaskPausedEvent { task: addr() }),
            NexusEventKind::TaskResumed(TaskResumedEvent { task: addr() }),
            NexusEventKind::TaskCanceled(TaskCanceledEvent {
                task: addr(),
                cleared_occurrences: 1,
                had_periodic: true,
            }),
            NexusEventKind::OccurrenceConsumed(OccurrenceConsumedEvent {
                task: addr(),
                start_time_ms: 1,
                deadline_ms: Some(2),
                priority_fee_per_gas_unit: 3,
                generator: PolicySymbol::Uid(addr()),
                executed_at: 4,
            }),
            NexusEventKind::PeriodicScheduleConfigured(PeriodicScheduleConfiguredEvent {
                task: addr(),
                period_ms: Some(1),
                deadline_offset_ms: Some(2),
                max_iterations: Some(3),
                generated: Some(4),
                priority_fee_per_gas_unit: Some(5),
                last_generated_start_ms: Some(6),
            }),
            NexusEventKind::FoundingLeaderCapCreated(FoundingLeaderCapCreatedEvent {
                leader_cap: addr(),
                network: addr(),
            }),
            NexusEventKind::GasSettlementUpdate(GasSettlementUpdateEvent {
                execution: addr(),
                tool_fqn: fqn.clone(),
                vertex: vertex.clone(),
                was_settled: true,
            }),
            NexusEventKind::PreKeyVaultCreated(PreKeyVaultCreatedEvent {
                vault: addr(),
                crypto_cap: addr(),
            }),
            NexusEventKind::PreKeyRequested(PreKeyRequestedEvent {
                requested_by: addr(),
            }),
            NexusEventKind::PreKeyFulfilled(PreKeyFulfilledEvent {
                requested_by: addr(),
                pre_key_bytes: vec![1, 2, 3],
            }),
            NexusEventKind::PreKeyAssociated(PreKeyAssociatedEvent {
                claimed_by: addr(),
                pre_key: vec![4, 5],
                initial_message: vec![6, 7],
            }),
            NexusEventKind::DAGCreated(DAGCreatedEvent { dag: addr() }),
        ]
    }
}
