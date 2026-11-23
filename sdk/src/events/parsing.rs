//! Nexus event type selection and decoding.

use crate::{
    events::{
        event_struct_tag,
        parse_bcs,
        DistributedEventMetadata,
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

pub(super) enum EventWrapper {
    Direct,
    Distributed,
}

impl EventWrapper {
    fn resolve(tag: &sui::types::StructTag, context: &NexusContext) -> Option<Self> {
        use crate::move_bindings::struct_tag_matches;
        if struct_tag_matches::<event_move::EventWrapper<MoveNexusData>>(context, tag) {
            Some(Self::Direct)
        } else if struct_tag_matches::<distributed_event_move::DistributedEventWrapper<MoveNexusData>>(
            context, tag,
        ) {
            Some(Self::Distributed)
        } else {
            None
        }
    }

    pub(super) fn decode<T: serde::de::DeserializeOwned>(
        self,
        bytes: &[u8],
    ) -> anyhow::Result<(T, Option<DistributedEventMetadata>)> {
        match self {
            Self::Direct => Ok((
                bcs::from_bytes::<event_move::EventWrapper<T>>(bytes)?.event,
                None,
            )),
            Self::Distributed => {
                let wrapped: distributed_event_move::DistributedEventWrapper<T> =
                    bcs::from_bytes(bytes)?;
                Ok((
                    wrapped.event,
                    Some(DistributedEventMetadata {
                        deadline: super::duration_from_millis(wrapped.deadline_ms)?,
                        requested_at: super::datetime_from_millis(wrapped.requested_at_ms)?,
                        task_id: wrapped.task_id.address(),
                        leaders: wrapped.leaders.into_iter().map(|id| id.address()).collect(),
                    }),
                ))
            }
        }
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
    let Some(wrapper) = EventWrapper::resolve(wrapper_type, context) else {
        return Ok(None);
    };
    let [sui::types::TypeTag::Struct(tag)] = wrapper_type.type_params() else {
        return Ok(None);
    };
    let name = tag.name().as_str();
    let Some(expected) = event_struct_tag(context, name) else {
        if !name.ends_with("Event")
            || !context.packages().all().any(|package| {
                package
                    .type_origins
                    .get(tag.module().as_str())
                    .and_then(|types| types.get(name))
                    == Some(tag.address())
            })
        {
            return Ok(None);
        }
        return Ok(Some(NexusEventCandidate::Unsupported(
            UnsupportedNexusEvent {
                id: (digest, index),
                source_package: emitting_package,
                event_type: tag.clone(),
            },
        )));
    };
    if tag.address() != expected.address()
        || tag.module() != expected.module()
        || tag.type_params().len() != expected.type_params().len()
    {
        return Ok(None);
    }
    let (data, distribution) = parse_bcs(name, wrapper, contents).map_err(|error| {
        NexusEventDecodeError::Contents(error.context(format!("Could not decode {tag}")))
    })?;

    Ok(Some(NexusEventCandidate::Supported(Box::new(NexusEvent {
        id: (digest, index),
        emitting_package,
        generics: tag.type_params().to_vec(),
        data,
        distribution,
    }))))
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
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    #[test]
    fn distribution_metadata_uses_the_same_checked_times_in_bcs_and_json() {
        let task = address("0x41");
        let leaders = vec![address("0x42")];
        let max = i64::MAX as u64;
        for (deadline, timestamp, valid) in [
            (0, 0, true),
            (max, 1, true),
            (max + 1, 0, false),
            (u64::MAX, 0, false),
            (0, u64::MAX, false),
        ] {
            let bytes = bcs::to_bytes(&(7u8, deadline, timestamp, task, &leaders)).unwrap();
            let bcs = EventWrapper::Distributed
                .decode::<u8>(&bytes)
                .map(|(event, metadata)| {
                    assert_eq!(event, 7);
                    metadata.unwrap()
                });
            let json = serde_json::from_value::<DistributedEventMetadata>(serde_json::json!({
                "deadline_ms": deadline, "requested_at_ms": timestamp, "task_id": task, "leaders": leaders,
            }));
            for metadata in [bcs.ok(), json.ok()] {
                assert_eq!(
                    metadata.is_some(),
                    valid,
                    "deadline {deadline}, timestamp {timestamp}"
                );
                if let Some(metadata) = metadata {
                    assert_eq!(metadata.deadline.num_milliseconds() as u64, deadline);
                    assert_eq!(metadata.requested_at.timestamp_millis() as u64, timestamp);
                    assert_eq!(metadata.task_id, task);
                    assert_eq!(metadata.leaders, leaders);
                }
            }
        }
        assert!(EventWrapper::Distributed.decode::<u8>(&[7]).is_err());
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
        let bytes = bcs::to_bytes(&event_move::EventWrapper { event }).expect("event serializes");

        let (event, distribution) =
            parse_bcs("TaskCreatedEvent", EventWrapper::Direct, &bytes).expect("event parses");

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
        let interface = packages.interface.as_mut().unwrap();
        interface.storage_id = address("0xa7");
        interface.version = 2;
        let context = crate::types::NexusContext::new(std::sync::Arc::new(objects), packages);

        let event = TaskCreatedEvent::new(
            ID::new(address("0x41")),
            TaskController::Address {
                pos0: address("0x42"),
            },
            ID::new(address("0x43")),
            7,
        );
        let bytes = bcs::to_bytes(&event_move::EventWrapper { event }).expect("event serializes");
        let inner = crate::move_bindings::struct_tag::<TaskCreatedEvent>(&context);
        let wrapper =
            crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(&context);
        let scheduler = context
            .require_package(crate::types::PackageRole::Scheduler)
            .unwrap();
        let classify = |inner, contents: &[u8]| {
            let tag = sui::types::StructTag::new(
                *wrapper.address(),
                wrapper.module().clone(),
                wrapper.name().clone(),
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            );
            classify_nexus_event(
                0,
                sui::types::Digest::ZERO,
                scheduler.storage_id,
                contents,
                &tag,
                &context,
            )
        };
        let unrelated = sui::types::StructTag::new(
            *inner.address(),
            sui::types::Identifier::from_static("unrelated"),
            inner.name().clone(),
            vec![],
        );
        assert!(classify(unrelated, &[]).unwrap().is_none());
        let candidate = classify(inner, &bytes)
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
        assert_eq!(*distributed_wrapper.address(), address("0xa2"));
    }
}
