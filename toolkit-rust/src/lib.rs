//! # Nexus Toolkit
//!
//! The Nexus Toolkit is a Rust library that provides a trait to define a Nexus
//! Tool. A Nexus Tool is a service that can be invoked over HTTP. The Toolkit
//! automatically generates the necessary endpoints for the Tool.
//!
//! See more documentation at <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

mod config;
mod nexus_tool;
#[doc(hidden)]
pub mod runtime;
mod serde_tracked;
mod signed_http_warp;

pub use {
    anyhow::Result as AnyResult,
    config::{SignedHttpMode, ToolkitRuntimeConfig, ENV_TOOLKIT_CONFIG_PATH},
    env_logger,
    log::debug,
    nexus_tool::{AuthContext, NexusTool},
    runtime::{routes_for_, routes_for_with_config_},
    serde_tracked::*,
    warp::{self, http::StatusCode},
};
