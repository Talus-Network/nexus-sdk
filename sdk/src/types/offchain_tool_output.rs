//! Ordered active-v3 off-chain Tool output body.

use {
    crate::move_bindings::primitives::data::NexusValue,
    serde::{Deserialize, Serialize},
};

/// One producer-named output port in the signed HTTP v3 body.
///
/// This type retains the Tool producer's raw name and witness group until MetaSchema validation;
/// it is not a second stored `NexusData` representation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffchainToolOutputPort {
    pub port_name: Vec<u8>,
    pub values: Vec<NexusValue>,
}

/// Schema-ordered Tool output serialized directly as the signed HTTP v3 body.
///
/// The generated Move `TaggedOutput` contains named stored `NexusData`, so reusing it here would change the authenticated bytes and prevent the Leader from carrying decoded witnesses into the typed on-chain boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffchainToolOutput {
    pub tag: Vec<u8>,
    pub ports: Vec<OffchainToolOutputPort>,
}
