//! Identifiers for the `nexus_workflow` Move package.
//!
//! The per-module unit structs (`Execution`, `ExecutionEntries`,
//! `ExecutionSubmission`, `Gas`, …) and their `ModuleAndNameIdent` constants are
//! generated at build time from `generated/ir/workflow.json`. This module keeps
//! the hand-written `TypeTag` helper on top of them.
//!
//! Note: verifier identifiers live in the `nexus_interface` package — see
//! [`crate::idents::interface::Verifier`].

use crate::{idents::ModuleAndNameIdent, sui};

include!(concat!(env!("OUT_DIR"), "/idents_workflow.rs"));

/// Helper to turn a `ModuleAndNameIdent` into a `sui::types::TypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(
    workflow_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        workflow_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_type_tag() {
        let rng = &mut rand::thread_rng();
        let workflow_pkg_id = sui::types::Address::generate(rng);
        let ident = ModuleAndNameIdent {
            module: sui::types::Identifier::from_static("foo"),
            name: sui::types::Identifier::from_static("bar"),
        };

        let tag = into_type_tag(workflow_pkg_id, ident);

        assert_eq!(
            tag,
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                workflow_pkg_id,
                sui::types::Identifier::from_static("foo"),
                sui::types::Identifier::from_static("bar"),
                vec![],
            )))
        )
    }
}
