//! Agent object inputs for programmable transaction builders.

use {
    crate::{move_boundary, sui},
    sui::types::Argument,
};

/// Already-resolved agent object input accepted by SDK transaction builders.
///
/// The type separates ownership classification from the Move borrow a builder needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentInput {
    Owned(sui::types::ObjectReference),
    Shared(sui::types::ObjectReference),
    Immutable(sui::types::ObjectReference),
}

impl AgentInput {
    /// Export this object as a mutable generated-boundary PTB argument.
    pub(crate) fn mutable_ptb_argument(
        self,
        tx: &mut move_boundary::NexusPtbBuilder<'_>,
    ) -> anyhow::Result<Argument> {
        match self {
            Self::Owned(object) => Ok(tx.owned_object(&object)?),
            Self::Shared(object) => Ok(tx.shared_object(&object, true)?),
            Self::Immutable(object) => Err(anyhow::anyhow!(
                "agent '{}' is immutable and cannot be used where a mutable agent reference is required",
                object.object_id()
            )),
        }
    }

    /// Export this object as an immutable generated-boundary PTB argument.
    pub(crate) fn immutable_ptb_argument(
        self,
        tx: &mut move_boundary::NexusPtbBuilder<'_>,
    ) -> anyhow::Result<Argument> {
        match self {
            Self::Owned(object) | Self::Immutable(object) => Ok(tx.owned_object(&object)?),
            Self::Shared(object) => Ok(tx.shared_object(&object, false)?),
        }
    }
}
