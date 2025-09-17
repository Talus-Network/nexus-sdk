//! Error types for HTTP tool

use {
    reqwest::Error as ReqwestError,
    serde_json::Error as JsonError,
    thiserror::Error,
    url::ParseError as UrlParseError,
};

/// HTTP tool errors
#[derive(Error, Debug)]
pub enum HttpToolError {
    #[error("HTTP error {status}: {reason}")]
    Http {
        status: u16,
        reason: String,
        snippet: String,
    },

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] JsonError),

    #[error("Schema validation failed: {errors:?}")]
    SchemaValidation { errors: Vec<String> },

    #[error("Network error: {0}")]
    Network(#[from] ReqwestError),

    #[error("Input validation error: {0}")]
    Input(String),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] UrlParseError),

    #[error("Base64 decode error: {0}")]
    Base64Decode(String),
}

impl HttpToolError {
    /// Convert HttpToolError to Output enum for API compatibility
    pub fn to_output(self) -> crate::http::Output {
        match self {
            HttpToolError::Http {
                status,
                reason,
                snippet,
            } => crate::http::Output::ErrHttp {
                status,
                reason,
                snippet,
            },
            HttpToolError::JsonParse(e) => crate::http::Output::ErrJsonParse { msg: e.to_string() },
            HttpToolError::SchemaValidation { errors } => {
                crate::http::Output::ErrSchemaValidation { errors }
            }
            HttpToolError::Network(e) => crate::http::Output::ErrNetwork { msg: e.to_string() },
            HttpToolError::Input(msg) => crate::http::Output::ErrInput { msg },
            HttpToolError::UrlParse(e) => crate::http::Output::ErrInput {
                msg: format!("URL parse error: {}", e),
            },
            HttpToolError::Base64Decode(msg) => crate::http::Output::ErrInput {
                msg: format!("Base64 decode error: {}", msg),
            },
        }
    }
}
