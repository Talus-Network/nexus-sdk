//! Minimal Leader-to-Tool signing transport.
//!
//! The protocol deliberately signs only the values consumed by the onchain RegisteredKey
//! verifier:
//! - the Leader signs `SHA-256(BCS(canonical_tool_inputs))`;
//! - the Tool signs `leader_signature || SHA-256(result_bytes)`;
//! - `result_bytes` is the exact BCS `TaggedOutput` submitted onchain.
//!
//! Leader identity, active-key selection, and nonce-based response caching remain offchain
//! transport concerns. They are carried as headers but are not added to either signature.
//! Authenticated HTTPS must protect the request body, nonce, and unsigned headers on every network
//! hop; these minimal onchain signatures do not make plaintext HTTP safe.

pub mod keys;
pub mod v2;
