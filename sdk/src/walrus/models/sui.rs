use serde::{Deserialize, Serialize};

/// Represents a Sui transaction reference
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionReference {
    /// The Sui transaction digest
    pub tx_digest: String,
    /// The Sui object ID
    pub object_id: String,
}
