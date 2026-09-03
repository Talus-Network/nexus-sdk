//! Typed query for Nexus events.

use {
    super::{
        event_struct_tag,
        parsing::classify_nexus_event,
        supports_event,
        NexusEventCandidate,
        NexusEventDecodeError,
    },
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

/// Known direct events ordered by their measured frequency in execution
/// workloads. A filtered consumer can exclude the first eight events it does
/// not use while retaining both wrapper heads inside Sui's ten literal
/// default. The generated event catalog validates these names, while their
/// order remains explicit because the IR does not contain workload frequency.
const DIRECT_FILTER_EXCLUSION_CANDIDATES: [&str; 8] = [
    "ExecutionPaymentFeesRecordedEvent",
    "GasPaymentConsumedEvent",
    "InvocationLockedEvent",
    "InvocationSettledEvent",
    "PriorityFeeDepositCreatedEvent",
    "ToolVerificationResolvedEvent",
    "ExecutionPaymentToolCostSnapshottedEvent",
    "TaskCreatedEvent",
];

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
    supported_event_filter: Option<fn(&str) -> bool>,
}

impl NexusEventQuery {
    /// Creates a query whose server filter uses `filter_context`.
    pub fn new(filter_context: Arc<NexusContext>, decoder: NexusEventDecoder) -> Self {
        Self {
            filter_context,
            decoder,
            supported_event_filter: None,
        }
    }

    /// Keeps compiled protocol events accepted by `filter`.
    ///
    /// Event names absent from the compiled protocol catalog are still
    /// decoded. This preserves the fail closed behavior for a compatible
    /// package that emits an event unknown to this SDK.
    pub fn with_supported_event_filter(mut self, filter: fn(&str) -> bool) -> Self {
        self.supported_event_filter = Some(filter);
        self
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
        nexus_server_filter(&self.filter_context, self.supported_event_filter)
    }

    fn read_mask(&self) -> sui::grpc::FieldMask {
        sui::grpc::FieldMask::from_paths(["package_id", "event_type", "contents"])
    }

    async fn decode(&self, event: sui::grpc::Event) -> Result<Option<Self::Output>, Self::Error> {
        let wrapper_type = event
            .event_type_opt()
            .ok_or(NexusEventDecodeError::MissingField("event_type"))?
            .parse()?;
        if self
            .supported_event_filter
            .is_some_and(|filter| !should_decode_event(&wrapper_type, filter))
        {
            return Ok(None);
        }
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

fn nexus_server_filter(
    context: &NexusContext,
    supported_event_filter: Option<fn(&str) -> bool>,
) -> sui::grpc::EventFilter {
    let wrapper =
        crate::move_bindings::struct_tag::<event_move::EventWrapper<MoveNexusData>>(context);
    let distributed_wrapper = crate::move_bindings::struct_tag::<
        distributed_event_move::DistributedEventWrapper<MoveNexusData>,
    >(context);

    let mut direct = vec![event_filter::event_type(wrapper_head(&wrapper))];
    if let Some(filter) = supported_event_filter {
        direct.extend(
            DIRECT_FILTER_EXCLUSION_CANDIDATES
                .into_iter()
                .filter(|name| !filter(name))
                .filter_map(|name| event_struct_tag(context, name))
                .map(|event| {
                    event_filter::event_type(instantiate_wrapper(&wrapper, event)).negate()
                }),
        );
    }

    sui::grpc::EventFilter::any([
        sui::grpc::EventTerm::all(direct),
        sui::grpc::EventTerm::all([event_filter::event_type(wrapper_head(&distributed_wrapper))]),
    ])
}

fn wrapper_head(wrapper: &sui::types::StructTag) -> String {
    format!(
        "{}::{}::{}",
        wrapper.address(),
        wrapper.module(),
        wrapper.name()
    )
}

fn instantiate_wrapper(
    wrapper: &sui::types::StructTag,
    event: sui::types::StructTag,
) -> sui::types::StructTag {
    sui::types::StructTag::new(
        *wrapper.address(),
        wrapper.module().clone(),
        wrapper.name().clone(),
        vec![sui::types::TypeTag::Struct(Box::new(event))],
    )
}

fn wrapped_event_name(wrapper: &sui::types::StructTag) -> Option<&str> {
    let sui::types::TypeTag::Struct(event) = wrapper.type_params().first()? else {
        return None;
    };
    Some(event.name().as_str())
}

fn should_decode_event(wrapper: &sui::types::StructTag, filter: fn(&str) -> bool) -> bool {
    wrapped_event_name(wrapper).is_none_or(|name| !supports_event(name) || filter(name))
}

/// [`EventIngestor`] configured by [`NexusEventQuery`].
pub type NexusEventIngestor = EventIngestor<NexusEventQuery>;

#[cfg(test)]
mod tests {
    use super::*;

    fn event_wrapper(name: &str) -> sui::types::StructTag {
        sui::types::StructTag::new(
            sui::types::Address::from_static("0x1"),
            "event".parse().unwrap(),
            "EventWrapper".parse().unwrap(),
            vec![sui::types::TypeTag::Struct(Box::new(
                sui::types::StructTag::new(
                    sui::types::Address::from_static("0x2"),
                    "events".parse().unwrap(),
                    name.parse().unwrap(),
                    Vec::new(),
                ),
            ))],
        )
    }

    #[test]
    fn extracts_wrapped_event_name() {
        assert_eq!(
            wrapped_event_name(&event_wrapper("TaskCreatedEvent")),
            Some("TaskCreatedEvent")
        );
    }

    #[test]
    fn filter_omits_known_events_but_retains_unknown_events() {
        let only_tasks = |name: &str| name == "TaskCreatedEvent";

        assert!(should_decode_event(
            &event_wrapper("TaskCreatedEvent"),
            only_tasks
        ));
        assert!(!should_decode_event(
            &event_wrapper("DAGCreatedEvent"),
            only_tasks
        ));
        assert!(should_decode_event(
            &event_wrapper("FutureProtocolEvent"),
            only_tasks
        ));
    }

    #[test]
    fn direct_filter_exclusions_match_the_generated_event_catalog() {
        for (index, name) in DIRECT_FILTER_EXCLUSION_CANDIDATES.iter().enumerate() {
            assert!(
                crate::events::NexusEventKind::NAMES.contains(name),
                "{name} is absent from the generated event catalog"
            );
            assert!(
                !DIRECT_FILTER_EXCLUSION_CANDIDATES[..index].contains(name),
                "{name} appears more than once"
            );
        }
    }

    #[cfg(feature = "test_utils")]
    #[test]
    fn server_filter_uses_the_portable_literal_budget() {
        let context = crate::test_utils::sui_mocks::mock_nexus_context();
        let filter = nexus_server_filter(&context, Some(|_| false));

        assert_eq!(filter.terms.len(), 2);
        assert_eq!(
            filter
                .terms
                .iter()
                .map(|term| term.literals.len())
                .sum::<usize>(),
            10
        );
        assert!(!filter.terms[0].literals[0].negated);
        assert!(filter.terms[0].literals[1..]
            .iter()
            .all(|literal| literal.negated));
        for literal in &filter.terms[0].literals[1..] {
            let Some(sui::grpc::event_literal::Predicate::EventType(event_type)) =
                &literal.predicate
            else {
                panic!("exclusion is not an event type")
            };
            let wrapper: sui::types::StructTag = event_type
                .event_type
                .as_deref()
                .expect("event type is present")
                .parse()
                .expect("event type is valid");
            let Some(sui::types::TypeTag::Struct(event)) = wrapper.type_params().first() else {
                panic!("excluded wrapper has no inner event")
            };
            assert_ne!(*event.address(), sui::types::Address::ZERO);
        }
        assert_eq!(filter.terms[1].literals.len(), 1);
        assert!(!filter.terms[1].literals[0].negated);
    }

    #[cfg(feature = "test_utils")]
    #[test]
    fn unfiltered_query_retains_both_wrapper_heads_without_exclusions() {
        let context = crate::test_utils::sui_mocks::mock_nexus_context();
        let filter = nexus_server_filter(&context, None);

        assert_eq!(filter.terms.len(), 2);
        assert!(filter
            .terms
            .iter()
            .all(|term| term.literals.len() == 1 && !term.literals[0].negated));
    }
}
