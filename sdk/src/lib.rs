//! This library contains all Nexus types that are shared between different
//! parts of Nexus. This includes the CLI, the Toolkits and the Leader node.

pub mod kat;

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

/// Nexus types represent the structure of various objects that are defined
/// on-chain. It also provides the logic for serializing and deserializing these
/// objects.
#[cfg(feature = "types")]
pub mod types;

/// Nexus events that are fired by the Nexus workflow package and are used to
/// communicate between the on-chain and off-chain parts of Nexus. This module
/// also contains the logic for serializing and deserializing these events.
#[cfg(feature = "events")]
pub mod events;

/// Object crawler attempts to improve the Sui SDK object fetching by allowing
/// direct parsing into structs.
#[cfg(feature = "object_crawler")]
pub mod object_crawler;

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

/// Cryptographic primitives including X3DH for secure key exchange
#[cfg(feature = "crypto")]
pub mod crypto;

/// Secret core provides a generic secret type that can be used to store
#[cfg(feature = "secret_core")]
pub mod secret_core;

/// Provides DAG-related utilities
#[cfg(feature = "dag")]
pub mod dag;

/// Groth16 provides a generic implementation of the Groth16 proof system.
#[cfg(feature = "groth16")]
pub mod groth16;
