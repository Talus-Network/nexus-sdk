//! Signed Tool transport v2.
//!
//! This hard-cutover protocol has no HTTP claims object or transcript. See [`wire`] for the
//! complete header and signature contract.

pub mod error;
pub mod wire;

#[cfg(test)]
mod tests;
