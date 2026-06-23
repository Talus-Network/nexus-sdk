//! Identifiers for the `nexus_registry` Move package.
//!
//! The per-module unit structs (`AgentRegistry`, `ToolRegistry`, `NetworkAuth`,
//! `Leader`, `VerifierRegistry`, …) and their `ModuleAndNameIdent` constants are
//! generated at build time from `generated/ir/registry.json`. This module keeps
//! the public module identifiers consumed elsewhere and the `TypeTag` helper.

use crate::{idents::ModuleAndNameIdent, sui};

/// Module identifier for `nexus_registry::agent_registry`.
pub const AGENT_REGISTRY_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("agent_registry");
/// Module identifier for `nexus_registry::network_auth`.
pub const NETWORK_AUTH_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("network_auth");
/// Module identifier for `nexus_registry::leader_cap`.
pub const LEADER_CAP_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("leader_cap");
/// Module identifier for `nexus_registry::leader`.
pub const LEADER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("leader");
/// Module identifier for `nexus_registry::verifier_registry`.
pub const VERIFIER_REGISTRY_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_registry");

include!(concat!(env!("OUT_DIR"), "/idents_registry.rs"));

/// Helper to turn a registry `ModuleAndNameIdent` into a `sui::types::TypeTag`.
pub fn into_type_tag(
    registry_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        registry_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}
