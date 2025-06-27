use {
    reqwest::{Response, StatusCode},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    thiserror::Error,
};

/// Error kind enumeration for Twitter operations
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TwitterErrorKind {
    /// Network-related error
    Network,
    /// Connection error
    Connection,
    /// Timeout error
    Timeout,
    /// Error parsing response
    Parse,
    /// Authentication/authorization error
    Auth,
    /// Resource not found
    NotFound,
    /// Rate limit exceeded
    RateLimit,
    /// Server error
    Server,
    /// Forbidden access
    Forbidden,
    /// API-specific error
    Api,
    /// Unknown error
    Unknown,
    /// Validation error
    Validation,
}

/// A Twitter API error returned by the API
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct TwitterApiError {
    pub title: String,
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
}

/// Error type for Twitter operations
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum TwitterError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Response parsing error: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Twitter API error: {0} (type: {1}){2}")]
    ApiError(Box<str>, Box<str>, Box<str>),

    #[error("Twitter API status error: {0}")]
    StatusError(StatusCode),

    #[error("Unknown error: {0}")]
    Other(Box<str>),
}

/// Standard error response structure for Twitter tools
#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterErrorResponse {
    /// Detailed error message
    pub reason: String,
    /// Type of error (network, server, auth, etc.)
    pub kind: TwitterErrorKind,
    /// HTTP status code if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

impl TwitterError {
    /// Create a new error from a Twitter API error object
    pub fn from_api_error(error: &TwitterApiError) -> Self {
        let detail = error
            .detail
            .clone()
            .map_or_else(String::new, |d| format!(" - {}", d));

        TwitterError::ApiError(
            error.title.as_str().into(),
            error.error_type.as_str().into(),
            detail.into(),
        )
    }

    /// Convert the error to a standardized TwitterErrorResponse
    pub fn to_error_response(&self) -> TwitterErrorResponse {
        match self {
            TwitterError::Network(req_err) => {
                let kind = if req_err.is_timeout() {
                    TwitterErrorKind::Timeout
                } else if req_err.is_connect() {
                    TwitterErrorKind::Connection
                } else {
                    TwitterErrorKind::Network
                };

                TwitterErrorResponse {
                    reason: self.to_string(),
                    kind,
                    status_code: None,
                }
            }
            TwitterError::ParseError(_) => TwitterErrorResponse {
                reason: self.to_string(),
                kind: TwitterErrorKind::Parse,
                status_code: None,
            },
            TwitterError::ApiError(title, error_type, _) => {
                // Extract error kind and status code from API errors
                let (kind, code) = if error_type.contains("rate") || title.contains("Rate") {
                    (TwitterErrorKind::RateLimit, Some(429))
                } else if error_type.contains("auth") || title.contains("Unauthorized") {
                    (TwitterErrorKind::Auth, Some(401))
                } else if error_type.contains("not-found") || title.contains("Not Found") {
                    (TwitterErrorKind::NotFound, Some(404))
                } else if error_type.contains("forbidden") {
                    (TwitterErrorKind::Forbidden, Some(403))
                } else if error_type.contains("server") {
                    (TwitterErrorKind::Server, Some(500))
                } else {
                    (TwitterErrorKind::Api, None)
                };

                TwitterErrorResponse {
                    reason: self.to_string(),
                    kind,
                    status_code: code,
                }
            }
            TwitterError::StatusError(status) => {
                let code = status.as_u16();
                let kind = if code == 429 {
                    TwitterErrorKind::RateLimit
                } else if code == 401 {
                    TwitterErrorKind::Auth
                } else if code == 403 {
                    TwitterErrorKind::Forbidden
                } else if code == 404 {
                    TwitterErrorKind::NotFound
                } else if code >= 500 {
                    TwitterErrorKind::Server
                } else {
                    TwitterErrorKind::Unknown
                };

                TwitterErrorResponse {
                    reason: self.to_string(),
                    kind,
                    status_code: Some(code),
                }
            }
            TwitterError::Other(_) => TwitterErrorResponse {
                reason: self.to_string(),
                kind: TwitterErrorKind::Unknown,
                status_code: None,
            },
        }
    }
}

/// Result type for Twitter operations
pub type TwitterResult<T> = Result<T, TwitterError>;

#[derive(Debug, Serialize, Deserialize)]
struct TwitterDefaultError {
    code: i32,
    message: String,
}

/// Parse a successful Twitter API response
fn parse_successful_twitter_response<T>(text: String) -> TwitterResult<T>
where
    T: for<'de> Deserialize<'de> + std::fmt::Debug,
{
    // Try to parse response as JSON
    let parsed = serde_json::from_str::<T>(&text).map_err(TwitterError::ParseError)?;

    // Check if the parsed response has errors field
    if let Ok(value) = serde_json::from_str::<Value>(&text) {
        if let Some(errors) = value.get("errors").and_then(|e| e.as_array()) {
            if let Some(first_error) = errors.first() {
                if let Ok(twitter_error) =
                    serde_json::from_value::<TwitterApiError>(first_error.clone())
                {
                    return Err(TwitterError::from_api_error(&twitter_error));
                }

                return Err(parse_error_from_json(first_error));
            }
        }
    }

    Ok(parsed)
}

/// Parse a failed Twitter API response
fn parse_failed_twitter_response<T>(text: String, status: StatusCode) -> TwitterResult<T> {
    // Try to parse as default Twitter error format
    if let Ok(default_error) = serde_json::from_str::<TwitterDefaultError>(&text) {
        let (error_type, title) = match default_error.code {
            32 => ("authentication", "Unauthorized"),
            88 => ("rate_limit", "Rate Limit Exceeded"),
            34 => ("not-found", "Not Found Error"),
            _ => ("default", "Twitter API Error"),
        };

        return Err(TwitterError::ApiError(
            title.into(),
            error_type.into(),
            format!(
                " - {} (Code: {})",
                default_error.message, default_error.code
            )
            .into(),
        ));
    }

    // Try to parse as error response with errors array
    if let Ok(error_response) = serde_json::from_str::<Value>(&text) {
        if let Some(errors) = error_response.get("errors").and_then(|e| e.as_array()) {
            if let Some(first_error) = errors.first() {
                return Err(parse_error_from_json(first_error));
            }
        }
    }

    // If we couldn't parse the error response, return the status code
    Err(TwitterError::StatusError(status))
}

/// Parse error details from a JSON Value
fn parse_error_from_json(error: &Value) -> TwitterError {
    let code = error.get("code").and_then(|c| c.as_i64());

    let title = match code {
        Some(32) => "Unauthorized",
        Some(88) => "Rate Limit Exceeded",
        Some(34) => "Not Found Error",
        _ => error
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Unknown Error"),
    };

    let error_type = match code {
        Some(32) => "authentication",
        Some(88) => "rate_limit",
        Some(34) => "not-found",
        _ => error
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown"),
    };

    let mut detail = String::new();
    if let Some(d) = error.get("detail").and_then(|d| d.as_str()) {
        detail.push_str(&format!(" - {}", d));
    }
    if let Some(message) = error.get("message").and_then(|m| m.as_str()) {
        detail.push_str(&format!(" - {}", message));
    }

    TwitterError::ApiError(title.into(), error_type.into(), detail.into())
}

/// Helper function to parse Twitter API response
pub async fn parse_twitter_response<T>(response: Response) -> TwitterResult<T>
where
    T: for<'de> Deserialize<'de> + std::fmt::Debug,
{
    let status = response.status();
    let text = response.text().await.map_err(TwitterError::Network)?;

    // Handle empty response for 204 No Content
    if status == reqwest::StatusCode::NO_CONTENT && text.is_empty() {
        // For EmptyResponse type, return empty response
        if std::any::type_name::<T>()
            == std::any::type_name::<crate::media::models::EmptyResponse>()
        {
            return serde_json::from_value(serde_json::json!({})).map_err(TwitterError::ParseError);
        }
    }

    if status.is_success() {
        parse_successful_twitter_response(text)
    } else {
        parse_failed_twitter_response(text, status)
    }
}
