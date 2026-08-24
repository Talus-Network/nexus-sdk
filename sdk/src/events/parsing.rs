//! Nexus event type selection and decoding.

use crate::{
    events::{
        parse_bcs,
        supports_event,
        NexusEvent,
        NexusEventCandidate,
        NexusEventDecodeError,
        UnsupportedNexusEvent,
    },
    move_bindings::{
        interface::distributed_event as distributed_event_move,
        primitives::{data::NexusData as MoveNexusData, event as event_move},
    },
    sui,
    types::NexusObjects,
};

struct NexusEventType<'a> {
    tag: &'a sui::types::StructTag,
    name: &'a str,
}

impl<'a> NexusEventType<'a> {
    fn resolve(wrapper_type: &'a sui::types::StructTag, objects: &NexusObjects) -> Option<Self> {
        if !is_event_wrapper(wrapper_type, objects) {
            return None;
        }

        let tag = wrapper_type
            .type_params()
            .first()
            .and_then(|tag| match tag {
                sui::types::TypeTag::Struct(struct_tag) => Some(struct_tag),
                _ => None,
            })?;
        if !is_nexus_package(*tag.address(), objects) {
            return None;
        }

        Some(Self {
            tag,
            name: tag.name().as_str(),
        })
    }
}

pub(super) fn classify_nexus_event(
    index: u64,
    digest: sui::types::Digest,
    emitting_package: sui::types::Address,
    contents: &[u8],
    wrapper_type: &sui::types::StructTag,
    objects: &NexusObjects,
) -> Result<Option<NexusEventCandidate>, NexusEventDecodeError> {
    let Some(event_type) = NexusEventType::resolve(wrapper_type, objects) else {
        return Ok(None);
    };
    if !supports_event(event_type.name) {
        return Ok(Some(NexusEventCandidate::Unsupported(
            UnsupportedNexusEvent {
                id: (digest, index),
                source_package: emitting_package,
                event_type: Box::new(event_type.tag.clone()),
            },
        )));
    }
    let (data, distribution) = parse_bcs(event_type.name, contents).map_err(|error| {
        NexusEventDecodeError::Contents(
            error.context(format!("Could not decode {}", event_type.name)),
        )
    })?;

    Ok(Some(NexusEventCandidate::Supported(Box::new(NexusEvent {
        id: (digest, index),
        emitting_package,
        generics: event_type.tag.type_params().to_vec(),
        data,
        distribution,
    }))))
}

fn is_nexus_package(address: sui::types::Address, objects: &NexusObjects) -> bool {
    objects.is_nexus_package(address)
}

fn is_event_wrapper(tag: &sui::types::StructTag, objects: &NexusObjects) -> bool {
    crate::move_bindings::struct_tag_matches::<event_move::EventWrapper<MoveNexusData>>(
        objects, tag,
    ) || crate::move_bindings::struct_tag_matches::<
        distributed_event_move::DistributedEventWrapper<MoveNexusData>,
    >(objects, tag)
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            events::NexusEventKind,
            move_bindings::{
                move_std::option::Option as MoveOption,
                scheduler::{
                    schedule::OccurrenceSource,
                    scheduler::{OccurrenceAdvertisedEvent, TaskCreatedEvent},
                    task::TaskController,
                },
                sui_framework::object::ID,
            },
        },
        serde::Serialize,
    };

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

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    #[test]
    fn parses_direct_event_wrapper() {
        let task_id = ID::new(address("0x41"));
        let event = TaskCreatedEvent::new(
            task_id,
            TaskController::Address {
                pos0: address("0x42"),
            },
            ID::new(address("0x43")),
            7,
        );
        let bytes = bcs::to_bytes(&Wrapper { event }).expect("event serializes");

        let (event, distribution) = parse_bcs("TaskCreatedEvent", &bytes).expect("event parses");

        assert!(distribution.is_none());
        assert!(matches!(event, NexusEventKind::TaskCreated(_)));
    }

    #[test]
    fn resolves_event_type_origins_after_package_upgrades() {
        let mut objects = crate::test_utils::sui_mocks::mock_nexus_objects();
        objects
            .packages
            .primitives
            .insert_type_origin(
                crate::types::DatatypeKey::new("event", "EventWrapper"),
                address("0xa1"),
            )
            .unwrap();
        objects
            .packages
            .scheduler
            .insert_type_origin(
                crate::types::DatatypeKey::new("scheduler", "TaskCreatedEvent"),
                address("0xa2"),
            )
            .unwrap();
        objects
            .packages
            .interface
            .insert_type_origin(
                crate::types::DatatypeKey::new("distributed_event", "DistributedEventWrapper"),
                address("0xa3"),
            )
            .unwrap();

        let event = TaskCreatedEvent::new(
            ID::new(address("0x41")),
            TaskController::Address {
                pos0: address("0x42"),
            },
            ID::new(address("0x43")),
            7,
        );
        let bytes = bcs::to_bytes(&Wrapper { event }).expect("event serializes");
        let inner = crate::move_bindings::struct_tag::<TaskCreatedEvent>(&objects);
        let wrapper =
            crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(&objects);
        let wrapper = sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        );

        let emitter = objects.packages.scheduler.storage_id;
        let candidate = classify_nexus_event(
            0,
            sui::types::Digest::ZERO,
            emitter,
            &bytes,
            &wrapper,
            &objects,
        )
        .expect("event decoding succeeds")
        .expect("event is recognized");
        let decoded = candidate
            .into_supported()
            .expect("known event should be supported");

        assert!(matches!(decoded.data, NexusEventKind::TaskCreated(_)));
        assert!(decoded.was_emitted_by(&objects));
        let mut stale = decoded.clone();
        stale.emitting_package = objects.packages.scheduler.initial_id;
        objects.packages.scheduler.storage_id = address("0xb2");
        assert!(!stale.was_emitted_by(&objects));
        let distributed_wrapper = crate::move_bindings::struct_tag::<
            distributed_event_move::DistributedEventWrapper<MoveNexusData>,
        >(&objects);
        assert_eq!(*distributed_wrapper.address(), address("0xa3"));
    }

    #[test]
    fn parses_distributed_event_wrapper() {
        let event = OccurrenceAdvertisedEvent::new(
            ID::new(address("0x51")),
            3,
            100,
            MoveOption::from_option(Some(200)),
            20,
            OccurrenceSource::Standalone,
        );
        let pickup_task_id = crate::move_bindings::derive_task_execution_id(address("0x51"), 3)
            .expect("execution identity derives");
        let bytes = bcs::to_bytes(&DistributedWrapper {
            event,
            deadline_ms: 30_000,
            requested_at_ms: 90,
            task_id: pickup_task_id,
            leaders: vec![address("0x52")],
        })
        .expect("event serializes");

        let (event, distribution) =
            parse_bcs("OccurrenceAdvertisedEvent", &bytes).expect("event parses");

        assert!(matches!(event, NexusEventKind::OccurrenceAdvertised(_)));
        let distribution = distribution.expect("distribution metadata");
        assert_eq!(distribution.task_id, pickup_task_id);
        assert_eq!(distribution.leaders, [address("0x52")]);
    }
}
