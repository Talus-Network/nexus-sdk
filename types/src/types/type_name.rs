//! Ubiqutously used name wrapper type. Useful to have this defined globally
//! so that we don't have to redefine it in every module that uses it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeName {
    pub name: String,
}
