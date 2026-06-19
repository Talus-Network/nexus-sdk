//! Move identifiers generated from committed on-chain package metadata.
//!
//! The constants below are rendered at build time by `build.rs` from the
//! normalized package IR (intermediate representation) committed under
//! `src/idents/generated/ir/*.json`. One `pub mod <package>` is emitted per IR
//! file, and within it one unit struct per Move module exposing
//! [`ModuleAndNameIdent`](crate::idents::ModuleAndNameIdent) constants for every
//! function and datatype.
//!
//! These constants are deliberately **address-free**: the package id is supplied
//! at call time from the runtime-injected `NexusObjects`, so the same generated
//! identifier works against any deployment (localnet, testnet, mainnet). This is
//! what lets generated code replace the hand-maintained `idents` submodules
//! without baking a deployment-specific package address into the binary.
//!
//! To refresh the IR (after the on-chain Move changes), publish the Nexus
//! packages to a localnet and run the `generate_binding` integration test —
//! `just regenerate-idents` wraps the whole pipeline.

include!(concat!(env!("OUT_DIR"), "/generated_idents.rs"));

#[cfg(test)]
mod tests {
    use crate::sui;

    /// A snake_case Move function maps to a SCREAMING_SNAKE constant carrying
    /// the bare module + name (no package address).
    #[test]
    fn function_ident_is_address_free() {
        let ident = super::workflow::Dag::ABORT_EXPIRED_EXECUTION;
        assert_eq!(ident.module, sui::types::Identifier::from_static("dag"));
        assert_eq!(
            ident.name,
            sui::types::Identifier::from_static("abort_expired_execution")
        );
    }

    /// A PascalCase Move datatype is screaming-cased correctly
    /// (`DAGExecution` -> `DAG_EXECUTION`) while keeping the on-chain name.
    #[test]
    fn datatype_ident_preserves_onchain_name() {
        let ident = super::workflow::Dag::DAG_EXECUTION;
        assert_eq!(ident.module, sui::types::Identifier::from_static("dag"));
        assert_eq!(
            ident.name,
            sui::types::Identifier::from_static("DAGExecution")
        );
    }

    /// The package address is supplied at call time, not baked into the const.
    #[test]
    fn qualified_name_uses_runtime_package_id() {
        let pkg = sui::types::Address::generate(rand::thread_rng());
        let qualified = super::workflow::Dag::DAG_EXECUTION.qualified_name(pkg);
        assert_eq!(qualified, format!("{pkg}::dag::DAGExecution"));
    }
}
