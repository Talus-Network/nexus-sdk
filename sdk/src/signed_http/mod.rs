//! Leader-to-Tool signing transport.
//!
//! The SDK keeps signed HTTP v3 and v4 as separate wire protocols. V3 authenticates the
//! canonical input hash without a Tool ID; v4 authenticates the registered Tool ID, nonce,
//! and canonical input hash in a domain-separated request transcript. Both versions bind Tool
//! result signatures to the Leader signature, nonce, and exact ordered BCS result bytes.
//!
//! The Toolkit requires a configured Tool ID for each FQN in required mode for both versions.
//! This local setting does not make the v3 request authenticate a Tool ID.
//!
//! The current Leader emits v4, and current on-chain runtime verification requires v4 evidence.
//! Leader identity and active-key selection remain off-chain transport concerns. Authenticated
//! HTTPS must still protect request bodies and unsigned headers on every network hop.

pub mod keys;
pub mod v2;
pub mod v3;
pub mod v4;
