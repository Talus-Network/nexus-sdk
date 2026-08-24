//! Typed query for Nexus events.

use {
    super::{parsing::classify_nexus_event, NexusEventCandidate, NexusEventDecodeError},
    crate::{
        events::NexusEvent,
        move_bindings::{
            interface::distributed_event as distributed_event_move,
            primitives::{data::NexusData as MoveNexusData, event as event_move},
        },
        nexus::state::StateResolver,
        sui::{
            self,
            events::{EventIngestor, EventQuery},
        },
        types::{NexusContext, NexusObjects},
    },
    std::sync::Arc,
    sui_rpc::{field::FieldMaskUtil as _, proto::sui::rpc::v2::filter::event as event_filter},
};

/// Decodes events using the immutable package graph selected by each emitter.
///
/// This value does not resolve live object authority. It uses emitter package
/// metadata only to select the historical event layout.
#[derive(Clone)]
pub struct NexusEventDecoder {
    resolver: StateResolver,
    objects: Arc<NexusObjects>,
}

impl NexusEventDecoder {
    /// Creates a decoder backed by `resolver` and stable environment identity.
    pub fn new(resolver: StateResolver, objects: Arc<NexusObjects>) -> Self {
        Self { resolver, objects }
    }

    /// Decodes one Sui transaction event using its emitter package graph.
    ///
    /// Returns [`None`] when the event is not a Nexus event.
    ///
    /// # Errors
    ///
    /// Returns [`NexusEventDecodeError`] when emitter metadata cannot be
    /// resolved, event contents cannot be decoded, or the inner event type is
    /// unsupported.
    pub async fn decode_sui_event(
        &self,
        index: u64,
        digest: sui::types::Digest,
        event: &sui::types::Event,
    ) -> Result<Option<NexusEvent>, NexusEventDecodeError> {
        self.decode_parts(
            index,
            digest,
            event.package_id,
            &event.type_,
            &event.contents,
        )
        .await
    }

    async fn decode_parts(
        &self,
        index: u64,
        digest: sui::types::Digest,
        emitting_package: sui::types::Address,
        wrapper_type: &sui::types::StructTag,
        contents: &[u8],
    ) -> Result<Option<NexusEvent>, NexusEventDecodeError> {
        if !crate::move_bindings::struct_shape_matches::<event_move::EventWrapper<MoveNexusData>>(
            wrapper_type,
        ) && !crate::move_bindings::struct_shape_matches::<
            distributed_event_move::DistributedEventWrapper<MoveNexusData>,
        >(wrapper_type)
        {
            return Ok(None);
        }
        let context = self
            .resolver
            .resolve_emitter_context(Arc::clone(&self.objects), emitting_package)
            .await
            .map_err(|source| NexusEventDecodeError::EmitterPackage {
                package: emitting_package,
                source,
            })?;
        match classify_parts(
            &context,
            index,
            digest,
            emitting_package,
            wrapper_type,
            contents,
        )? {
            Some(candidate) => candidate
                .into_supported()
                .map(Some)
                .map_err(NexusEventDecodeError::from),
            None => Ok(None),
        }
    }
}

/// Query that filters event wrappers using one current package graph.
///
/// Event emitter metadata chooses historical code identity. Live execution
/// authority must be resolved separately from current object witnesses.
#[derive(Clone)]
pub struct NexusEventQuery {
    filter_context: Arc<NexusContext>,
    decoder: NexusEventDecoder,
}

impl NexusEventQuery {
    /// Creates a query whose server filter uses `filter_context`.
    pub fn new(filter_context: Arc<NexusContext>, decoder: NexusEventDecoder) -> Self {
        Self {
            filter_context,
            decoder,
        }
    }

    /// Decodes one Sui transaction event using this query.
    ///
    /// Returns [`None`] when the event is not a Nexus event.
    ///
    /// # Errors
    ///
    /// Returns [`NexusEventDecodeError`] when event contents cannot be decoded
    /// or the inner Nexus event type is unsupported.
    pub async fn decode_sui_event(
        &self,
        index: u64,
        digest: sui::types::Digest,
        event: &sui::types::Event,
    ) -> Result<Option<NexusEvent>, NexusEventDecodeError> {
        self.decoder.decode_sui_event(index, digest, event).await
    }
}

fn classify_parts(
    context: &NexusContext,
    index: u64,
    digest: sui::types::Digest,
    source_package: sui::types::Address,
    wrapper_type: &sui::types::StructTag,
    contents: &[u8],
) -> Result<Option<NexusEventCandidate>, NexusEventDecodeError> {
    classify_nexus_event(
        index,
        digest,
        source_package,
        contents,
        wrapper_type,
        context,
    )
}

impl EventQuery for NexusEventQuery {
    type Error = NexusEventDecodeError;
    type Output = NexusEventCandidate;

    fn filter(&self) -> sui::grpc::EventFilter {
        let wrapper = crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(
            &self.filter_context,
        );
        let distributed_wrapper = crate::move_bindings::struct_tag::<
            distributed_event_move::DistributedEventWrapper<MoveNexusData>,
        >(&self.filter_context);

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
        sui::grpc::FieldMask::from_paths(["package_id", "event_type", "contents"])
    }

    async fn decode(&self, event: sui::grpc::Event) -> Result<Option<Self::Output>, Self::Error> {
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
        let emitting_package = event
            .package_id
            .as_deref()
            .ok_or(NexusEventDecodeError::MissingField("package_id"))?
            .parse()
            .map_err(|error| {
                NexusEventDecodeError::Identity(format!("emitting package is invalid: {error}"))
            })?;

        let context = self
            .decoder
            .resolver
            .resolve_emitter_context(Arc::clone(&self.decoder.objects), emitting_package)
            .await
            .map_err(|source| NexusEventDecodeError::EmitterPackage {
                package: emitting_package,
                source,
            })?;
        classify_parts(
            &context,
            event_index.into(),
            digest,
            emitting_package,
            &wrapper_type,
            contents,
        )
    }
}

/// [`EventIngestor`] configured by [`NexusEventQuery`].
pub type NexusEventIngestor = EventIngestor<NexusEventQuery>;
