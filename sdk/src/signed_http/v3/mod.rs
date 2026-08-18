//! Signed Tool transport v3.
//!
//! This protocol signs ordered BCS `OffchainToolOutput` bytes with the direct on-chain transcript. See [`wire`] for the complete header and signature contract.

pub mod error;
pub mod wire;

#[cfg(test)]
mod tests;
