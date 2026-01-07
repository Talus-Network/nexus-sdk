use {
    crate::ToolFqn,
    serde::{Deserialize, Serialize},
};

/// Useful struct holding Tool metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolMeta {
    pub fqn: ToolFqn,
    pub url: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
}
