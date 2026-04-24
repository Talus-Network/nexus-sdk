//! # nexus-dag-dsl
//!
//! A type-safe Rust domain-specific language for constructing Nexus DAGs
//! programmatically. The DSL emits the canonical [`nexus_sdk::types::Dag`]
//! wire struct, which can be serialized to the JSON form consumed by every
//! Nexus tool (CLI `dag validate` / `dag publish` / `dag execute`).
//!
//! Two authoring variants coexist in a single crate:
//!
//! - **Relaxed** ([`DagBuilder`]) — string-keyed ports, validation at
//!   [`DagBuilder::build`]. Works with any tool, no descriptors required.
//! - **Strict** (`TypedDagBuilder`, lands in a later step) — typed port
//!   handles, port-payload type mismatches are compile errors.
//!
//! Two authoring surfaces coexist in each variant:
//!
//! - **Flat** — 1:1 mirror of the JSON wire format; every edge-kind has a
//!   dedicated method.
//! - **Scoped** (lands in a later step) — lexical loop bodies via closures;
//!   for-each / collect and do-while / break pairs are enforced by scope
//!   construction.
//!
//! The crate re-exports the wire types from [`nexus_sdk::types`] for
//! convenience.
//!
//! # Example — relaxed flat builder
//!
//! ```
//! use nexus_dag_dsl::{DagBuilder, ToolFqn};
//! use std::str::FromStr;
//!
//! let splitter_fqn = ToolFqn::from_str("xyz.taluslabs.math.i64.add@1").unwrap();
//! let summary_fqn  = ToolFqn::from_str("xyz.taluslabs.math.i64.add@1").unwrap();
//!
//! let mut dag = DagBuilder::new();
//! let splitter = dag.offchain("splitter", splitter_fqn);
//! let summary  = dag.offchain("summary", summary_fqn);
//!
//! dag.entry_port(&splitter, "a")
//!    .inline_default(&splitter, "b", 1)
//!    .edge(splitter.out("ok", "result"), summary.inp("a"))
//!    .inline_default(&summary, "b", 2)
//!    .output(summary.out("ok", "result"));
//!
//! let built = dag.build().expect("DAG validates");
//! assert_eq!(built.vertices.len(), 2);
//! ```

#![warn(missing_docs)]

mod builder;
mod error;
mod scoped;
mod typed;

// Public re-exports from the SDK so callers don't need to depend on
// `nexus-sdk` directly for the wire types they build against.
pub use {
    builder::{DagBuilder, InPortRef, OutPortRef, VertexRef},
    error::DagError,
    nexus_sdk::{
        types::{
            Dag,
            Data,
            DefaultValue,
            Edge,
            EdgeKind,
            EntryGroup,
            EntryPort,
            FromPort,
            StorageKind,
            ToPort,
            Vertex,
            VertexKind,
            DEFAULT_ENTRY_GROUP,
        },
        ToolFqn,
    },
    scoped::{DoWhileScope, ForEachScope, ItemHandle, StateHandle},
    typed::{
        Err,
        InPort,
        Ok,
        OutPort,
        ToolDescriptor,
        TypedDagBuilder,
        TypedVertexRef,
        UntypedVertexRef,
    },
};
