//! This module contains identifiers for all Nexus (and some Sui) Move
//! resources. Also exports some helper functions to cut down on boilerplate
//! code especially when creating Move resources from string.
//!
//! # Example
//!
//! ```ignore
//! use crate::idents::workflow;
//!
//! let vertex = workflow::Dag::vertex_from_str(&mut tx, workflow_pkg_id, "my_vertex");
//! ```

pub mod move_std;
pub mod primitives;
pub mod sap;
pub mod sui_framework;
pub mod workflow;

use crate::sui;

/// This struct is used to define Nexus Move resources as `const`s.
pub struct ModuleAndNameIdent {
    pub module: &'static sui::MoveIdentStr,
    pub name: &'static sui::MoveIdentStr,
}
