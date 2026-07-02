//! This library contains all Nexus types that are shared between different
//! parts of Nexus. This includes the CLI, the Toolkits and the Leader node.

/// The ToolFqn type represents a fully qualified tool name. Contains the
/// logic for verifying, serializing and deserializing the FQN.
#[cfg(feature = "tool_fqn")]
mod tool_fqn;
#[cfg(feature = "tool_fqn")]
pub use tool_fqn::*;

/// Re-exporting Sui types into something that makes more sense.
#[cfg(feature = "sui_types")]
pub mod sui;

/// Generated Move package bindings used as the SDK's Move ABI boundary.
#[cfg(feature = "types")]
pub mod move_bindings;

/// Nexus types represent the structure of various objects that are defined
/// on-chain. It also provides the logic for serializing and deserializing these
/// objects.
#[cfg(feature = "types")]
pub mod types;

#[cfg(feature = "types")]
mod move_boundary;

/// DAG JSON parsing and static validation helpers for SDK and CLI authoring flows.
#[cfg(feature = "dag")]
pub mod dag;

/// Move module introspection helpers for on-chain tool schema generation.
#[cfg(feature = "onchain_schema_gen")]
pub mod onchain_schema_gen;

/// Nexus events that are fired by the Nexus workflow package and are used to
/// communicate between the on-chain and off-chain parts of Nexus. This module
/// also contains the logic for serializing and deserializing these events.
#[cfg(feature = "events")]
pub mod events;

/// Transactions module contains builders for PTBs that are submitted to Sui
/// and perform various operations on the Nexus ecosystem.
#[cfg(feature = "transactions")]
pub mod transactions;

/// Test utils container management for Sui and Redis, faucet, Move code
/// compilation and deployment and similar.
#[cfg(feature = "test_utils")]
pub mod test_utils;

/// Walrus client provides integration with the Walrus decentralized blob storage
/// system for storing and retrieving files.
#[cfg(feature = "walrus")]
pub mod walrus;

/// Provides various Nexus utilities like deployment and execution of workflows.
#[cfg(feature = "nexus")]
pub mod nexus;

/// Application-layer request/response signatures for HTTP.
#[cfg(feature = "signed_http")]
pub mod signed_http;
