//! This module defines transformers from various Sui types to Nexus event types.
//! Namely we support:
//! - Parsing GRPC [`sui_sdk_types::Event`]
//! - Parsing GQL response type

use {
    crate::{
        events::{parse_bcs, NexusEvent},
        idents::primitives,
        sui,
        types::NexusObjects,
    },
    anyhow::bail,
};

/// [`sui_sdk_types::Event`] -> [`NexusEvent`]
pub trait FromSuiGrpcEvent {
    /// Parse a Sui GRPC event into a Nexus event.
    fn from_sui_grpc_event(
        event: &sui::types::Event,
        objects: &NexusObjects,
    ) -> anyhow::Result<NexusEvent>;
}

/// TODO: GQL -> [`NexusEvent`]
pub trait FromSuiGqlEvent {
    /// Parse a Sui GQL event into a Nexus event.
    fn from_sui_gql_event(
        // event: &sui::gql::Event,
        objects: &NexusObjects,
    ) -> anyhow::Result<NexusEvent>;
}

impl FromSuiGrpcEvent for NexusEvent {
    fn from_sui_grpc_event(
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
            generics: event_type.type_params().to_vec(),
            data: parse_bcs(event_name, &event.contents)?,
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
        crate::{events::DAGCreatedEvent, idents::primitives, test_utils::sui_mocks},
        serde::{Deserialize, Serialize},
    };

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct Wrapper<T> {
        event: T,
    }

    #[test]
    fn test_parse_valid_nexus_event() {
        let mut rng = rand::thread_rng();
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

        let result = NexusEvent::from_sui_grpc_event(&event, &objects);
        assert!(result.is_ok(), "Should parse valid Nexus event");
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.generics, vec![]);
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_addr
        ));
    }

    #[test]
    fn test_parse_non_nexus_package_event() {
        let mut rng = rand::thread_rng();
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

        let result = NexusEvent::from_sui_grpc_event(&event, &objects);
        assert!(result.is_err(), "Should fail for non-Nexus package event");
    }

    #[test]
    fn test_parse_non_event_wrapper_type() {
        let objects = sui_mocks::mock_nexus_objects();
        let wrong_tag = sui::types::StructTag::new(
            objects.primitives_pkg_id,
            sui::types::Identifier::new("wrong_module").unwrap(),
            sui::types::Identifier::new("wrong_name").unwrap(),
            vec![],
        );
        let event = sui_mocks::mock_sui_event(objects.primitives_pkg_id, wrong_tag, vec![1, 2, 3]);
        let result = NexusEvent::from_sui_grpc_event(&event, &objects);
        assert!(result.is_err(), "Should fail for non-EventWrapper type");
    }

    #[test]
    fn test_parse_event_wrapper_missing_type_param() {
        let objects = sui_mocks::mock_nexus_objects();
        let wrapper_tag = sui::types::StructTag::new(
            objects.primitives_pkg_id,
            sui::types::Identifier::new(primitives::Event::EVENT_WRAPPER.module.as_str()).unwrap(),
            sui::types::Identifier::new(primitives::Event::EVENT_WRAPPER.name.as_str()).unwrap(),
            vec![],
        );
        let event =
            sui_mocks::mock_sui_event(objects.primitives_pkg_id, wrapper_tag, vec![1, 2, 3]);
        let result = NexusEvent::from_sui_grpc_event(&event, &objects);
        assert!(result.is_err(), "Should fail for missing type param");
    }

    #[test]
    fn test_parse_valid_nexus_event_with_generics() {
        let mut rng = rand::thread_rng();
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

        let result = NexusEvent::from_sui_grpc_event(&event, &objects);
        assert!(result.is_ok(), "Should parse valid Nexus event");
        let nexus_event = result.unwrap();
        assert_eq!(nexus_event.generics, vec![sui::types::TypeTag::U64]);
        assert!(matches!(
            nexus_event.data,
            crate::events::NexusEventKind::DAGCreated(DAGCreatedEvent { dag }) if dag == dag_addr
        ));
    }
}
