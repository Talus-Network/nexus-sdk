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
}

/// A Twitter API error returned by the API
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    ApiError(String, String, String),

    #[error("Twitter API status error: {0}")]
    StatusError(StatusCode),

    #[error("Unknown error: {0}")]
    Other(String),
}

/// Standard error response structure for Twitter tools
#[derive(Debug, Serialize, Deserialize)]
pub struct TwitterErrorResponse {
    /// Type of error (network, server, auth, etc.)
    pub kind: TwitterErrorKind,
    /// Detailed error message
    pub reason: String,
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

        TwitterError::ApiError(error.title.clone(), error.error_type.clone(), detail)
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
                    kind,
                    reason: self.to_string(),
                    status_code: None,
                }
            }
            TwitterError::ParseError(_) => TwitterErrorResponse {
                kind: TwitterErrorKind::Parse,
                reason: self.to_string(),
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
                    kind,
                    reason: self.to_string(),
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
                    kind,
                    reason: self.to_string(),
                    status_code: Some(code),
                }
            }
            TwitterError::Other(_) => TwitterErrorResponse {
                kind: TwitterErrorKind::Unknown,
                reason: self.to_string(),
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

/// Helper function to parse Twitter API response
pub async fn parse_twitter_response<T>(response: Response) -> TwitterResult<T>
where
    T: for<'de> Deserialize<'de> + std::fmt::Debug,
{
    // Check if response is successful
    if !response.status().is_success() {
        let status = response.status();

        // Try to parse error response
        match response.text().await {
            Ok(text) => {
                // Try to parse as default Twitter error format
                if let Ok(default_error) = serde_json::from_str::<TwitterDefaultError>(&text) {
                    // Check for specific error codes
                    let error_type = match default_error.code {
                        32 => "authentication", // Auth error
                        88 => "rate_limit",     // Rate limit
                        34 => "not-found",      // Not found
                        _ => "default",
                    };

                    // Map common error codes to better titles
                    let title = match default_error.code {
                        32 => "Unauthorized",
                        88 => "Rate Limit Exceeded",
                        34 => "Not Found Error",
                        _ => "Twitter API Error",
                    };

                    return Err(TwitterError::ApiError(
                        title.to_string(),
                        error_type.to_string(),
                        format!(
                            " - {} (Code: {})",
                            default_error.message, default_error.code
                        ),
                    ));
                }

                if let Ok(error_response) = serde_json::from_str::<Value>(&text) {
                    if let Some(errors) = error_response.get("errors").and_then(|e| e.as_array()) {
                        if let Some(first_error) = errors.first() {
                            // Check for code in the error object
                            let code = first_error.get("code").and_then(|c| c.as_i64());

                            // Set title based on code or fallback to title field
                            let title = match code {
                                Some(32) => "Unauthorized",
                                Some(88) => "Rate Limit Exceeded",
                                Some(34) => "Not Found Error",
                                _ => error_response
                                    .get("title")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("Unknown Error"),
                            };

                            // Set error_type based on code or fallback to type field
                            let error_type = match code {
                                Some(32) => "authentication",
                                Some(88) => "rate_limit",
                                Some(34) => "not-found",
                                _ => error_response
                                    .get("type")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("unknown"),
                            };

                            let mut detail = String::new();

                            if let Some(d) = error_response.get("detail").and_then(|d| d.as_str()) {
                                detail.push_str(&format!(" - {}", d));
                            }

                            if let Some(message) =
                                first_error.get("message").and_then(|m| m.as_str())
                            {
                                detail.push_str(&format!(" - {}", message));
                            }

                            return Err(TwitterError::ApiError(
                                title.to_string(),
                                error_type.to_string(),
                                detail,
                            ));
                        }
                    }
                }

                // If we couldn't parse the error response, return the status code
                Err(TwitterError::StatusError(status))
            }
            Err(e) => Err(TwitterError::Network(e)),
        }
    } else {
        // Try to parse response as JSON
        match response.text().await {
            Ok(text) => {
                match serde_json::from_str::<T>(&text) {
                    Ok(parsed) => {
                        // Check if the parsed response has errors field
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            if let Some(errors) = value.get("errors").and_then(|e| e.as_array()) {
                                if let Some(first_error) = errors.first() {
                                    // Check for code in the error object
                                    let code = first_error.get("code").and_then(|c| c.as_i64());

                                    if let Some(twitter_error) =
                                        serde_json::from_value::<TwitterApiError>(
                                            first_error.clone(),
                                        )
                                        .ok()
                                    {
                                        return Err(TwitterError::from_api_error(&twitter_error));
                                    } else {
                                        // Set title based on code or fallback to title field
                                        let title = match code {
                                            Some(32) => "Unauthorized",
                                            Some(88) => "Rate Limit Exceeded",
                                            Some(34) => "Not Found Error",
                                            _ => first_error
                                                .get("title")
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("Unknown Error"),
                                        };

                                        // Set error_type based on code or fallback to type field
                                        let error_type = match code {
                                            Some(32) => "authentication",
                                            Some(88) => "rate_limit",
                                            Some(34) => "not-found",
                                            _ => first_error
                                                .get("type")
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("unknown"),
                                        };

                                        let detail = first_error
                                            .get("detail")
                                            .and_then(|d| d.as_str())
                                            .map(|s| format!(" - {}", s))
                                            .unwrap_or_default();

                                        // If there's a message field, append it to the detail
                                        let detail_with_message = if let Some(message) =
                                            first_error.get("message").and_then(|m| m.as_str())
                                        {
                                            if detail.is_empty() {
                                                format!(" - {}", message)
                                            } else {
                                                format!("{} - {}", detail, message)
                                            }
                                        } else {
                                            detail
                                        };

                                        return Err(TwitterError::ApiError(
                                            title.to_string(),
                                            error_type.to_string(),
                                            detail_with_message,
                                        ));
                                    }
                                }
                            }
                        }

                        Ok(parsed)
                    }
                    Err(e) => Err(TwitterError::ParseError(e)),
                }
            }
            Err(e) => Err(TwitterError::Network(e)),
        }
    }
}
