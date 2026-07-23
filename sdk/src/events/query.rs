//! Typed query for Nexus events.

use {
    super::parsing::{classify_nexus_event, decode_nexus_event, NexusEventClassification},
    crate::{
        events::NexusEvent,
        move_bindings::primitives::{
            data::NexusData as MoveNexusData,
            distributed_event as distributed_event_move,
            event as event_move,
        },
        sui::{
            self,
            events::{EventIngestor, EventQuery},
        },
        types::NexusObjects,
    },
    std::sync::Arc,
    sui_rpc::{field::FieldMaskUtil as _, proto::sui::rpc::v2::filter::event as event_filter},
    thiserror::Error,
};

/// Failure returned by [`NexusEventQuery`] while converting a Sui event.
#[derive(Debug, Error)]
pub enum NexusEventDecodeError {
    /// Required event identity is missing or invalid.
    #[error("Nexus event identity is invalid: {0}")]
    Identity(String),
    /// A required Sui event field is missing.
    #[error("Required Nexus event field is missing: {0}")]
    MissingField(&'static str),
    /// The Sui event type is invalid.
    #[error("Nexus event type is invalid: {0}")]
    EventType(#[from] sui::types::TypeParseError),
    /// The Nexus wrapper or its contents are invalid.
    #[error("Nexus event contents are invalid: {0}")]
    Nexus(#[source] anyhow::Error),
}

/// Query that selects and decodes events for one Nexus deployment.
#[derive(Clone)]
pub struct NexusEventQuery {
    objects: Arc<NexusObjects>,
}

impl NexusEventQuery {
    /// Creates a query for `objects`.
    pub fn new(objects: Arc<NexusObjects>) -> Self {
        Self { objects }
    }
}

impl EventQuery for NexusEventQuery {
    type Error = NexusEventDecodeError;
    type Output = NexusEvent;

    fn filter(&self) -> sui::grpc::EventFilter {
        let wrapper = crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(
            &self.objects,
        );
        let distributed_wrapper = crate::move_bindings::struct_tag::<
            distributed_event_move::DistributedEventWrapper<MoveNexusData>,
        >(&self.objects);

        sui::grpc::EventFilter::any([wrapper, distributed_wrapper].map(|tag| {
            event_filter::event_type(format!(
                "{}::{}::{}",
                tag.address(),
                tag.module(),
                tag.name()
            ))
        }))
    }

    fn read_mask(&self) -> sui::grpc::FieldMask {
        sui::grpc::FieldMask::from_paths(["event_type", "contents"])
    }

    fn decode(&self, event: sui::grpc::Event) -> Result<Option<Self::Output>, Self::Error> {
        let wrapper_type = event
            .event_type_opt()
            .ok_or(NexusEventDecodeError::MissingField("event_type"))?
            .parse()?;
        let (event_type, event_name) = match classify_nexus_event(&wrapper_type, &self.objects)
            .map_err(NexusEventDecodeError::Nexus)?
        {
            NexusEventClassification::Decode {
                event_type,
                event_name,
            } => (event_type, event_name),
            NexusEventClassification::Ignore(_) => return Ok(None),
        };
        let digest = event
            .transaction_digest
            .as_deref()
            .ok_or_else(|| {
                NexusEventDecodeError::Identity("transaction digest is missing".to_owned())
            })?
            .parse()
            .map_err(|error| {
                NexusEventDecodeError::Identity(format!("transaction digest is invalid: {error}"))
            })?;
        let event_index = event
            .event_index
            .ok_or_else(|| NexusEventDecodeError::Identity("event index is missing".to_owned()))?;
        let contents = event
            .contents_opt()
            .and_then(|contents| contents.value_opt())
            .ok_or(NexusEventDecodeError::MissingField("contents"))?;
        let event = decode_nexus_event(
            event_index.into(),
            digest,
            contents,
            event_type,
            &event_name,
        )
        .map_err(NexusEventDecodeError::Nexus)?;

        Ok(Some(event))
    }
}

/// [`EventIngestor`] configured by [`NexusEventQuery`].
pub type NexusEventIngestor = EventIngestor<NexusEventQuery>;
