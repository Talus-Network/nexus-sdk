//! Common access to stable Move object anchors backed by `sui::versioned`.

use crate::{move_bindings::sui_framework::versioned::Versioned, sui};

/// A stable object anchor whose replaceable payload lives below [`Versioned`].
pub trait VersionedAnchor {
    /// Stable object identity embedded in the Move value.
    fn object_id(&self) -> sui::types::Address;

    /// Container that selects the current payload schema.
    fn versioned_state(&self) -> &Versioned;
}

macro_rules! impl_versioned_anchor {
    ($($type:path),+ $(,)?) => {
        $(
            impl VersionedAnchor for $type {
                fn object_id(&self) -> sui::types::Address {
                    self.id.id.bytes
                }

                fn versioned_state(&self) -> &Versioned {
                    &self.state
                }
            }
        )+
    };
}

impl_versioned_anchor!(
    crate::move_bindings::primitives::protocol::Protocol,
    crate::move_bindings::interface::agent::Agent,
    crate::move_bindings::interface::agent::AgentPaymentVault,
    crate::move_bindings::interface::authorization::AgentSkillAuthorization,
    crate::move_bindings::interface::dag::DAG,
    crate::move_bindings::interface::payment::TaskPaymentReserve,
    crate::move_bindings::registry::agent_registry::AgentRegistry,
    crate::move_bindings::registry::leader::LeaderRegistry,
    crate::move_bindings::registry::network_auth::KeyBinding,
    crate::move_bindings::registry::network_auth::NetworkAuth,
    crate::move_bindings::registry::priority_fee_vault::PriorityFeeVault,
    crate::move_bindings::registry::tool_registry::Tool,
    crate::move_bindings::registry::tool_registry::ToolRegistry,
    crate::move_bindings::registry::verifier_registry::VerifierRegistry,
    crate::move_bindings::gas::gas::GasService,
    crate::move_bindings::gas::gas::ToolGas,
    crate::move_bindings::scheduler::task::Task,
);
