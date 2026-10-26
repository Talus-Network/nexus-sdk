//! Signed Tool transport v3.
//!
//! This protocol signs Tool input commitments, receiver context, and ordered BCS `OffchainToolOutput` bytes. See [`wire`] for the header and signature contract.

pub mod error;
pub mod wire;

#[cfg(test)]
mod tests;
