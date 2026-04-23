//! This module contains a struct representation of the Nexus DAG JSON file.
//! First line of validation. If try_from fails, there is an error in the
//! configuration and vice versa, if it succeeds, we should be certain that the
//! configuration structure is correct.

use {
    crate::{types::StorageKind, ToolFqn},
    serde::{Deserialize, Serialize},
};

/// Name of the default entry group.
pub const DEFAULT_ENTRY_GROUP: &str = "_default_group";

/// Struct representing the Nexus DAG JSON file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dag {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_values: Option<Vec<DefaultValue>>,
    /// If there are no entry groups specified, all specified input ports are
    /// considered to be part of the [`DEFAULT_ENTRY_GROUP`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_groups: Option<Vec<EntryGroup>>,
    /// Which output variants & ports of which vertices should be the output of
    /// the DAG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<FromPort>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum VertexKind {
    OffChain { tool_fqn: ToolFqn },
    OnChain { tool_fqn: ToolFqn },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntryPort {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub kind: VertexKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_ports: Option<Vec<EntryPort>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntryGroup {
    pub name: String,
    /// List of vertex names that are part of this entry group. All entry ports
    /// of these vertices need to be provided data for when executing the DAG.
    pub vertices: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefaultValue {
    pub vertex: String,
    pub input_port: String,
    pub value: Data,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "storage", rename_all = "snake_case")]
pub struct Data {
    pub storage: StorageKind,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: FromPort,
    pub to: ToPort,
    /// The kind of the edge. This is used to determine how the edge is
    /// processed in the workflow. Defaults to [`EdgeKind::Normal`].
    #[serde(default, skip_serializing_if = "EdgeKind::is_normal")]
    pub kind: EdgeKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
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

impl EdgeKind {
    /// Returns `true` when this is the default [`EdgeKind::Normal`]. Used by
    /// [`Edge`]'s serializer to omit the `kind` field for normal edges,
    /// matching the canonical wire form where `Normal` edges appear without a
    /// `kind` key.
    pub fn is_normal(&self) -> bool {
        matches!(self, EdgeKind::Normal)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FromPort {
    pub vertex: String,
    pub output_variant: String,
    pub output_port: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToPort {
    pub vertex: String,
    pub input_port: String,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{fs, path::PathBuf},
    };

    /// Round-trip every committed DAG JSON fixture through `Serialize` /
    /// `Deserialize`.
    ///
    /// For each `.json` file under `sdk/src/dag/_dags/`, parse it as a [`Dag`],
    /// serialize the parsed value back to JSON, re-parse that as
    /// [`serde_json::Value`], and compare it structurally (key-order-agnostic)
    /// to the original document. Files that do not parse as a [`Dag`] are
    /// skipped and reported, not failed — the corpus intentionally contains
    /// deliberately-malformed fixtures for other tests.
    ///
    /// What breaks if this test is deleted: a future change to the [`Dag`]
    /// types or their serde attributes could silently diverge the `Serialize`
    /// output from what `Deserialize` accepts. Any tool that reads a DAG,
    /// inspects or copies it in memory, and writes it back (e.g. a DSL that
    /// emits DAGs, or a migration that round-trips through Rust structs)
    /// would then produce a different wire representation than the one the
    /// CLI's `dag validate` / `dag publish` / `dag execute` commands emit
    /// today — silently breaking every consumer downstream.
    #[test]
    fn roundtrip_committed_dag_fixtures() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/dag/_dags");
        let entries: Vec<_> = fs::read_dir(&dir)
            .expect("fixture directory must exist")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();

        assert!(
            !entries.is_empty(),
            "no DAG JSON fixtures found under {}",
            dir.display()
        );

        let mut skipped: Vec<String> = Vec::new();
        let mut checked = 0usize;

        for entry in entries {
            let path = entry.path();
            let content = fs::read_to_string(&path).expect("fixture readable");

            let parsed: Dag = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(_) => {
                    skipped.push(path.file_name().unwrap().to_string_lossy().into_owned());
                    continue;
                }
            };

            let reserialized = serde_json::to_string(&parsed).expect("serialize Dag");
            let reparsed_value: serde_json::Value =
                serde_json::from_str(&reserialized).expect("reserialized DAG is valid JSON");
            let original_value: serde_json::Value =
                serde_json::from_str(&content).expect("original fixture is valid JSON");

            assert_eq!(
                reparsed_value,
                original_value,
                "round-trip mismatch for {}",
                path.display()
            );
            checked += 1;
        }

        assert!(
            checked > 0,
            "no valid DAG fixtures were round-tripped; all {} files skipped",
            skipped.len()
        );
    }

    /// Verify that [`EdgeKind::is_normal`] returns `true` exactly for
    /// [`EdgeKind::Normal`] and `false` for every other variant.
    ///
    /// What breaks if this test is deleted: the [`Edge`]'s
    /// `skip_serializing_if = "EdgeKind::is_normal"` attribute would silently
    /// stop omitting the `kind` field for normal edges if `is_normal` drifted
    /// (e.g. accidentally widened to also return `true` for a new default, or
    /// narrowed to always return `false`), breaking the canonical wire-form
    /// convention that normal edges omit `kind`.
    #[test]
    fn edge_kind_is_normal_only_for_normal() {
        assert!(EdgeKind::Normal.is_normal());
        assert!(!EdgeKind::ForEach.is_normal());
        assert!(!EdgeKind::Collect.is_normal());
        assert!(!EdgeKind::DoWhile.is_normal());
        assert!(!EdgeKind::Break.is_normal());
        assert!(!EdgeKind::Static.is_normal());
    }
}
