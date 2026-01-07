//! This module contains identifiers for all Nexus (and some Sui) Move
//! resources. Also exports some helper functions to cut down on boilerplate
//! code especially when creating Move resources from string.
//!
//! # Example
//!
//! ```
//! use nexus_sdk::sui;
//! use nexus_sdk::idents::workflow;
//!
//! let mut tx = sui::tx::TransactionBuilder::new();
//! let workflow_pkg_id = sui::types::Address::generate(&mut rand::thread_rng());
//! let vertex = workflow::Dag::vertex_from_str(&mut tx, workflow_pkg_id, "my_vertex");
//!
//! assert!(matches!(vertex, Ok(sui::types::Argument::Result(_))));
//! ```

pub mod move_std;
pub mod primitives;
pub mod sui_framework;
pub mod tap;
pub mod workflow;

use {
    crate::sui,
    base64::{prelude::BASE64_STANDARD as BASE64, Engine},
    serde::Serialize,
};

/// This struct is used to define Nexus Move resources as `const`s.
pub struct ModuleAndNameIdent {
    pub module: sui::types::Identifier,
    pub name: sui::types::Identifier,
}

impl ModuleAndNameIdent {
    /// Returns the fully-qualified string for this identifier under the given package ID.
    pub fn qualified_name(&self, package: sui::types::Address) -> String {
        format!("{package}::{}::{}", self.module, self.name)
    }
}

/// Helper to create a pure [`sui::tx::Input`].
pub fn pure_arg<T: Serialize>(value: &T) -> anyhow::Result<sui::tx::Input> {
    Ok(sui::tx::Input {
        value: Some(sui::tx::Value::String(BASE64.encode(bcs::to_bytes(value)?))),
        kind: Some(sui::tx::InputKind::Pure),
        ..Default::default()
    })
}
