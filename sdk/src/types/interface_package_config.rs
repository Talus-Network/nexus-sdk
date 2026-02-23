use {
    super::SharedObjectRef,
    serde::{Deserialize, Serialize},
};

/// On-chain persisted interface configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InterfacePackageConfig {
    pub shared_objects: Vec<SharedObjectRef>,
}
