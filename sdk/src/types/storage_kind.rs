//! Defines the `StorageKind` enum which specifies the storage method for data in
//! the DAG.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Inline,
    Walrus,
}
