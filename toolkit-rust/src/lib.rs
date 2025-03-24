//! # Nexus Toolkit
//!
//! The Nexus Toolkit is a Rust library that provides a trait to define a Nexus
//! Tool. A Nexus Tool is a service that can be invoked over HTTP. The Toolkit
//! automatically generates the necessary endpoints for the Tool.
//!
//! See more documentation at <https://github.com/Talus-Network/gitbook-docs/nexus-sdk/toolkit-rust.md>

mod nexus_tool;
mod runtime;
mod secret;

pub use {
    anyhow::Result as AnyResult,
    nexus_tool::NexusTool,
    runtime::routes_for_,
    secret::{BestEncryptionEver, EncryptionStrategy, Secret},
    warp::{self, http::StatusCode},
};
