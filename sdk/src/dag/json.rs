//! JSON document parser for Nexus DAG specification files.

use {
    crate::{
        move_bindings::{
            interface::{
                graph::{EdgeKind, PostFailureAction},
                verifier::{VerifierConfig, VerifierMode},
            },
            move_std::ascii::String as MoveString,
            primitives::data::NexusData,
        },
        types::{
            DagDefaultValue, DagEdge, DagEntryGroup, DagEntryPort, DagInput, DagOutput, DagSpec,
            DagVertex, DagVertexKind,
        },
        ToolFqn,
    },
    serde::{Deserialize, Deserializer},
};

pub fn parse_dag_spec(input: &str) -> Result<DagSpec, serde_json::Error> {
    serde_json::from_str::<DagDocument>(input).map(Into::into)
}

#[derive(Clone, Debug, Deserialize)]
struct DagDocument {
    vertices: Vec<VertexDocument>,
    edges: Vec<EdgeDocument>,
    default_values: Option<Vec<DefaultValueDocument>>,
    #[serde(default, deserialize_with = "deserialize_post_failure_action_option")]
    post_failure_action: Option<PostFailureAction>,
    #[serde(default, deserialize_with = "deserialize_verifier_config_option")]
    leader_verifier: Option<VerifierConfig>,
    #[serde(default, deserialize_with = "deserialize_verifier_config_option")]
    tool_verifier: Option<VerifierConfig>,
    entry_groups: Option<Vec<EntryGroupDocument>>,
    outputs: Option<Vec<OutputPortDocument>>,
}

impl From<DagDocument> for DagSpec {
    fn from(document: DagDocument) -> Self {
        Self {
            vertices: document.vertices.into_iter().map(Into::into).collect(),
            edges: document.edges.into_iter().map(Into::into).collect(),
            default_values: document
                .default_values
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            post_failure_action: document.post_failure_action,
            leader_verifier: document.leader_verifier,
            tool_verifier: document.tool_verifier,
            entry_groups: document
                .entry_groups
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            outputs: document
                .outputs
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
enum VertexKindDocument {
    OffChain { tool_fqn: ToolFqn },
    OnChain { tool_fqn: ToolFqn },
}

impl From<VertexKindDocument> for DagVertexKind {
    fn from(kind: VertexKindDocument) -> Self {
        match kind {
            VertexKindDocument::OffChain { tool_fqn } => Self::OffChain { tool_fqn },
            VertexKindDocument::OnChain { tool_fqn } => Self::OnChain { tool_fqn },
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct EntryPortDocument {
    name: String,
}

impl From<EntryPortDocument> for DagEntryPort {
    fn from(document: EntryPortDocument) -> Self {
        Self {
            name: document.name,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct VertexDocument {
    kind: VertexKindDocument,
    name: String,
    entry_ports: Option<Vec<EntryPortDocument>>,
    #[serde(default, deserialize_with = "deserialize_post_failure_action_option")]
    post_failure_action: Option<PostFailureAction>,
    #[serde(default, deserialize_with = "deserialize_verifier_config_option")]
    leader_verifier: Option<VerifierConfig>,
    #[serde(default, deserialize_with = "deserialize_verifier_config_option")]
    tool_verifier: Option<VerifierConfig>,
}

impl From<VertexDocument> for DagVertex {
    fn from(document: VertexDocument) -> Self {
        Self {
            kind: document.kind.into(),
            name: document.name,
            entry_ports: document
                .entry_ports
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            post_failure_action: document.post_failure_action,
            leader_verifier: document.leader_verifier,
            tool_verifier: document.tool_verifier,
        }
    }
}

fn deserialize_post_failure_action_option<'de, D>(
    deserializer: D,
) -> Result<Option<PostFailureAction>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<PostFailureActionInput>::deserialize(deserializer)?
        .map(PostFailureActionInput::into_inner)
        .transpose()
        .map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PostFailureActionInput {
    Name(String),
}

impl PostFailureActionInput {
    fn into_inner(self) -> Result<PostFailureAction, String> {
        match self {
            Self::Name(name) => post_failure_action_from_name(&name),
        }
    }
}

fn post_failure_action_from_name(name: &str) -> Result<PostFailureAction, String> {
    match name {
        "TransientContinue" | "continue" => Ok(PostFailureAction::TransientContinue),
        "Terminate" | "terminate" => Ok(PostFailureAction::Terminate),
        _ => Err(format!("unknown post failure action `{name}`")),
    }
}

fn deserialize_verifier_config_option<'de, D>(
    deserializer: D,
) -> Result<Option<VerifierConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<VerifierConfigInput>::deserialize(deserializer)
        .map(|value| value.map(VerifierConfigInput::into_inner))
}

#[derive(Deserialize)]
struct VerifierConfigInput {
    mode: VerifierModeInput,
    method: String,
}

impl VerifierConfigInput {
    fn into_inner(self) -> VerifierConfig {
        VerifierConfig {
            mode: self.mode.0,
            method: MoveString::from(self.method),
        }
    }
}

struct VerifierModeInput(VerifierMode);

impl<'de> Deserialize<'de> for VerifierModeInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        verifier_mode_from_name(&name)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

fn verifier_mode_from_name(name: &str) -> Result<VerifierMode, String> {
    match name {
        "none" | "None" => Ok(VerifierMode::None),
        "leader_registered_key" | "LeaderRegisteredKey" | "leaderRegisteredKey" => {
            Ok(VerifierMode::LeaderRegisteredKey)
        }
        "tool_verifier_contract" | "ToolVerifierContract" | "toolVerifierContract" => {
            Ok(VerifierMode::ToolVerifierContract)
        }
        _ => Err(format!("unknown verifier mode `{name}`")),
    }
}

#[derive(Clone, Debug, Deserialize)]
struct EntryGroupDocument {
    name: String,
    vertices: Vec<String>,
}

impl From<EntryGroupDocument> for DagEntryGroup {
    fn from(document: EntryGroupDocument) -> Self {
        Self {
            name: document.name,
            vertices: document.vertices,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct DefaultValueDocument {
    vertex: String,
    input_port: String,
    #[serde(deserialize_with = "deserialize_nexus_data")]
    value: NexusData,
}

impl From<DefaultValueDocument> for DagDefaultValue {
    fn from(document: DefaultValueDocument) -> Self {
        Self {
            vertex: document.vertex,
            input_port: document.input_port,
            value: document.value,
        }
    }
}

fn deserialize_nexus_data<'de, D>(deserializer: D) -> Result<NexusData, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    nexus_data_from_json(value).map_err(serde::de::Error::custom)
}

fn nexus_data_from_json(value: serde_json::Value) -> Result<NexusData, String> {
    let serde_json::Value::Object(mut object) = value else {
        return serde_json::from_value(value).map_err(|error| error.to_string());
    };

    let Some(data) = object.remove("data") else {
        return serde_json::from_value(serde_json::Value::Object(object))
            .map_err(|error| error.to_string());
    };

    let storage = object
        .remove("storage")
        .and_then(|value| value.as_str().map(storage_tag_bytes))
        .ok_or_else(|| "missing nexus data storage".to_string())??;

    match data {
        serde_json::Value::Array(values) => Ok(NexusData {
            storage,
            one: Vec::new(),
            many: values
                .into_iter()
                .map(|value| serde_json::to_vec(&value).map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        value => Ok(NexusData {
            storage,
            one: serde_json::to_vec(&value).map_err(|error| error.to_string())?,
            many: Vec::new(),
        }),
    }
}

fn storage_tag_bytes(name: &str) -> Result<Vec<u8>, String> {
    match name {
        "inline" => Ok(b"inline".to_vec()),
        "walrus" => Ok(b"walrus".to_vec()),
        _ => Err(format!("unknown nexus data storage `{name}`")),
    }
}

#[derive(Clone, Debug, Deserialize)]
struct EdgeDocument {
    from: OutputPortDocument,
    to: InputPortDocument,
    #[serde(
        default = "default_edge_kind",
        deserialize_with = "deserialize_edge_kind"
    )]
    kind: EdgeKind,
}

impl From<EdgeDocument> for DagEdge {
    fn from(document: EdgeDocument) -> Self {
        Self {
            from: document.from.into(),
            to: document.to.into(),
            kind: document.kind,
        }
    }
}

fn default_edge_kind() -> EdgeKind {
    EdgeKind::Normal
}

fn deserialize_edge_kind<'de, D>(deserializer: D) -> Result<EdgeKind, D::Error>
where
    D: Deserializer<'de>,
{
    EdgeKindInput::deserialize(deserializer)
        .and_then(|value| value.into_inner().map_err(serde::de::Error::custom))
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EdgeKindInput {
    Name(String),
    Variant {
        #[serde(alias = "@variant", alias = "_variant_name")]
        variant: String,
    },
}

impl EdgeKindInput {
    fn into_inner(self) -> Result<EdgeKind, String> {
        match self {
            Self::Name(name) | Self::Variant { variant: name } => edge_kind_from_name(&name),
        }
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
struct OutputPortDocument {
    vertex: String,
    output_variant: String,
    output_port: String,
}

impl From<OutputPortDocument> for DagOutput {
    fn from(document: OutputPortDocument) -> Self {
        Self {
            vertex: document.vertex,
            output_variant: document.output_variant,
            output_port: document.output_port,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct InputPortDocument {
    vertex: String,
    input_port: String,
}

impl From<InputPortDocument> for DagInput {
    fn from(document: InputPortDocument) -> Self {
        Self {
            vertex: document.vertex,
            input_port: document.input_port,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::move_bindings::interface::verifier::VerifierMode};

    #[test]
    fn test_dag_deserialize_post_failure_action() {
        let dag: DagDocument = serde_json::from_str(
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
        let dag: DagDocument = serde_json::from_str(
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
        let error = serde_json::from_str::<DagDocument>(
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
        let dag: DagDocument = serde_json::from_str(
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
        let dag: DagDocument = serde_json::from_str(
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
    fn default_value_accepts_readable_inline_storage_shape() {
        let dag = parse_dag_spec(
            r#"{
                "vertices": [
                    {
                        "kind": { "variant": "off_chain", "tool_fqn": "xyz.tool.test@1" },
                        "name": "root"
                    }
                ],
                "default_values": [
                    {
                        "vertex": "root",
                        "input_port": "amount",
                        "value": {
                            "storage": "inline",
                            "data": -3
                        }
                    }
                ],
                "edges": []
            }"#,
        )
        .unwrap();

        let value = &dag.default_values[0].value;
        assert_eq!(value.storage, b"inline".to_vec());
        assert_eq!(value.one, b"-3".to_vec());
        assert!(value.many.is_empty());
    }

    #[test]
    fn edge_kind_config_uses_graph_edge_kind() {
        use crate::move_bindings::interface::graph::EdgeKind as DirectEdgeKind;

        let edge: EdgeDocument = serde_json::from_str(
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

        assert_eq!(edge.kind, DirectEdgeKind::ForEach);
    }

    #[test]
    fn edge_kind_config_accepts_move_spellings_and_defaults() {
        let pascal_case_edge: EdgeDocument = serde_json::from_str(
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

        let wrapper_edge: EdgeDocument = serde_json::from_str(
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

        let default_edge: EdgeDocument = serde_json::from_str(
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
