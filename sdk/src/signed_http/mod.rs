//! Leader-to-Tool signing transport.
//!
//! The offchain tool prepares signature consumed by the onchain RegisteredKey
//! verifier:
//! - the Leader signs `SHA-256(BCS(canonical_tool_inputs))`;
//! - the Tool signs `domain || leader_signature || deterministic_nonce || SHA-256(result_bytes)`;
//! - `result_bytes` is the exact BCS `TaggedOutput` submitted onchain.
//!
//! Leader identity and active-key selection remain offchain transport concerns. The deterministic
//! nonce is bound by the Tool signature and verified against authoritative execution context
//! onchain. Authenticated HTTPS must still protect the request body and unsigned headers on every
//! network hop; these minimal onchain signatures do not make plaintext HTTP safe.

pub mod keys;
pub mod v2;
