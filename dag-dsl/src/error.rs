//! Errors surfaced by the DSL's [`crate::DagBuilder::build`] step.
//!
//! [`DagBuilder`] aggregates every detected problem into `Vec<DagError>` so a
//! user sees all issues in one pass rather than fixing them one at a time.
//! Delegated validation from `nexus_sdk::dag::validator::validate` is bailed
//! on first error (a limitation of the existing sdk validator) and surfaced
//! as [`DagError::WireValidation`].
//!
//! [`DagBuilder`]: crate::DagBuilder

use thiserror::Error;

/// An error detected while building a DAG.
///
/// Every variant carries enough locator information (vertex name / port name
/// / edge endpoints) to fix the offending construct without re-reading the
/// whole builder input.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DagError {
    /// A vertex name was used more than once. Wire-level DAG references
    /// vertices by string name; duplicates would silently collide.
    #[error("duplicate vertex name `{name}`")]
    DuplicateVertex {
        /// The clashing vertex name.
        name: String,
    },

    /// An edge's `from` endpoint references a vertex that was never added to
    /// the builder.
    #[error(
        "edge source references unknown vertex `{vertex}` (variant `{variant}`, port `{port}`)"
    )]
    EdgeFromUnknownVertex {
        /// The unknown vertex name.
        vertex: String,
        /// The source output variant on the unknown vertex.
        variant: String,
        /// The source output port on the unknown vertex.
        port: String,
    },

    /// An edge's `to` endpoint references a vertex that was never added to
    /// the builder.
    #[error("edge destination references unknown vertex `{vertex}` (port `{port}`)")]
    EdgeToUnknownVertex {
        /// The unknown vertex name.
        vertex: String,
        /// The destination input port on the unknown vertex.
        port: String,
    },

    /// An entry group lists a vertex that was never added to the builder.
    #[error("entry group `{group}` references unknown vertex `{vertex}`")]
    EntryGroupUnknownVertex {
        /// The entry group name.
        group: String,
        /// The unknown vertex referenced by the group.
        vertex: String,
    },

    /// A default value references a vertex that was never added to the
    /// builder.
    #[error("default value references unknown vertex `{vertex}` (input port `{port}`)")]
    DefaultValueUnknownVertex {
        /// The unknown vertex name.
        vertex: String,
        /// The input port the default was targeting.
        port: String,
    },

    /// An output references a vertex that was never added to the builder.
    #[error("output references unknown vertex `{vertex}` (variant `{variant}`, port `{port}`)")]
    OutputUnknownVertex {
        /// The unknown vertex name.
        vertex: String,
        /// The source output variant on the unknown vertex.
        variant: String,
        /// The source output port on the unknown vertex.
        port: String,
    },

    /// The nexus-sdk wire-level validator rejected the assembled DAG. The
    /// message is the sdk validator's own error text — it covers
    /// acyclicity, for-each/collect pairing, do-while/break pairing,
    /// concurrency rules, and structural shape rules.
    #[error("wire-level validation failed: {message}")]
    WireValidation {
        /// Human-readable message produced by
        /// `nexus_sdk::dag::validator::validate`.
        message: String,
    },
}
