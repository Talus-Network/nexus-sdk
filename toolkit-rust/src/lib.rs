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
mod serde_tracked;
mod tls_utils;

pub use {
    anyhow::Result as AnyResult,
    env_logger,
    log::debug,
    nexus_tool::NexusTool,
    runtime::routes_for_,
    secret::{BestEncryptionEver, EncryptionStrategy, Secret},
    serde_tracked::*,
    tls_utils::{
        compute_key_hash,
        generate_key_and_hash,
        reqwest_with_pin,
        server_cfg,
        spawn_tls_server,
        spki_sha256,
        Pinned,
    },
    tokio_rustls,
    warp::{self, http::StatusCode},
};

/// TLS utilities for key generation and configuration
pub mod tls {
    pub use crate::tls_utils::*;
}
