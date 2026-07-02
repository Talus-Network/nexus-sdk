use crate::{
    move_bindings::{
        interface::{
            graph::{EdgeKind, PostFailureAction},
            verifier::VerifierConfig,
        },
        primitives::data::NexusData,
    },
    ToolFqn,
};

/// Name of the default entry group.
pub const DEFAULT_ENTRY_GROUP: &str = "_default_group";

/// Normalized typed specification for constructing a Nexus DAG on-chain.
#[derive(Clone, Debug, Default)]
pub struct DagSpec {
    pub vertices: Vec<DagVertex>,
    pub edges: Vec<DagEdge>,
    pub default_values: Vec<DagDefaultValue>,
    pub post_failure_action: Option<PostFailureAction>,
    pub leader_verifier: Option<VerifierConfig>,
    pub tool_verifier: Option<VerifierConfig>,
    pub entry_groups: Vec<DagEntryGroup>,
    pub outputs: Vec<DagOutput>,
}

#[derive(Clone, Debug)]
pub enum DagVertexKind {
    OffChain { tool_fqn: ToolFqn },
    OnChain { tool_fqn: ToolFqn },
}

#[derive(Clone, Debug)]
pub struct DagEntryPort {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct DagVertex {
    pub kind: DagVertexKind,
    pub name: String,
    pub entry_ports: Vec<DagEntryPort>,
    pub post_failure_action: Option<PostFailureAction>,
    pub leader_verifier: Option<VerifierConfig>,
    pub tool_verifier: Option<VerifierConfig>,
}

#[derive(Clone, Debug)]
pub struct DagEntryGroup {
    pub name: String,
    pub vertices: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct DagDefaultValue {
    pub vertex: String,
    pub input_port: String,
    pub value: NexusData,
}

#[derive(Clone, Debug)]
pub struct DagEdge {
    pub from: DagOutput,
    pub to: DagInput,
    pub kind: EdgeKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DagOutput {
    pub vertex: String,
    pub output_variant: String,
    pub output_port: String,
}

#[derive(Clone, Debug)]
pub struct DagInput {
    pub vertex: String,
    pub input_port: String,
}
