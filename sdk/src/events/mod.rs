use {
    crate::sui,
    anyhow::{bail, Result},
    serde::{Deserialize, Serialize},
    thiserror::Error,
};

mod parsing;
mod query;

pub use query::*;

/// Nexus event whose datatype is absent from [`NexusEventKind`].
#[derive(Clone, Debug, Error)]
#[error("Nexus event '{event_type}' from source package '{source_package}' is unsupported")]
pub struct UnsupportedNexusEvent {
    /// Transaction digest and event sequence that identify the event.
    pub id: (sui::types::Digest, u64),
    /// Package recorded by Sui as the source of the Move command.
    pub source_package: sui::types::Address,
    /// Full inner event type. Its address identifies the definition package.
    pub event_type: Box<sui::types::StructTag>,
}

/// Failure returned while converting a Sui event into a [`NexusEvent`].
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
    /// The immutable package graph selected by the emitter is invalid.
    #[error("Could not resolve Nexus event emitter package '{package}': {source}")]
    EmitterPackage {
        /// Package recorded as the event emitter.
        package: sui::types::Address,
        /// Resolver failure for that package.
        #[source]
        source: crate::nexus::error::NexusError,
    },
    /// The inner datatype belongs to Nexus but is absent from the SDK contract.
    #[error(transparent)]
    UnsupportedEvent(#[from] UnsupportedNexusEvent),
    /// The Nexus event contents are invalid.
    #[error("Nexus event contents are invalid: {0}")]
    Contents(#[source] anyhow::Error),
}

fn deserialize_u64_to_datetime<'de, D>(
    deserializer: D,
) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let timestamp = u64::deserialize(deserializer)?;
    chrono::DateTime::from_timestamp_millis(timestamp as i64)
        .ok_or_else(|| serde::de::Error::custom("datetime out of range"))
}

fn serialize_datetime_to_u64<S>(
    value: &chrono::DateTime<chrono::Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(value.timestamp_millis() as u64)
}

fn deserialize_u64_to_duration<'de, D>(deserializer: D) -> Result<chrono::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let millis = u64::deserialize(deserializer)?;
    Ok(chrono::Duration::milliseconds(millis as i64))
}

fn serialize_duration_to_u64<S>(value: &chrono::Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(value.num_milliseconds() as u64)
}

/// Distribution metadata for distributed events. This contains metadata about
/// the event deadline as well as the priority in which leaders should attempt
/// to execute the event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistributedEventMetadata {
    /// The execution window duration.
    #[serde(
        rename = "deadline_ms",
        deserialize_with = "deserialize_u64_to_duration",
        serialize_with = "serialize_duration_to_u64"
    )]
    pub deadline: chrono::Duration,
    /// The timestamp by which the event was requested.
    #[serde(
        rename = "requested_at_ms",
        deserialize_with = "deserialize_u64_to_datetime",
        serialize_with = "serialize_datetime_to_u64"
    )]
    pub requested_at: chrono::DateTime<chrono::Utc>,
    /// The priority list of leader addresses.
    pub leaders: Vec<sui::types::Address>,
    /// The task ID.
    pub task_id: sui::types::Address,
}

/// Struct holding the Sui event ID, the event generic arguments and the data
/// as one of [NexusEventKind].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NexusEvent {
    /// The event transaction digest and event sequence.
    pub id: (sui::types::Digest, u64),
    /// Package containing the top level Move function that emitted the event.
    ///
    /// This identifies runtime code and is intentionally distinct from the
    /// event datatype origin encoded in its type tag.
    pub emitting_package: sui::types::Address,
    /// If the `T in NexusEvent<T>` is also a generic, this field holds the
    /// generic type. Note that this can be nested indefinitely.
    pub generics: Vec<sui::types::TypeTag>,
    /// The event data.
    pub data: NexusEventKind,
    /// If the event is a distributed event, this field holds the distribution
    /// metadata.
    pub distribution: Option<DistributedEventMetadata>,
}

impl NexusEvent {
    /// Returns whether this event was emitted by a package in `context`.
    ///
    /// An operation pinned to a protocol version should use the configuration
    /// captured at its boundary. This permits compatible in flight work to
    /// finish without accepting an old package for newly created work.
    pub fn was_emitted_by(&self, context: &crate::types::NexusContext) -> bool {
        context
            .packages()
            .all()
            .any(|package| package.storage_id == self.emitting_package)
    }
}

/// Nexus event selected before source compatibility policy is applied.
#[derive(Clone, Debug)]
pub enum NexusEventCandidate {
    /// Event represented by [`NexusEventKind`].
    Supported(Box<NexusEvent>),
    /// Event absent from [`NexusEventKind`].
    Unsupported(UnsupportedNexusEvent),
}

impl NexusEventCandidate {
    /// Returns the package recorded by Sui as the Move command source.
    pub fn source_package(&self) -> sui::types::Address {
        match self {
            Self::Supported(event) => event.emitting_package,
            Self::Unsupported(event) => event.source_package,
        }
    }

    /// Converts [`Self::Supported`] into its decoded event.
    ///
    /// # Errors
    ///
    /// Returns [`UnsupportedNexusEvent`] for [`Self::Unsupported`].
    pub fn into_supported(self) -> Result<NexusEvent, UnsupportedNexusEvent> {
        match self {
            Self::Supported(event) => Ok(*event),
            Self::Unsupported(event) => Err(event),
        }
    }
}

macro_rules! events {
    (
        $(
            $event_ty:ty => $variant:ident, $name:expr
        ),* $(,)?
    ) => {

        // == enum NexusEventKind ==

        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(tag = "_nexus_event_type", content = "event")]
        pub enum NexusEventKind {
            $(
                #[serde(rename = $name)]
                $variant($event_ty),
            )*
        }

        impl NexusEventKind {
            /// Move datatype names supported by [`NexusEventKind`].
            pub const NAMES: &'static [&'static str] = &[
                $($name),*
            ];

            /// Returns the name of the event kind as a string.
            pub fn name(&self) -> String {
                match self {
                    $(
                        NexusEventKind::$variant(_) => $name.to_string(),
                    )*
                }
            }
        }

        // == Parsing from BCS ==

        pub(super) fn parse_bcs(name: &str, bytes: &[u8]) -> Result<(NexusEventKind, Option<DistributedEventMetadata>)> {
            #[derive(Deserialize)]
            struct Wrapper<T> {
                event: T,
            }

            #[derive(Deserialize)]
            struct DistributedWrapper<T> {
                event: T,
                deadline_ms: u64,
                requested_at_ms: u64,
                task_id: sui::types::Address,
                leaders: Vec<sui::types::Address>,
            }

            match name {
                $(
                    $name => {
                        match bcs::from_bytes::<DistributedWrapper<$event_ty>>(bytes) {
                            Ok(distributed) => {
                                let metadata = DistributedEventMetadata {
                                    deadline: chrono::Duration::milliseconds(distributed.deadline_ms as i64),
                                    requested_at: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(distributed.requested_at_ms as i64)
                                        .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
                                    task_id: distributed.task_id,
                                    leaders: distributed.leaders,
                                };

                                Ok((NexusEventKind::$variant(distributed.event), Some(metadata)))
                            }
                            Err(_) => {
                                 let obj: Wrapper<$event_ty> = bcs::from_bytes(bytes)?;

                                 Ok((NexusEventKind::$variant(obj.event), None))
                            }
                        }
                    }
                )*
                _ => bail!("Unknown event: {}", name),
            }
        }

        pub(super) fn supports_event(name: &str) -> bool {
            matches!(name, $($name)|*)
        }

        pub(super) fn event_struct_tag(
            context: &crate::types::NexusContext,
            name: &str,
        ) -> Option<crate::sui::types::StructTag> {
            match name {
                $(
                    $name => Some(crate::move_bindings::struct_tag::<$event_ty>(context)),
                )*
                _ => None,
            }
        }
    };
}

// The SDK event contract is generated from the same committed IR that renders
// the Move bindings above.
include!(concat!(env!("OUT_DIR"), "/nexus_events.rs"));

#[cfg(test)]
mod tests {
    use {super::NexusEventKind, std::collections::BTreeSet};

    const PROTOCOL_IR: [&str; 6] = [
        include_str!("../move_bindings/ir/interface.json"),
        include_str!("../move_bindings/ir/primitives.json"),
        include_str!("../move_bindings/ir/registry.json"),
        include_str!("../move_bindings/ir/scheduler.json"),
        include_str!("../move_bindings/ir/tool.json"),
        include_str!("../move_bindings/ir/workflow.json"),
    ];

    #[test]
    fn event_catalog_matches_committed_protocol_ir() {
        let mut ir_names = PROTOCOL_IR
            .into_iter()
            .flat_map(|contents| {
                let package: serde_json::Value =
                    serde_json::from_str(contents).expect("committed IR is valid JSON");
                package["modules"]
                    .as_object()
                    .expect("IR contains modules")
                    .values()
                    .flat_map(|module| {
                        module["datatypes"]
                            .as_array()
                            .expect("IR module contains datatypes")
                            .iter()
                            .filter_map(|datatype| datatype["name"].as_str())
                            .filter(|name| name.ends_with("Event"))
                            .map(str::to_owned)
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut catalog_names = NexusEventKind::NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            ir_names.len(),
            ir_names.iter().collect::<BTreeSet<_>>().len(),
            "Move event datatype names must be unique"
        );
        assert_eq!(
            catalog_names.len(),
            catalog_names.iter().collect::<BTreeSet<_>>().len(),
            "SDK event names must be unique"
        );

        ir_names.sort();
        catalog_names.sort();
        assert_eq!(catalog_names, ir_names);
    }
}
