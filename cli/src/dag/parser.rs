//! This module contains a struct representation of the Nexus DAG JSON file.
//! First line of validation. If try_from fails, there is an error in the
//! configuration and vice versa, if it succeeds, we should be certain that the
//! configuration structure is correct.
//!
//! # Example
//!
//! ```no_run
//! let dag: Dag = include_str!("./_dags/trip_planner.json").try_into()?;
//!
//! assert!(dag.is_ok());
//! ```

use crate::prelude::*;

/// Name of the default entry group.
pub(crate) const DEFAULT_ENTRY_GROUP: &str = "_default_group";

/// Struct representing the Nexus DAG JSON file.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Dag {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) entry_vertices: Vec<EntryVertex>,
    pub(crate) default_values: Option<Vec<DefaultValue>>,
    /// If there are no entry groups specified, all entry vertices are considered
    /// to be in the [DEFAULT_ENTRY_GROUP] that is automatically created on-chain
    /// when a new DAG object is created.
    pub(crate) entry_groups: Option<Vec<EntryGroup>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub(crate) enum VertexKind {
    OffChain {
        tool_fqn: ToolFqn,
    },
    OnChain {
        //
    },
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Vertex {
    pub(crate) kind: VertexKind,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct EntryGroup {
    pub(crate) name: String,
    pub(crate) vertices: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct EntryVertex {
    pub(crate) kind: VertexKind,
    pub(crate) name: String,
    pub(crate) input_ports: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct DefaultValue {
    pub(crate) vertex: String,
    pub(crate) input_port: String,
    pub(crate) value: Data,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub(crate) enum Data {
    Inline { data: serde_json::Value },
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Edge {
    pub(crate) from: FromPort,
    pub(crate) to: ToPort,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FromPort {
    pub(crate) vertex: String,
    pub(crate) output_variant: String,
    pub(crate) output_port: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ToPort {
    pub(crate) vertex: String,
    pub(crate) input_port: String,
}

// == Dag Impls ==

impl TryFrom<&str> for Dag {
    type Error = AnyError;

    fn try_from(s: &str) -> AnyResult<Self> {
        serde_json::from_str(s).map_err(AnyError::from)
    }
}
