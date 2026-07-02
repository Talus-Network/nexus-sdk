use {crate::ToolFqn, std::time::Duration};

/// Byte-owned tool metadata used to register an off-chain tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolMeta {
    pub fqn: ToolFqn,
    pub url: String,
    pub description: String,
    pub timeout: Duration,
    pub input_schema: Vec<u8>,
    pub output_schema: Vec<u8>,
}
