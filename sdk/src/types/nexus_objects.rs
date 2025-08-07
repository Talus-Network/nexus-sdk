//! [`NexusObjects`] struct is holding the Nexus object IDs and refs that are
//! generated during Nexus package deployment.
use {
    crate::sui,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexusObjects {
    pub workflow_pkg_id: sui::ObjectID,
    pub primitives_pkg_id: sui::ObjectID,
    pub interface_pkg_id: sui::ObjectID,
    pub network_id: sui::ObjectID,
    pub tool_registry: sui::ObjectRef,
    pub default_tap: sui::ObjectRef,
    pub gas_service: sui::ObjectRef,
    pub pre_key_vault: sui::ObjectRef,
}
