use {
    crate::list::models::{Includes, Meta},
    reqwest::{Response, StatusCode},
    serde::{de::Error, Deserialize, Serialize},
    serde_json::Value,
    std::fmt,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
}

/// Error type for Twitter operations
#[derive(Debug)]
pub enum TwitterError {
    Network(reqwest::Error),
    ParseError(serde_json::Error),
    ApiError(String, String, String),
    StatusError(StatusCode),
    Other(String),
}

impl fmt::Display for TwitterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TwitterError::Network(e) => write!(f, "Network error: {}", e),
            TwitterError::ParseError(e) => write!(f, "Response parsing error: {}", e),
            TwitterError::ApiError(title, error_type, detail) => {
                write!(
                    f,
                    "Twitter API error: {} (type: {}){}",
                    title, error_type, detail
                )
            }
            TwitterError::StatusError(status) => write!(f, "Twitter API status error: {}", status),
            TwitterError::Other(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl From<reqwest::Error> for TwitterError {
    fn from(err: reqwest::Error) -> Self {
        TwitterError::Network(err)
    }
}

impl From<serde_json::Error> for TwitterError {
    fn from(err: serde_json::Error) -> Self {
        TwitterError::ParseError(err)
    }
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

/// Helper function to parse Twitter API response
pub async fn parse_twitter_response_v2<T>(
    response: Response,
) -> TwitterResult<(T, Option<Includes>, Option<Meta>)>
where
    T: for<'de> Deserialize<'de> + std::fmt::Debug,
{
    // If response is successful, parse the response as JSON
    if response.status().is_success() {
        match response.text().await {
            Ok(text) => {
                let value: serde_json::Value = serde_json::from_str(&text)?;
                if let Some(errors) = value.get("errors") {
                    if let Some(error) = errors.as_array().and_then(|e| e.first()) {
                        let title = error
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Unknown Error");
                        let error_type = error
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");

                        return Err(TwitterError::ApiError(
                            title.to_string(),
                            error_type.to_string(),
                            "".to_string(),
                        ));
                    }
                }

                let data = if let Some(data) = value.get("data") {
                    serde_json::from_value(data.clone())?
                } else {
                    serde_json::from_value(value.clone())?
                };

                let includes = if let Some(includes) = value.get("includes") {
                    Some(serde_json::from_value(includes.clone())?)
                } else {
                    None
                };

                let meta = if let Some(meta) = value.get("meta") {
                    Some(serde_json::from_value(meta.clone())?)
                } else {
                    None
                };

                Ok((data, includes, meta))
            }
            Err(e) => Err(TwitterError::ParseError(serde_json::Error::custom(
                e.to_string(),
            ))),
        }
    } else {
        match response.text().await {
            Ok(text) => {
                if let Ok(twitter_api_error) = serde_json::from_str::<TwitterApiError>(&text) {
                    return Err(TwitterError::from_api_error(&twitter_api_error));
                } else if let Ok(error_response) =
                    serde_json::from_str::<TwitterDefaultError>(&text)
                {
                    return Err(TwitterError::ApiError(
                        "Twitter API Error".to_string(),
                        "default".to_string(),
                        format!(
                            " - {} (Code: {})",
                            error_response.message, error_response.code
                        ),
                    ));
                } else {
                    return Err(TwitterError::ParseError(serde_json::Error::custom(
                        "Unknown error".to_string(),
                    )));
                }
            }
            Err(e) => Err(TwitterError::Network(e)),
        }
    }
}
