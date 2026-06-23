//! Identifiers for the `nexus_scheduler` Move package.
//!
//! The `Scheduler` unit struct and its `ModuleAndNameIdent` constants are
//! generated at build time from `generated/ir/scheduler.json`; this module adds
//! the public module identifier and the hand-written `TypeTag` helper on top.

use crate::{idents::ModuleAndNameIdent, sui};

/// Module identifier for `nexus_scheduler::scheduler`.
pub const SCHEDULER_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("scheduler");

include!(concat!(env!("OUT_DIR"), "/idents_scheduler.rs"));

/// Helper to turn a scheduler `ModuleAndNameIdent` into a `sui::types::TypeTag`.
pub fn into_type_tag(
    scheduler_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        scheduler_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}
