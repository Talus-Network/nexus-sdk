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
    types::NexusContext,
};

struct NexusEventType<'a> {
    tag: &'a sui::types::StructTag,
    name: &'a str,
}

impl<'a> NexusEventType<'a> {
    fn resolve(wrapper_type: &'a sui::types::StructTag, context: &NexusContext) -> Option<Self> {
        if !is_event_wrapper(wrapper_type, context) {
            return None;
        }

        let tag = wrapper_type
            .type_params()
            .first()
            .and_then(|tag| match tag {
                sui::types::TypeTag::Struct(struct_tag) => Some(struct_tag),
                _ => None,
            })?;
        if !is_nexus_package(*tag.address(), context) {
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
    context: &NexusContext,
) -> Result<Option<NexusEventCandidate>, NexusEventDecodeError> {
    let Some(event_type) = NexusEventType::resolve(wrapper_type, context) else {
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

fn is_nexus_package(address: sui::types::Address, context: &NexusContext) -> bool {
    context.packages().contains_package(address)
}

fn is_event_wrapper(tag: &sui::types::StructTag, context: &NexusContext) -> bool {
    crate::move_bindings::struct_tag_matches::<event_move::EventWrapper<MoveNexusData>>(
        context, tag,
    ) || crate::move_bindings::struct_tag_matches::<
        distributed_event_move::DistributedEventWrapper<MoveNexusData>,
    >(context, tag)
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            events::NexusEventKind,
            move_bindings::{
                scheduler::{scheduler::TaskCreatedEvent, task::TaskController},
                sui_framework::object::ID,
            },
        },
        serde::Serialize,
    };

    #[derive(Serialize)]
    struct Wrapper<T> {
        event: T,
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
        let objects = crate::test_utils::sui_mocks::mock_nexus_objects();
        let mut packages = crate::test_utils::sui_mocks::mock_nexus_packages();
        packages
            .primitives
            .as_mut()
            .unwrap()
            .insert_type_origin(
                crate::types::DatatypeKey::new("event", "EventWrapper"),
                address("0xa1"),
            )
            .unwrap();
        packages
            .scheduler
            .as_mut()
            .unwrap()
            .insert_type_origin(
                crate::types::DatatypeKey::new("scheduler", "TaskCreatedEvent"),
                address("0xa2"),
            )
            .unwrap();
        let scheduler = packages.scheduler.as_mut().unwrap();
        scheduler.storage_id = address("0xa6");
        scheduler.version = 2;
        packages
            .interface
            .as_mut()
            .unwrap()
            .insert_type_origin(
                crate::types::DatatypeKey::new("distributed_event", "DistributedEventWrapper"),
                address("0xa3"),
            )
            .unwrap();
        let context = crate::types::NexusContext::new(std::sync::Arc::new(objects), packages);

        let event = TaskCreatedEvent::new(
            ID::new(address("0x41")),
            TaskController::Address {
                pos0: address("0x42"),
            },
            ID::new(address("0x43")),
            7,
        );
        let bytes = bcs::to_bytes(&Wrapper { event }).expect("event serializes");
        let inner = crate::move_bindings::struct_tag::<TaskCreatedEvent>(&context);
        let wrapper =
            crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(&context);
        let wrapper = sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        );

        let scheduler = context
            .require_package(crate::types::PackageRole::Scheduler)
            .unwrap();
        let emitter = scheduler.storage_id;
        let candidate = classify_nexus_event(
            0,
            sui::types::Digest::ZERO,
            emitter,
            &bytes,
            &wrapper,
            &context,
        )
        .expect("event decoding succeeds")
        .expect("event is recognized");
        let decoded = candidate
            .into_supported()
            .expect("known event should be supported");

        assert!(matches!(decoded.data, NexusEventKind::TaskCreated(_)));
        assert!(decoded.was_emitted_by(&context));
        let mut stale = decoded.clone();
        stale.emitting_package = scheduler.initial_id;
        assert!(!stale.was_emitted_by(&context));
        let distributed_wrapper = crate::move_bindings::struct_tag::<
            distributed_event_move::DistributedEventWrapper<MoveNexusData>,
        >(&context);
        assert_eq!(*distributed_wrapper.address(), address("0xa3"));
    }
}
