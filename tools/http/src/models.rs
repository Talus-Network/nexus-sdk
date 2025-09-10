//! Data models for HTTP Generic tool

use {
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

/// Input model for the HTTP Generic tool
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: String,
    
    /// Complete URL
    pub url: String,
}

/// Output model for the HTTP Generic tool
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    /// Successful response
    Ok {
        /// HTTP status code
        status: u16,
        /// Response body
        body: String,
    },
    /// Error response
    Err {
        /// Error message
        message: String,
    },
}
