//! This library contains all Nexus types that are shared between different
//! parts of Nexus. This includes the CLI, the Toolkits and the Leader node.

pub mod sui;

/// The ToolFqn type represents a fully qualified tool name. Contains the
/// logic for verifying, serializing and deserializing the FQN.
mod tool_fqn;
pub use tool_fqn::*;

/// Ubiqutously used resource identifiers for on-chain types and functions.
/// This includes workflow, primitives and interface Nexus modules but also
/// some Sui framework and Sui move std modules that we use.
pub mod idents;

// TODO: this should be split into features.
