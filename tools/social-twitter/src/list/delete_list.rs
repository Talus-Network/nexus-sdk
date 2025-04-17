//! # `xyz.taluslabs.social.twitter.delete-list@1`
//!
//! Standard Nexus Tool that deletes a list on Twitter.

use {
    crate::{auth::TwitterAuth, tweet::TWITTER_API_BASE},
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,
    /// The ID of the list to delete
    list_id: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Whether the list was deleted
        deleted: bool,
    },
    Err {
        /// Error message
        reason: String,
    },
}

pub(crate) struct DeleteList {
    api_base: String,
}

impl NexusTool for DeleteList {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/lists",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.delete-list@1")
    }

    fn path() -> &'static str {
        "/delete-list"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        let client = Client::new();

        let url = format!("{}/{}", self.api_base, request.list_id);

        // Generate OAuth authorization header with the complete URL
        let auth_header = request.auth.generate_auth_header_for_delete(&url);

        let response = client
            .delete(&url)
            .header("Authorization", auth_header)
            .send()
            .await;

        match response {
            Err(e) => Output::Err {
                reason: format!("Failed to send delete request to Twitter API: {}", e),
            },
            Ok(result) => {
                let text = match result.text().await {
                    Err(e) => {
                        return Output::Err {
                            reason: format!("Failed to read Twitter API response: {}", e),
                        }
                    }
                    Ok(text) => text,
                };

                let json: Value = match serde_json::from_str(&text) {
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

                let deleted = match data.get("deleted") {
                    None => {
                        return Output::Err {
                            reason: format!(
                                "Unexpected response format from Twitter API: {}",
                                json
                            ),
                        }
                    }
                    Some(deleted) => deleted.as_bool().unwrap_or(false),
                };

                Output::Ok { deleted }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ::{mockito::Server, serde_json::json},
    };

    impl DeleteList {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, DeleteList) {
        let server = Server::new_async().await;
        let tool = DeleteList::with_api_base(&(server.url() + "/lists"));
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            list_id: "12345".to_string(),
        }
    }

    #[tokio::test]
    async fn get_fqn() {
        let fqn = DeleteList::fqn();
        println!("fqn: {:?}", fqn);
        assert_eq!(fqn.domain(), "xyz.taluslabs.social.twitter");
        assert_eq!(fqn.name(), "delete-list");
        assert_eq!(fqn.version(), 1);
    }

    #[tokio::test]
    async fn get_path() {
        let path = DeleteList::path();
        assert_eq!(path, "/delete-list");
    }

    #[tokio::test]
    async fn health() {
        let tool = DeleteList::new().await;
        let status = tool.health().await;
        assert!(status.is_ok());
    }

    #[tokio::test]
    async fn test_successful_delete() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for successful delete
        let mock = server
            .mock("DELETE", "/lists/12345")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "deleted": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the delete request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok { deleted } => {
                assert_eq!(deleted, true);
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unauthorized_error() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for 401 Unauthorized response
        let mock = server
            .mock("DELETE", "/lists/12345")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "detail": "Unauthorized",
                    "status": 401,
                    "title": "Unauthorized",
                    "type": "about:blank"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the delete request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Unauthorized") && reason.contains("Status: 401"),
                    "Error should indicate unauthorized access. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_json_response() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for invalid JSON response
        let mock = server
            .mock("DELETE", "/lists/12345")
            .with_status(200)
            .with_body("invalid json")
            .create_async()
            .await;

        // Test the delete request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid JSON"),
                    "Error should indicate invalid JSON. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unexpected_format() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for unexpected response format
        let mock = server
            .mock("DELETE", "/lists/12345")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "some_other_field": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the delete request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Unexpected response format"),
                    "Error should indicate unexpected format. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_not_deleted() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for not deleted response
        let mock = server
            .mock("DELETE", "/lists/12345")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "deleted": false
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the delete request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok { deleted } => {
                assert_eq!(deleted, false);
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
