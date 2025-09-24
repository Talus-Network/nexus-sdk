//! Error types for HTTP tool

use {
    reqwest::Error as ReqwestError,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Error as JsonError,
    thiserror::Error,
    url::ParseError as UrlParseError,
};

/// HTTP error kinds for external API
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HttpErrorKind {
    /// HTTP error response (4xx, 5xx)
    #[serde(rename = "err_http")]
    Http,
    /// JSON parsing error
    #[serde(rename = "err_json_parse")]
    JsonParse,
    /// Schema validation error
    #[serde(rename = "err_schema_validation")]
    SchemaValidation,
    /// Network connectivity error
    #[serde(rename = "err_network")]
    Network,
    /// Request timeout error
    #[serde(rename = "err_timeout")]
    Timeout,
    /// Input validation error
    #[serde(rename = "err_input")]
    Input,
    /// URL parsing error
    #[serde(rename = "err_url_parse")]
    UrlParse,
    /// Base64 decoding error
    #[serde(rename = "err_base64_decode")]
    Base64Decode,
}

/// Input validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Schema validation requires expect_json=true")]
    SchemaRequiresJson,
    #[error("Invalid timeout: {0}")]
    InvalidTimeout(String),
    #[error("Invalid retries: {0}")]
    InvalidRetries(String),
    #[error("Multipart field name cannot be empty")]
    EmptyMultipartFieldName,
    #[error("Multipart field value cannot be empty")]
    EmptyMultipartFieldValue,
    #[error("Raw body data cannot be empty")]
    EmptyRawBody,
    #[error("Raw body data must be valid base64")]
    InvalidBase64Data,
    #[error("Form body data cannot be empty")]
    EmptyFormData,
    #[error("JSON body data cannot be null")]
    NullJsonData,
}

/// HTTP tool errors (internal)
#[derive(Error, Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HttpToolError {
    #[error("HTTP error {status}: {reason}")]
    ErrHttp {
        status: u16,
        reason: String,
        snippet: String,
    },

    #[error("JSON parse error: {0}")]
    ErrJsonParse(String),

    #[error("Schema validation failed: {errors:?}")]
    ErrSchemaValidation { errors: Vec<String> },

    #[error("Network error: {0}")]
    ErrNetwork(String),

    #[error("Request timeout: {0}")]
    ErrTimeout(String),

    #[error("Input validation error: {0}")]
    ErrInput(String),

    #[error("URL parse error: {0}")]
    ErrUrlParse(String),

    #[error("Base64 decode error: {0}")]
    ErrBase64Decode(String),
}

impl HttpToolError {
    /// Convert HttpToolError to Output enum for API compatibility
    pub fn to_output(self) -> crate::http::Output {
        match self {
            HttpToolError::ErrHttp {
                status,
                reason,
                snippet: _,
            } => crate::http::Output::Err {
                reason: format!("HTTP error {}: {}", status, reason),
                kind: HttpErrorKind::Http,
                status_code: Some(status),
            },
            HttpToolError::ErrJsonParse(msg) => crate::http::Output::Err {
                reason: format!("JSON parse error: {}", msg),
                kind: HttpErrorKind::JsonParse,
                status_code: None,
            },
            HttpToolError::ErrSchemaValidation { errors } => crate::http::Output::Err {
                reason: format!("Schema validation failed: {} errors", errors.len()),
                kind: HttpErrorKind::SchemaValidation,
                status_code: None,
            },
            HttpToolError::ErrNetwork(msg) => crate::http::Output::Err {
                reason: format!("Network error: {}", msg),
                kind: HttpErrorKind::Network,
                status_code: None,
            },
            HttpToolError::ErrTimeout(msg) => crate::http::Output::Err {
                reason: format!("Request timeout: {}", msg),
                kind: HttpErrorKind::Timeout,
                status_code: None,
            },
            HttpToolError::ErrInput(msg) => crate::http::Output::Err {
                reason: format!("Input validation error: {}", msg),
                kind: HttpErrorKind::Input,
                status_code: None,
            },
            HttpToolError::ErrUrlParse(msg) => crate::http::Output::Err {
                reason: format!("URL parse error: {}", msg),
                kind: HttpErrorKind::UrlParse,
                status_code: None,
            },
            HttpToolError::ErrBase64Decode(msg) => crate::http::Output::Err {
                reason: format!("Base64 decode error: {}", msg),
                kind: HttpErrorKind::Base64Decode,
                status_code: None,
            },
        }
    }

    /// Create HttpToolError from external error types
    pub fn from_json_error(e: JsonError) -> Self {
        Self::ErrJsonParse(e.to_string())
    }

    pub fn from_network_error(e: ReqwestError) -> Self {
        let error_msg = e.to_string();
        if e.is_timeout() {
            Self::ErrTimeout(error_msg)
        } else {
            // Check if it's a timeout by looking at the error message
            if error_msg.contains("timeout") || error_msg.contains("timed out") {
                Self::ErrTimeout(error_msg)
            } else {
                Self::ErrNetwork(error_msg)
            }
        }
    }

    pub fn from_url_parse_error(e: UrlParseError) -> Self {
        Self::ErrUrlParse(e.to_string())
    }

    pub fn from_validation_error(e: ValidationError) -> Self {
        Self::ErrInput(e.to_string())
    }
}
