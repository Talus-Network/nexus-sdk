//! Leader-to-Tool signing transport.
//!
//! The offchain Tool prepares the signature consumed by the onchain RegisteredKey verifier.
//! Legacy v2 signs `TaggedOutput` bytes with its v2 response domain; active v3 signs ordered BCS
//! `OffchainToolOutput` bytes with the direct on-chain transcript:
//! - the Leader signs the schema-ordered `PortCommitment` input hash;
//! - the Tool signs `domain || leader_signature || deterministic_nonce || SHA-256(result_bytes)`;
//! - `result_bytes` is the exact ordered BCS body reconstructed from typed fields onchain.
//!
//! Leader identity and active-key selection remain offchain transport concerns. The deterministic
//! nonce is bound by the Tool signature and verified against authoritative execution context
//! onchain. Authenticated HTTPS must still protect the request body and unsigned headers on every
//! network hop; these minimal onchain signatures do not make plaintext HTTP safe.

pub mod keys;
pub mod v2;
pub mod v3;
