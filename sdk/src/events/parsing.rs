//! Nexus event type selection and decoding.

use crate::{
    events::{parse_bcs, supports_event, NexusEvent},
    move_bindings::primitives::{
        data::NexusData as MoveNexusData,
        distributed_event as distributed_event_move,
        event as event_move,
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

        let name = tag.name().as_str();
        supports_event(name).then_some(Self { tag, name })
    }
}

pub(super) fn decode_nexus_event(
    index: u64,
    digest: sui::types::Digest,
    contents: &[u8],
    wrapper_type: &sui::types::StructTag,
    objects: &NexusObjects,
) -> anyhow::Result<Option<NexusEvent>> {
    let Some(event_type) = NexusEventType::resolve(wrapper_type, objects) else {
        return Ok(None);
    };
    let (data, distribution) = parse_bcs(event_type.name, contents)?;

    Ok(Some(NexusEvent {
        id: (digest, index),
        generics: event_type.tag.type_params().to_vec(),
        data,
        distribution,
    }))
}

fn is_nexus_package(address: sui::types::Address, objects: &NexusObjects) -> bool {
    address == objects.primitives_pkg_id
        || address == objects.interface_pkg_id
        || address == objects.registry_pkg_id
        || objects.is_scheduler_package(address)
        || objects.is_workflow_package(address)
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
                    scheduler::{OccurrenceAdvertised, TaskCreated},
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
    fn parses_direct_task_event() {
        let task_id = ID::new(address("0x41"));
        let event = TaskCreated::new(
            task_id,
            TaskController::Address {
                pos0: address("0x42"),
            },
            ID::new(address("0x43")),
            7,
        );
        let bytes = bcs::to_bytes(&Wrapper { event }).expect("event serializes");

        let (event, distribution) = parse_bcs("TaskCreated", &bytes).expect("event parses");

        assert!(distribution.is_none());
        assert!(matches!(event, NexusEventKind::TaskCreated(_)));
    }

    #[test]
    fn parses_distributed_occurrence_advertisement() {
        let event = OccurrenceAdvertised::new(
            ID::new(address("0x51")),
            3,
            100,
            MoveOption::from_option(Some(200)),
            20,
            OccurrenceSource::Manual,
        );
        let pickup_task_id =
            crate::move_bindings::derive_occurrence_advertisement_task_id(address("0x51"), &3)
                .expect("pickup identity derives");
        let bytes = bcs::to_bytes(&DistributedWrapper {
            event,
            deadline_ms: 30_000,
            requested_at_ms: 90,
            task_id: pickup_task_id,
            leaders: vec![address("0x52")],
        })
        .expect("event serializes");

        let (event, distribution) =
            parse_bcs("OccurrenceAdvertised", &bytes).expect("event parses");

        assert!(matches!(event, NexusEventKind::OccurrenceAdvertised(_)));
        let distribution = distribution.expect("distribution metadata");
        assert_eq!(distribution.task_id, pickup_task_id);
        assert_eq!(distribution.leaders, [address("0x52")]);
    }
}
