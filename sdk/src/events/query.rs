//! Typed query for Nexus events.

use {
    super::parsing::decode_nexus_event,
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
    /// The Nexus event contents are invalid.
    #[error("Nexus event contents are invalid: {0}")]
    Contents(#[source] anyhow::Error),
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

    /// Decodes one Sui transaction event using this query.
    ///
    /// Returns [`None`] when the wrapper contains an unsupported event type.
    ///
    /// # Errors
    ///
    /// Returns [`NexusEventDecodeError`] when supported event contents cannot
    /// be decoded.
    pub fn decode_sui_event(
        &self,
        index: u64,
        digest: sui::types::Digest,
        event: &sui::types::Event,
    ) -> Result<Option<NexusEvent>, NexusEventDecodeError> {
        self.decode_parts(index, digest, &event.type_, &event.contents)
    }

    fn decode_parts(
        &self,
        index: u64,
        digest: sui::types::Digest,
        wrapper_type: &sui::types::StructTag,
        contents: &[u8],
    ) -> Result<Option<NexusEvent>, NexusEventDecodeError> {
        decode_nexus_event(index, digest, contents, wrapper_type, &self.objects)
            .map_err(NexusEventDecodeError::Contents)
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

        self.decode_parts(event_index.into(), digest, &wrapper_type, contents)
    }
}

/// [`EventIngestor`] configured by [`NexusEventQuery`].
pub type NexusEventIngestor = EventIngestor<NexusEventQuery>;
