use serde::{Deserialize, Serialize};

/// Represents a Sui blockchain event
#[derive(Debug, Deserialize, Serialize)]
pub struct SuiEvent {
    #[serde(rename = "txDigest")]
    pub tx_digest: String,
}
