use {
    reqwest::{Response, StatusCode},
    serde::{Deserialize, Serialize},
    serde_json::Value,
    thiserror::Error,
};

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

impl TwitterError {
    /// Create a new error from a Twitter API error object
    pub fn from_api_error(error: &TwitterApiError) -> Self {
        let detail = error
            .detail
            .clone()
            .map_or_else(String::new, |d| format!(" - {}", d));

        TwitterError::ApiError(error.title.clone(), error.error_type.clone(), detail)
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
                    return Err(TwitterError::ApiError(
                        "Twitter API Error".to_string(),
                        "default".to_string(),
                        format!(
                            " - {} (Code: {})",
                            default_error.message, default_error.code
                        ),
                    ));
                }

                if let Ok(error_response) = serde_json::from_str::<Value>(&text) {
                    if let Some(errors) = error_response.get("errors").and_then(|e| e.as_array()) {
                        if let Some(first_error) = errors.first() {
                            let title = error_response
                                .get("title")
                                .and_then(|t| t.as_str())
                                .unwrap_or("Unknown Error")
                                .to_string();

                            let error_type = error_response
                                .get("type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("unknown")
                                .to_string();

                            let mut detail = String::new();

                            if let Some(d) = error_response.get("detail").and_then(|d| d.as_str()) {
                                detail.push_str(&format!(" - {}", d));
                            }

                            if let Some(message) =
                                first_error.get("message").and_then(|m| m.as_str())
                            {
                                detail.push_str(&format!(" - {}", message));
                            }

                            return Err(TwitterError::ApiError(title, error_type, detail));
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
                                    if let Some(twitter_error) =
                                        serde_json::from_value::<TwitterApiError>(
                                            first_error.clone(),
                                        )
                                        .ok()
                                    {
                                        return Err(TwitterError::from_api_error(&twitter_error));
                                    } else {
                                        let title = first_error
                                            .get("title")
                                            .and_then(|t| t.as_str())
                                            .unwrap_or("Unknown Error")
                                            .to_string();

                                        let error_type = first_error
                                            .get("type")
                                            .and_then(|t| t.as_str())
                                            .unwrap_or("unknown")
                                            .to_string();

                                        let detail = first_error
                                            .get("detail")
                                            .and_then(|d| d.as_str())
                                            .map(|s| format!(" - {}", s))
                                            .unwrap_or_default();

                                        return Err(TwitterError::ApiError(
                                            title, error_type, detail,
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
