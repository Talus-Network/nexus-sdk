//! This module contains a struct representation of the Nexus DAG JSON file.
//! First line of validation. If try_from fails, there is an error in the
//! configuration and vice versa, if it succeeds, we should be certain that the
//! configuration structure is correct.

use {
    crate::{
        types::{PostFailureAction, StorageKind, VerifierConfig},
        ToolFqn,
    },
    serde::{Deserialize, Deserializer},
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
    #[serde(default, deserialize_with = "deserialize_post_failure_action_option")]
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
    #[serde(default, deserialize_with = "deserialize_post_failure_action_option")]
    pub post_failure_action: Option<PostFailureAction>,
    /// Override the DAG-level leader verifier for this specific vertex.
    pub leader_verifier: Option<VerifierConfig>,
    /// Override the DAG-level tool verifier for this specific vertex.
    pub tool_verifier: Option<VerifierConfig>,
}

fn deserialize_post_failure_action_option<'de, D>(
    deserializer: D,
) -> Result<Option<PostFailureAction>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<serde_json::Value>::deserialize(deserializer)? else {
        return Ok(None);
    };

    if let Some(text) = value.as_str() {
        return match text {
            "continue" => Ok(Some(PostFailureAction::TransientContinue)),
            "terminate" => Ok(Some(PostFailureAction::Terminate)),
            _ => serde_json::from_value(value)
                .map(Some)
                .map_err(serde::de::Error::custom),
        };
    }

    serde_json::from_value(value)
        .map(Some)
        .map_err(serde::de::Error::custom)
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
    #[serde(
        default = "default_edge_kind",
        deserialize_with = "deserialize_edge_kind"
    )]
    pub kind: EdgeKind,
}

pub type EdgeKind = crate::types::generated::interface_types::graph::EdgeKind;

fn default_edge_kind() -> EdgeKind {
    EdgeKind::Normal
}

fn deserialize_edge_kind<'de, D>(deserializer: D) -> Result<EdgeKind, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    parse_edge_kind_value(value).map_err(serde::de::Error::custom)
}

fn parse_edge_kind_value(value: serde_json::Value) -> Result<EdgeKind, String> {
    match value {
        serde_json::Value::String(name) => edge_kind_from_name(&name),
        serde_json::Value::Object(mut object) => {
            if let Some(name) = object
                .remove("@variant")
                .or_else(|| object.remove("_variant_name"))
                .or_else(|| object.remove("variant"))
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
            {
                return edge_kind_from_name(&name);
            }

            if object.len() == 1 {
                let name = object.keys().next().expect("object has one key");
                return edge_kind_from_name(name);
            }

            serde_json::from_value(serde_json::Value::Object(object)).map_err(|err| err.to_string())
        }
        value => serde_json::from_value(value).map_err(|err| err.to_string()),
    }
}

fn edge_kind_from_name(name: &str) -> Result<EdgeKind, String> {
    match name {
        "normal" | "Normal" => Ok(EdgeKind::Normal),
        "for_each" | "ForEach" => Ok(EdgeKind::ForEach),
        "collect" | "Collect" => Ok(EdgeKind::Collect),
        "do_while" | "DoWhile" => Ok(EdgeKind::DoWhile),
        "break" | "Break" => Ok(EdgeKind::Break),
        "static" | "Static" => Ok(EdgeKind::Static),
        _ => Err(format!("unknown edge kind `{name}`")),
    }
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
                "post_failure_action": "TransientContinue",
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root",
                        "post_failure_action": "Terminate"
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
    fn test_dag_deserialize_post_failure_action_aliases_and_null() {
        let dag: Dag = serde_json::from_str(
            r#"{
                "post_failure_action": "continue",
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root",
                        "post_failure_action": "terminate"
                    },
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@2" },
                        "name": "optional",
                        "post_failure_action": null
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
        assert_eq!(dag.vertices[1].post_failure_action, None);
    }

    #[test]
    fn test_dag_deserialize_post_failure_action_rejects_unknown_json_value() {
        let error = serde_json::from_str::<Dag>(
            r#"{
                "post_failure_action": { "invalid": true },
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root"
                    }
                ],
                "edges": []
            }"#,
        )
        .unwrap_err();

        assert!(!error.to_string().is_empty());
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
                "leader_verifier": { "mode": "LeaderRegisteredKey", "method": "signed_http_v1" },
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root",
                        "tool_verifier": { "mode": "ToolVerifierContract", "method": "demo_verifier_v1" }
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
                method: "signed_http_v1".into(),
            })
        );
        assert_eq!(
            dag.vertices[0].tool_verifier,
            Some(VerifierConfig {
                mode: VerifierMode::ToolVerifierContract,
                method: "demo_verifier_v1".into(),
            })
        );
    }

    #[test]
    fn edge_kind_config_uses_generated_graph_edge_kind() {
        use crate::types::generated::interface_types::graph::EdgeKind as GeneratedEdgeKind;

        let edge: Edge = serde_json::from_str(
            r#"{
                "from": {
                    "vertex": "producer",
                    "output_variant": "ok",
                    "output_port": "items"
                },
                "to": {
                    "vertex": "consumer",
                    "input_port": "item"
                },
                "kind": "for_each"
            }"#,
        )
        .unwrap();

        assert_eq!(edge.kind, GeneratedEdgeKind::ForEach);

        let bytes = bcs::to_bytes(&edge.kind).unwrap();
        assert_eq!(
            bcs::from_bytes::<GeneratedEdgeKind>(&bytes).unwrap(),
            GeneratedEdgeKind::ForEach
        );
    }

    #[test]
    fn edge_kind_config_accepts_generated_spellings_and_defaults() {
        let pascal_case_edge: Edge = serde_json::from_str(
            r#"{
                "from": {
                    "vertex": "loop",
                    "output_variant": "ok",
                    "output_port": "next"
                },
                "to": {
                    "vertex": "loop",
                    "input_port": "continue"
                },
                "kind": "DoWhile"
            }"#,
        )
        .unwrap();
        assert_eq!(pascal_case_edge.kind, EdgeKind::DoWhile);

        let wrapper_edge: Edge = serde_json::from_str(
            r#"{
                "from": {
                    "vertex": "loop",
                    "output_variant": "ok",
                    "output_port": "next"
                },
                "to": {
                    "vertex": "loop",
                    "input_port": "item"
                },
                "kind": { "@variant": "Collect" }
            }"#,
        )
        .unwrap();
        assert_eq!(wrapper_edge.kind, EdgeKind::Collect);

        let default_edge: Edge = serde_json::from_str(
            r#"{
                "from": {
                    "vertex": "producer",
                    "output_variant": "ok",
                    "output_port": "result"
                },
                "to": {
                    "vertex": "consumer",
                    "input_port": "input"
                }
            }"#,
        )
        .unwrap();
        assert_eq!(default_edge.kind, EdgeKind::Normal);
    }
}
