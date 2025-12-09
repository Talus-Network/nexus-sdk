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
        let Some(event_type) = event.type_.type_params().get(0).and_then(|tag| match tag {
            sui::types::TypeTag::Struct(struct_tag) => Some(struct_tag),
            _ => None,
        }) else {
            bail!("EventWrapper does not have a valid event type parameter");
        };

        let event_name = event_type.name().as_str();

        Ok(NexusEvent {
            id: (digest, index),
            generics: event_type.type_params().to_vec(),
            data: parse_bcs(event_name, &event.contents)?,
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
            bail!(
                "Event does not come from a Nexus package, it comes from '{}' instead",
                package_id
            );
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
        let Some(event_type) = struct_tag.type_params().get(0).and_then(|tag| match tag {
            sui::types::TypeTag::Struct(struct_tag) => Some(struct_tag),
            _ => None,
        }) else {
            bail!("EventWrapper does not have a valid event type parameter");
        };

        let event_name = event_type.name().as_str();

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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::{
                events_query::events_query::EventsQueryEventsNodesContentsType,
                DAGCreatedEvent,
            },
            idents::primitives,
            test_utils::sui_mocks,
        },
        serde::{Deserialize, Serialize},
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
}
