//! # Nexus Toolkit
//!
//! The Nexus Toolkit is a Rust library that provides a trait to define a Nexus
//! Tool. A Nexus Tool is a service that can be invoked over HTTP. The Toolkit
//! automatically generates the necessary endpoints for the Tool.
//!
//! See more documentation at <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

mod nexus_tool;
mod runtime;
mod serde_tracked;

pub use {
    anyhow::Result as AnyResult,
    env_logger,
    log::debug,
    nexus_tool::NexusTool,
    runtime::routes_for_,
    serde_tracked::*,
    warp::{self, http::StatusCode},
};
