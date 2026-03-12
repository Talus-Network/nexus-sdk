//! This module defines transformers from various Sui types to Nexus event types.
//! Namely we support:
//! - Parsing GRPC [`sui::types::Event`]

use {
    crate::{
        events::{parse_bcs, NexusEvent},
        idents::primitives,
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

        // Only accept events that come from the Nexus packages unless the
        // event is marked as foreign.
        if !is_nexus_package(event.package_id, objects) && !is_foreign_event(&event_name) {
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

/// Helper function to determine whether the event name corresponds to a foreign
/// event.
fn is_foreign_event(event_name: &str) -> bool {
    match event_name {
        "AnnounceInterfacePackageEvent" => true,
        _ => false,
    }
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
        crate::{events::*, idents::primitives, test_utils::sui_mocks, types::SharedObjectRef},
        serde::{Deserialize, Serialize},
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

    /// Return a sample list of various events in BCS directly from the chain.
    fn sample_events() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            (
                "RequestScheduledWalkEvent",
                vec![
                    172, 45, 232, 250, 15, 55, 177, 42, 63, 139, 114, 186, 218, 6, 79, 233, 155,
                    245, 118, 65, 38, 9, 194, 133, 80, 214, 234, 139, 42, 249, 215, 254, 137, 85,
                    88, 251, 70, 35, 154, 244, 157, 83, 95, 160, 229, 41, 235, 87, 49, 34, 108,
                    227, 130, 217, 34, 60, 63, 1, 217, 168, 78, 221, 225, 177, 225, 55, 128, 133,
                    77, 55, 145, 161, 138, 59, 157, 111, 32, 91, 56, 106, 138, 120, 219, 83, 24,
                    59, 31, 20, 245, 161, 67, 82, 113, 22, 105, 245, 0, 0, 0, 0, 0, 0, 0, 0, 1, 5,
                    100, 117, 109, 109, 121, 15, 4, 47, 22, 180, 230, 20, 20, 148, 255, 73, 141,
                    105, 187, 102, 235, 190, 39, 150, 230, 194, 116, 197, 79, 149, 160, 81, 118,
                    119, 162, 50, 67, 98, 56, 48, 51, 48, 51, 101, 51, 57, 48, 100, 51, 99, 50, 51,
                    99, 100, 51, 98, 100, 50, 101, 102, 97, 56, 52, 51, 100, 55, 53, 55, 50, 54,
                    48, 57, 101, 51, 53, 51, 53, 48, 57, 51, 100, 50, 56, 101, 100, 55, 50, 50, 50,
                    52, 48, 55, 55, 52, 49, 55, 56, 101, 50, 53, 101, 54, 58, 58, 100, 101, 102,
                    97, 117, 108, 116, 95, 116, 97, 112, 58, 58, 68, 101, 102, 97, 117, 108, 116,
                    84, 65, 80, 86, 49, 87, 105, 116, 110, 101, 115, 115, 194, 253, 110, 49, 44,
                    232, 173, 17, 187, 224, 166, 199, 143, 30, 119, 151, 248, 210, 201, 14, 34, 15,
                    69, 8, 49, 214, 26, 231, 17, 124, 250, 113, 0, 0, 0, 0, 0, 0, 0, 0, 212, 39,
                    34, 195, 156, 1, 0, 0, 212, 39, 34, 195, 156, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    48, 117, 0, 0, 0, 0, 0, 0, 212, 39, 34, 195, 156, 1, 0, 0, 235, 182, 85, 86,
                    227, 215, 174, 219, 221, 108, 207, 129, 228, 115, 113, 83, 118, 141, 234, 189,
                    80, 32, 47, 194, 209, 37, 52, 154, 4, 96, 207, 221, 2, 103, 9, 248, 223, 25,
                    42, 12, 58, 238, 126, 174, 234, 146, 57, 167, 24, 134, 63, 229, 18, 116, 169,
                    175, 18, 228, 106, 208, 225, 232, 155, 93, 123, 249, 197, 12, 241, 105, 213,
                    209, 99, 241, 22, 91, 54, 129, 43, 201, 235, 31, 195, 43, 0, 38, 92, 210, 42,
                    35, 69, 206, 211, 21, 62, 247, 125,
                ],
            ),
            (
                "DAGCreatedEvent",
                vec![
                    172, 45, 232, 250, 15, 55, 177, 42, 63, 139, 114, 186, 218, 6, 79, 233, 155,
                    245, 118, 65, 38, 9, 194, 133, 80, 214, 234, 139, 42, 249, 215, 254,
                ],
            ),
            (
                "GasLockUpdateEvent",
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
        ]
    }
}
