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
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub default_values: Option<Vec<DefaultValue>>,
    pub post_failure_action: Option<PostFailureAction>,
    pub leader_verifier: Option<VerifierConfig>,
    pub tool_verifier: Option<VerifierConfig>,
    /// If there are no entry groups specified, all specified input ports are
    /// considered to be part of the [`DEFAULT_ENTRY_GROUP`].
    pub entry_groups: Option<Vec<EntryGroup>>,
    /// Which output variants & ports of which vertices should be the output of
    /// the DAG.
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
    pub kind: VertexKind,
    pub name: String,
    pub entry_ports: Option<Vec<EntryPort>>,
    pub post_failure_action: Option<PostFailureAction>,
    pub leader_verifier: Option<VerifierConfig>,
    pub tool_verifier: Option<VerifierConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EntryGroup {
    pub name: String,
    /// List of vertex names that are part of this entry group. All entry ports
    /// of these vertices need to be provided data for when executing the DAG.
    pub vertices: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DefaultValue {
    pub vertex: String,
    pub input_port: String,
    pub value: Data,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub struct Data {
    pub storage: StorageKind,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Edge {
    pub from: FromPort,
    pub to: ToPort,
    /// The kind of the edge. This is used to determine how the edge is
    /// processed in the workflow. Defaults to [`EdgeKind::Normal`].
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
    pub vertex: String,
    pub output_variant: String,
    pub output_port: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToPort {
    pub vertex: String,
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
