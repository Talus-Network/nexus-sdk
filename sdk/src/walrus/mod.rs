//! Walrus client module provides integration with the Walrus decentralized blob storage system.
//!
//! This module allows for:
//! - Uploading files to the Walrus network
//! - Uploading bytes to the Walrus network
//! - Downloading files from the Walrus network
//! - Reading bytes from the Walrus network
//! - Verifying the existence of files in the Walrus network

mod client;
mod models;
#[cfg(feature = "types")]
mod nexus_data;
#[cfg(feature = "types")]
mod reader;

// Re-exports
#[cfg(feature = "types")]
pub use nexus_data::*;
#[cfg(feature = "types")]
pub use reader::*;
pub use {client::*, models::*};

#[cfg(feature = "types")]
mod reference;
#[cfg(feature = "types")]
pub use reference::*;
#[cfg(feature = "walrus_native")]
mod native;
#[cfg(feature = "walrus_native")]
pub use native::*;

#[cfg(feature = "types")]
mod upload_data;
#[cfg(feature = "types")]
pub use upload_data::*;
