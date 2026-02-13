//! Define models for Sui Move objects related to Nexus DAGs.

use {
    crate::{
        nexus::crawler::{DynamicMap, Map},
        sui,
        types::{NexusData, TypeName},
        ToolFqn,
    },
    serde::{Deserialize, Serialize},
};

/// Struct holding the DAG definition from our Move code.
///
/// See <sui/workflow/sources/dag.move:DAG> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Dag {
    pub vertices: DynamicMap<TypeName, DagVertexInfo>,
    pub defaults_to_input_ports: DynamicMap<DagVertexInputPort, NexusData>,
    /// We fetch the edges so that we know which output ports need encryption.
    pub edges: DynamicMap<TypeName, Vec<DagEdge>>,
    /// We fetch the outputs so that we know which output ports need encryption.
    pub outputs: DynamicMap<TypeName, Vec<DagOutputVariantPort>>,
}

/// Struct holding the DAG execution information.
///
/// See <sui/workflow/sources/dag.move:DAGExecution> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagExecution {
    /// The address of the sender of the transaction to trigger this DAG
    /// execution.
    pub invoker: sui::types::Address,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagVertexInfo {
    pub kind: DagVertexKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "_variant_name")]
pub enum DagVertexKind {
    OffChain { tool_fqn: ToolFqn },
    OnChain { tool_fqn: ToolFqn },
}

impl DagVertexKind {
    pub fn tool_fqn(&self) -> &ToolFqn {
        match self {
            DagVertexKind::OffChain { tool_fqn } => tool_fqn,
            DagVertexKind::OnChain { tool_fqn } => tool_fqn,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct DagVertexInputPort {
    pub vertex: TypeName,
    pub port: DagInputPort,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct DagInputPort {
    pub name: String,
    pub encrypted: bool,
}

/// Enum distinguishing between a plain vertex and a vertex with an iterator.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "_variant_name")]
pub enum DagPortData {
    Single { data: NexusData },
    Many { data: Map<String, NexusData> },
}

/// Struct holding the evaluations for a vertex in the DAG.
///
/// See <sui/workflow/sources/dag.move:VertexEvaluations> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagVertexEvaluations {
    pub ports_to_data: Map<TypeName, DagPortData>,
}

/// Struct holding the edges in the DAG.
///
/// See <sui/workflow/sources/dag.move:Edge> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagEdge {
    pub from: DagOutputVariantPort,
}

/// Struct holding the output variant and port pair, holding the encryption
/// info.
///
/// See <sui/workflow/sources/dag.move:OutputVariantPort> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagOutputVariantPort {
    pub variant: TypeName,
    pub port: TypeName,
    pub encrypted: bool,
}
