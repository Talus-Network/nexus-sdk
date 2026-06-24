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

/// Hand-written identifiers for scheduler entry points that aren't yet in the
/// committed IR. Regenerate `generated/ir/scheduler.json` after the matching
/// Move package update lands and drop this block.
impl Scheduler {
    /// `nexus_scheduler::scheduler::add_occurrence_absolute_for_agent_funded_task`
    pub const ADD_OCCURRENCE_ABSOLUTE_FOR_AGENT_FUNDED_TASK: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: SCHEDULER_MODULE,
            name: sui::types::Identifier::from_static(
                "add_occurrence_absolute_for_agent_funded_task",
            ),
        };
    /// `nexus_scheduler::scheduler::add_occurrence_relative_for_agent_funded_task`
    pub const ADD_OCCURRENCE_RELATIVE_FOR_AGENT_FUNDED_TASK: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: SCHEDULER_MODULE,
            name: sui::types::Identifier::from_static(
                "add_occurrence_relative_for_agent_funded_task",
            ),
        };
}

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
