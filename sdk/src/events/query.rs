//! Typed query for Nexus events.

use {
    super::parsing::decode_nexus_event,
    crate::{
        events::NexusEvent,
        move_bindings::{
            interface::distributed_event as distributed_event_move,
            primitives::{
                data::NexusData as MoveNexusData,
                event as event_move,
                protocol::ProtocolVersionActivatedV1,
            },
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
        self.decode_parts(
            index,
            digest,
            event.package_id,
            &event.type_,
            &event.contents,
        )
    }

    fn decode_parts(
        &self,
        index: u64,
        digest: sui::types::Digest,
        emitting_package: sui::types::Address,
        wrapper_type: &sui::types::StructTag,
        contents: &[u8],
    ) -> Result<Option<NexusEvent>, NexusEventDecodeError> {
        decode_nexus_event(
            index,
            digest,
            emitting_package,
            contents,
            wrapper_type,
            &self.objects,
        )
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
        sui::grpc::FieldMask::from_paths(["package_id", "event_type", "contents"])
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
        let emitting_package = event
            .package_id
            .as_deref()
            .ok_or(NexusEventDecodeError::MissingField("package_id"))?
            .parse()
            .map_err(|error| {
                NexusEventDecodeError::Identity(format!("emitting package is invalid: {error}"))
            })?;

        self.decode_parts(
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

/// Canonical wake signal emitted after a protocol version is activated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolActivation {
    /// Transaction digest and event index that uniquely identify this notification.
    pub id: (sui::types::Digest, u64),
    /// Exact primitives package that emitted this notification.
    pub emitting_package: sui::types::Address,
    /// Decoded protocol activation payload.
    pub event: ProtocolVersionActivatedV1,
}

impl ProtocolActivation {
    /// Verify notification metadata against a fully resolved candidate.
    ///
    /// The candidate is independently validated by
    /// [`crate::nexus::protocol::ProtocolResolver`].
    pub fn matches_candidate(&self, candidate: &NexusObjects) -> bool {
        self.event.protocol_id.bytes == *candidate.protocol.object_id()
            && self.event.protocol_version == candidate.protocol_version
            && self.event.config_hash == candidate.config_hash
            && self.emitting_package == candidate.primitives_pkg_id()
    }
}

/// Event query for the `protocol::ProtocolVersionActivatedV1` datatype.
#[derive(Clone)]
pub struct ProtocolActivationQuery {
    objects: Arc<NexusObjects>,
}

impl ProtocolActivationQuery {
    /// Creates a query scoped to one resolved protocol configuration.
    pub fn new(objects: Arc<NexusObjects>) -> Self {
        Self { objects }
    }

    fn decode_parts(
        &self,
        index: u64,
        digest: sui::types::Digest,
        emitting_package: sui::types::Address,
        event_type: &sui::types::StructTag,
        contents: &[u8],
    ) -> Result<Option<ProtocolActivation>, NexusEventDecodeError> {
        type ActivationWrapper = event_move::EventWrapper<ProtocolVersionActivatedV1>;

        let expected = crate::move_bindings::struct_tag::<ActivationWrapper>(&self.objects);
        if event_type != &expected {
            return Ok(None);
        }
        let wrapper: ActivationWrapper = bcs::from_bytes(contents)
            .map_err(anyhow::Error::from)
            .map_err(NexusEventDecodeError::Contents)?;
        Ok(Some(ProtocolActivation {
            id: (digest, index),
            emitting_package,
            event: wrapper.event,
        }))
    }
}

impl EventQuery for ProtocolActivationQuery {
    type Error = NexusEventDecodeError;
    type Output = ProtocolActivation;

    fn filter(&self) -> sui::grpc::EventFilter {
        let wrapper = crate::move_bindings::struct_tag::<
            event_move::EventWrapper<ProtocolVersionActivatedV1>,
        >(&self.objects);
        sui::grpc::EventFilter::any([event_filter::event_type(format!(
            "{}::{}::{}",
            wrapper.address(),
            wrapper.module(),
            wrapper.name()
        ))])
    }

    fn read_mask(&self) -> sui::grpc::FieldMask {
        sui::grpc::FieldMask::from_paths(["package_id", "event_type", "contents"])
    }

    fn decode(&self, event: sui::grpc::Event) -> Result<Option<Self::Output>, Self::Error> {
        let emitting_package = event
            .package_id
            .as_deref()
            .ok_or(NexusEventDecodeError::MissingField("package_id"))?
            .parse()
            .map_err(|error| {
                NexusEventDecodeError::Identity(format!("emitting package is invalid: {error}"))
            })?;
        let event_type = event
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
        self.decode_parts(
            event_index.into(),
            digest,
            emitting_package,
            &event_type,
            contents,
        )
    }
}

/// [`EventIngestor`] configured for protocol activation notifications.
pub type ProtocolActivationIngestor = EventIngestor<ProtocolActivationQuery>;

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::move_bindings::{
            primitives::{event::EventWrapper, protocol::ProtocolVersionActivatedV1},
            sui_framework::object::ID,
        },
    };

    fn activation_fixture() -> (NexusObjects, ProtocolActivation) {
        let mut objects = crate::test_utils::sui_mocks::mock_nexus_objects();
        objects.protocol_version = 2;
        objects.config_hash = vec![7; 32];
        let activation = ProtocolActivation {
            id: (sui::types::Digest::ZERO, 0),
            emitting_package: objects.primitives_pkg_id(),
            event: ProtocolVersionActivatedV1::new(
                ID::new(*objects.protocol.object_id()),
                objects.protocol_version,
                objects.config_hash.clone(),
            ),
        };

        (objects, activation)
    }

    #[test]
    fn activation_requires_candidate_protocol_version_hash_and_emitter() {
        let (objects, activation) = activation_fixture();
        assert!(activation.matches_candidate(&objects));

        let mut spoofed = activation.clone();
        spoofed.emitting_package = sui::types::Address::ZERO;
        assert!(!spoofed.matches_candidate(&objects));
    }

    #[test]
    fn activation_query_decodes_the_nexus_event_wrapper() {
        let (objects, activation) = activation_fixture();
        let objects = Arc::new(objects);
        let wrapper: EventWrapper<ProtocolVersionActivatedV1> =
            EventWrapper::new(activation.event.clone());
        let wrapper_type =
            crate::move_bindings::struct_tag::<EventWrapper<ProtocolVersionActivatedV1>>(&objects);
        let decoded = ProtocolActivationQuery::new(Arc::clone(&objects))
            .decode_parts(
                activation.id.1,
                activation.id.0,
                activation.emitting_package,
                &wrapper_type,
                &bcs::to_bytes(&wrapper).expect("wrapper should serialize"),
            )
            .expect("wrapper should decode")
            .expect("activation wrapper should match");

        assert_eq!(decoded, activation);
        assert!(decoded.matches_candidate(&objects));
    }

    #[test]
    fn activation_query_ignores_other_nexus_event_wrappers() {
        let (objects, activation) = activation_fixture();
        let objects = Arc::new(objects);
        let other_wrapper =
            crate::move_bindings::struct_tag::<EventWrapper<MoveNexusData>>(&objects);
        let decoded = ProtocolActivationQuery::new(objects)
            .decode_parts(
                activation.id.1,
                activation.id.0,
                activation.emitting_package,
                &other_wrapper,
                &[],
            )
            .expect("unrelated wrapper should be ignored");

        assert!(decoded.is_none());
    }
}
