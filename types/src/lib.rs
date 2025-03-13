//! This library contains all Nexus types that are shared between different
//! parts of Nexus. This includes the CLI, the Toolkits and the Leader node.

/// The ToolFqn type represents a fully qualified tool name. Contains the
/// logic for verifying, serializing and deserializing the FQN.
#[cfg(feature = "tool_fqn")]
mod tool_fqn;
#[cfg(feature = "tool_fqn")]
pub use tool_fqn::*;

/// Ubiquitously used resource identifiers for on-chain types and functions.
/// This includes workflow, primitives and interface Nexus modules but also
/// some Sui framework and Sui move std modules that we use.
#[cfg(feature = "sui_idents")]
pub mod idents;
/// Re-exporting Sui types into something that makes more sense.
#[cfg(feature = "sui_types")]
pub mod sui;
