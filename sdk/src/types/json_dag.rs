//! This module contains a struct representation of the Nexus DAG JSON file.
//! First line of validation. If try_from fails, there is an error in the
//! configuration and vice versa, if it succeeds, we should be certain that the
//! configuration structure is correct.

use {crate::ToolFqn, serde::Deserialize};

/// Name of the default entry group.
pub const DEFAULT_ENTRY_GROUP: &str = "_default_group";

/// Struct representing the Nexus DAG JSON file.
#[derive(Clone, Debug, Deserialize)]
pub struct Dag {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub default_values: Option<Vec<DefaultValue>>,
    /// If there are no entry groups specified, all specified input ports are
    /// considered to be part of the [`DEFAULT_ENTRY_GROUP`].
    pub entry_groups: Option<Vec<EntryGroup>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum VertexKind {
    OffChain {
        tool_fqn: ToolFqn,
    },
    OnChain {
        //
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Vertex {
    pub kind: VertexKind,
    pub name: String,
    pub input_ports: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntryGroup {
    pub name: String,
    pub members: Vec<EntryMember>,
}

/// Entry members are either pairs of vertex + input port or just vertex names.
/// If only a vertex name is provided, the vertex in question must have no input
/// ports.
#[derive(Clone, Debug, Deserialize)]
pub struct EntryMember {
    pub vertex: String,
    pub input_port: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DefaultValue {
    pub vertex: String,
    pub input_port: String,
    pub value: Data,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub enum Data {
    Inline { data: serde_json::Value },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Edge {
    pub from: FromPort,
    pub to: ToPort,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FromPort {
    pub vertex: String,
    pub output_variant: String,
    pub output_port: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToPort {
    pub vertex: String,
    pub input_port: String,
}
