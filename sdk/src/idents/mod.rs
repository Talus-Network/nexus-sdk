//! This module contains identifiers for all Nexus (and some Sui) Move
//! resources. Also exports some helper functions to cut down on boilerplate
//! code especially when creating Move resources from string.
//!
//! # Example
//!
//! ```
//! use nexus_sdk::sui;
//! use nexus_sdk::idents::interface;
//!
//! let mut tx = sui::tx::TransactionBuilder::new();
//! let interface_pkg_id = sui::types::Address::generate(&mut rand::thread_rng());
//! let vertex = interface::Graph::vertex_from_str(&mut tx, interface_pkg_id, "my_vertex");
//!
//! assert!(vertex.is_ok());
//! ```

pub mod interface;
pub mod move_std;
pub mod primitives;
pub mod registry;
pub mod scheduler;
pub mod sui_framework;
pub mod tap;
pub mod workflow;

use crate::sui;

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

/// Normalize package dependency IDs for Sui publish commands.
///
/// Current Sui clients reject a `Publish` command with an empty dependency
/// vector. Test fixtures and simple user packages can still depend only on the
/// implicit framework packages, which the compiler may report as an empty
/// storage-dependency list. In that case, include the fixed framework package
/// IDs so the publish command remains valid without changing package code.
#[cfg_attr(not(feature = "move_publish"), allow(dead_code))]
pub(crate) fn publish_dependency_ids_or_framework_defaults(
    dependency_ids: impl IntoIterator<Item = sui::types::Address>,
) -> Vec<sui::types::Address> {
    let dependency_ids = dependency_ids.into_iter().collect::<Vec<_>>();
    if dependency_ids.is_empty() {
        vec![move_std::PACKAGE_ID, sui_framework::PACKAGE_ID]
    } else {
        dependency_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_dependency_ids_defaults_frameworks_when_empty() {
        let dependencies = publish_dependency_ids_or_framework_defaults([]);

        assert_eq!(
            dependencies,
            vec![move_std::PACKAGE_ID, sui_framework::PACKAGE_ID]
        );
    }

    #[test]
    fn publish_dependency_ids_preserves_compiler_dependencies() {
        let package = sui::types::Address::from_static("0x42");

        let dependencies = publish_dependency_ids_or_framework_defaults([package]);

        assert_eq!(dependencies, vec![package]);
    }
}
