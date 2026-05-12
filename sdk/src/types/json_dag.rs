//! This module contains a struct representation of the Nexus DAG JSON file.
//! First line of validation. If try_from fails, there is an error in the
//! configuration and vice versa, if it succeeds, we should be certain that the
//! configuration structure is correct.

use {
    crate::{
        types::{PostFailureAction, StorageKind, VerifierConfig},
        ToolFqn,
    },
    serde::Deserialize,
};

/// Name of the default entry group.
pub const DEFAULT_ENTRY_GROUP: &str = "_default_group";

/// Struct representing the Nexus DAG JSON file.
#[derive(Clone, Debug, Deserialize)]
pub struct Dag {
    /// List of all vertices (tools) in the DAG.
    pub vertices: Vec<Vertex>,
    /// List of edges defining data flow between vertices.
    pub edges: Vec<Edge>,
    /// Optional static input values for vertices.
    pub default_values: Option<Vec<DefaultValue>>,
    /// Default post-failure action for the entire DAG. Can be overridden per vertex.
    /// Determines whether the DAG continues execution or terminates when a vertex fails.
    pub post_failure_action: Option<PostFailureAction>,
    /// Configuration for verifying leader requests. Can be overridden per vertex.
    pub leader_verifier: Option<VerifierConfig>,
    /// Configuration for verifying tool responses. Can be overridden per vertex.
    pub tool_verifier: Option<VerifierConfig>,
    /// Named entry groups defining different starting configurations for the DAG.
    /// If there are no entry groups specified, all specified input ports are
    /// considered to be part of the [`DEFAULT_ENTRY_GROUP`].
    pub entry_groups: Option<Vec<EntryGroup>>,
    /// Which output variants & ports of which vertices should be the output of the DAG.
    /// Only ports specified here will be included in the EndStateReachedEvent.
    pub outputs: Option<Vec<FromPort>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum VertexKind {
    OffChain { tool_fqn: ToolFqn },
    OnChain { tool_fqn: ToolFqn },
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntryPort {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Vertex {
    /// The type and FQN of the tool used by this vertex.
    pub kind: VertexKind,
    /// Unique name for this vertex within the DAG.
    pub name: String,
    /// Entry ports for this vertex that require input data from the user.
    pub entry_ports: Option<Vec<EntryPort>>,
    /// Override the DAG-level post-failure action for this specific vertex.
    pub post_failure_action: Option<PostFailureAction>,
    /// Override the DAG-level leader verifier for this specific vertex.
    pub leader_verifier: Option<VerifierConfig>,
    /// Override the DAG-level tool verifier for this specific vertex.
    pub tool_verifier: Option<VerifierConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntryGroup {
    /// Name of this entry group for reference during DAG execution.
    pub name: String,
    /// List of vertex names that are entry points for this group.
    /// All entry ports of these vertices need to be provided data when executing with this group.
    pub vertices: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DefaultValue {
    /// Name of the vertex this default value applies to.
    pub vertex: String,
    /// Name of the input port on the vertex.
    pub input_port: String,
    /// The static data value to provide.
    pub value: Data,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub struct Data {
    /// Storage location for the data (inline or remote).
    pub storage: StorageKind,
    /// The actual JSON data value.
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Edge {
    /// Output port of the source vertex.
    pub from: FromPort,
    /// Input port of the target vertex.
    pub to: ToPort,
    /// The kind of the edge. This is used to determine how the edge is processed in the workflow.
    /// Defaults to [`EdgeKind::Normal`].
    #[serde(default)]
    pub kind: EdgeKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    #[default]
    Normal,
    /// For-each and collect control edges.
    ForEach,
    Collect,
    /// Do-while and break control edges.
    DoWhile,
    Break,
    /// Provide static values to loops from outside the loop body.
    Static,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FromPort {
    /// Name of the source vertex.
    pub vertex: String,
    /// Output variant of the source vertex (e.g., "ok", "err").
    pub output_variant: String,
    /// Output port name of the source vertex.
    pub output_port: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToPort {
    /// Name of the target vertex.
    pub vertex: String,
    /// Input port name of the target vertex.
    pub input_port: String,
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::VerifierMode};

    #[test]
    fn test_dag_deserialize_post_failure_action() {
        let dag: Dag = serde_json::from_str(
            r#"{
                "post_failure_action": "continue",
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root",
                        "post_failure_action": "terminate"
                    }
                ],
                "edges": []
            }"#,
        )
        .unwrap();

        assert_eq!(
            dag.post_failure_action,
            Some(PostFailureAction::TransientContinue)
        );
        assert_eq!(
            dag.vertices[0].post_failure_action,
            Some(PostFailureAction::Terminate)
        );
    }

    #[test]
    fn test_dag_deserialize_without_post_failure_action() {
        let dag: Dag = serde_json::from_str(
            r#"{
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root"
                    }
                ],
                "edges": []
            }"#,
        )
        .unwrap();

        assert_eq!(dag.post_failure_action, None);
        assert_eq!(dag.vertices[0].post_failure_action, None);
    }

    #[test]
    fn test_dag_deserialize_verifier_config() {
        let dag: Dag = serde_json::from_str(
            r#"{
                "leader_verifier": { "mode": "leader_registered_key", "method": "signed_http_v1" },
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root",
                        "tool_verifier": { "mode": "tool_verifier_contract", "method": "demo_verifier_v1" }
                    }
                ],
                "edges": []
            }"#,
        )
        .unwrap();

        assert_eq!(
            dag.leader_verifier,
            Some(VerifierConfig {
                mode: VerifierMode::LeaderRegisteredKey,
                method: "signed_http_v1".to_string(),
            })
        );
        assert_eq!(
            dag.vertices[0].tool_verifier,
            Some(VerifierConfig {
                mode: VerifierMode::ToolVerifierContract,
                method: "demo_verifier_v1".to_string(),
            })
        );
    }
}
