//! # `xyz.taluslabs.social.twitter.create-list@1`
//!
//! Standard Nexus Tool that creates a list on Twitter.

use {
    crate::{auth::TwitterAuth, tweet::TWITTER_API_BASE},
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,
    /// The name of the list to create (1-25 characters)
    #[schemars(length(min = 1, max = 25))]
    name: String,
    /// The description of the list to create (max 100 characters)
    #[schemars(length(max = 100))]
    description: String,
    /// The privacy setting of the list to create
    /// - public: The list is public and can be viewed by anyone (default)
    /// - private: The list is private and can only be viewed by the user who created it
    #[serde(default)]
    private: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ListResponse {
    /// Tweet's unique identifier
    id: String,
    /// The name of the list
    name: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The successful tweet response data
        id: String,
        /// The name of the list
        name: String,
    },
    Err {
        /// Error message if the tweet failed
        reason: String,
    },
}

pub(crate) struct CreateList {
    api_base: String,
}

impl NexusTool for CreateList {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/lists",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.create-list@1")
    }

    fn path() -> &'static str {
        "/create-list"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Validate name length
        if request.name.len() < 1 || request.name.len() > 25 {
            return Output::Err {
                reason: "List name must be between 1 and 25 characters".to_string(),
            };
        }

        // Validate description length
        if request.description.len() > 100 {
            return Output::Err {
                reason: "List description must not exceed 100 characters".to_string(),
            };
        }

        // Generate OAuth authorization header using the auth helper
        let auth_header = request.auth.generate_auth_header(&self.api_base);

        // Initialize HTTP client
        let client = Client::new();

        let request_body = format!(
            r#"{{
                "name": "{}",
                "description": "{}",
                "private": {}
            }}"#,
            request.name, request.description, request.private
        );

        // Make the request
        let response = client
            .post(&self.api_base)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await;

        match response {
            Err(e) => {
                return Output::Err {
                    reason: format!("Failed to send request to Twitter API: {}", e),
                }
            }
            Ok(result) => {
                let text = match result.text().await {
                    Err(e) => {
                        return Output::Err {
                            reason: format!("Failed to read Twitter API response: {}", e),
                        }
                    }
                    Ok(text) => text,
                };

                let json: serde_json::Value = match serde_json::from_str(&text) {
                    Err(e) => {
                        return Output::Err {
                            reason: format!("Invalid JSON response: {}", e),
                        }
                    }
                    Ok(json) => json,
                };

                // Check for error response with code/message format
                if let Some(code) = json.get("code") {
                    let message = json
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");

                    return Output::Err {
                        reason: format!("Twitter API error: {} (Code: {})", message, code),
                    };
                }

                // Check for error response with detail/status/title format
                if let Some(detail) = json.get("detail") {
                    let status = json.get("status").and_then(|s| s.as_u64()).unwrap_or(0);
                    let title = json
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Unknown");

                    return Output::Err {
                        reason: format!(
                            "Twitter API error: {} (Status: {}, Title: {})",
                            detail.as_str().unwrap_or("Unknown error"),
                            status,
                            title
                        ),
                    };
                }

                // Check for errors array
                if let Some(errors) = json.get("errors") {
                    return Output::Err {
                        reason: format!("Twitter API returned errors: {}", errors),
                    };
                }

                // Check for success response format
                let data = match json.get("data") {
                    None => {
                        return Output::Err {
                            reason: format!(
                                "Unexpected response format from Twitter API: {}",
                                json
                            ),
                        }
                    }
                    Some(data) => data,
                };

                // Parse the list data
                match serde_json::from_value::<ListResponse>(data.clone()) {
                    Ok(list_data) => Output::Ok {
                        id: list_data.id,
                        name: list_data.name,
                    },
                    Err(e) => Output::Err {
                        reason: format!("Failed to parse list data: {}", e),
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl CreateList {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    fn create_test_input() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            name: "Test List".to_string(),
            description: "Test Description".to_string(),
            private: false,
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, CreateList) {
        let server = Server::new_async().await;
        let tool = CreateList::with_api_base(&(server.url() + "/lists"));
        (server, tool)
    }

    #[tokio::test]
    async fn test_create_list_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "id": "1234567890",
                        "name": "Test List"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { id, name } => {
                assert_eq!(id, "1234567890");
                assert_eq!(name, "Test List");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_list_rate_limit_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists")
            .match_header("content-type", "application/json")
            .with_status(429)
            .with_body(
                json!({
                    "title": "Too Many Requests",
                    "detail": "Too Many Requests",
                    "type": "about:blank",
                    "status": 429
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Twitter API error: Too Many Requests")
                        && reason.contains("Status: 429")
                        && reason.contains("Title: Too Many Requests"),
                    "Expected rate limit error message with status and title, got: {}",
                    reason
                );
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_list_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists")
            .match_header("content-type", "application/json")
            .with_status(401)
            .with_body(
                json!({
                    "errors": [{
                        "message": "Unauthorized",
                        "code": 32
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Twitter API returned errors")
                        && reason.contains("Unauthorized"),
                    "Expected unauthorized error message, got: {}",
                    reason
                );
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_list_invalid_json() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body("invalid json")
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid JSON response"),
                    "Expected invalid JSON error message, got: {}",
                    reason
                );
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_list_missing_data() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(
                json!({
                    "meta": {
                        "status": "ok"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Unexpected response format"),
                    "Expected missing data error message, got: {}",
                    reason
                );
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_list_name_too_short() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            name: "".to_string(), // Empty name
            description: "Test Description".to_string(),
            private: false,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("List name must be between 1 and 25 characters"),
                    "Expected name length error message, got: {}",
                    reason
                );
            }
        }
    }

    #[tokio::test]
    async fn test_create_list_name_too_long() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            name: "This is a very long list name that exceeds 25 characters".to_string(), /* Name > 25 chars */
            description: "Test Description".to_string(),
            private: false,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("List name must be between 1 and 25 characters"),
                    "Expected name length error message, got: {}",
                    reason
                );
            }
        }
    }

    #[tokio::test]
    async fn test_create_list_description_too_long() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            name: "Test List".to_string(),
            description: "This is a very long description that exceeds 100 characters. This is a very long description that exceeds 100 characters.".to_string(), // Description > 100 chars
            private: false,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("List description must not exceed 100 characters"),
                    "Expected description length error message, got: {}",
                    reason
                );
            }
        }
    }
}
