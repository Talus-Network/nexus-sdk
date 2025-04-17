//! # `xyz.taluslabs.social.twitter.remove-member@1`
//!
//! Standard Nexus Tool that removes a member from a list on Twitter.

use {
    super::models::ListMemberResponse,
    crate::{auth::TwitterAuth, tweet::TWITTER_API_BASE},
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json,
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,
    /// List ID to remove member from
    list_id: String,
    /// User ID to remove from list
    user_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Whether the user is a member of the list
        is_member: bool,
    },
    Err {
        /// Error message if the tweet failed
        reason: String,
    },
}

pub(crate) struct RemoveMember {
    api_base: String,
}

impl NexusTool for RemoveMember {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/lists",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.remove-member@1")
    }

    fn path() -> &'static str {
        "/remove-member"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Add authentication header
        let url = format!(
            "{}/{}/members/{}",
            self.api_base, request.list_id, request.user_id
        );

        // Generate OAuth authorization header using the auth helper
        let auth_header = request.auth.generate_auth_header_for_delete(&url);

        // Initialize HTTP client
        let client = Client::new();

        // Make the request
        let response = client
            .delete(&url)
            .header("Authorization", auth_header)
            .send()
            .await;

        match response {
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

                // Parse the list data
                match serde_json::from_value::<ListMemberResponse>(json) {
                    Ok(list_data) => Output::Ok {
                        is_member: list_data.data.unwrap().is_member,
                    },
                    Err(e) => Output::Err {
                        reason: format!("Failed to parse list data: {}", e),
                    },
                }
            }
            Err(e) => {
                return Output::Err {
                    reason: format!("Failed to send request to Twitter API: {}", e),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl RemoveMember {
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
            list_id: "test_list_id".to_string(),
            user_id: "test_user_id".to_string(),
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, RemoveMember) {
        let server = Server::new_async().await;
        let tool = RemoveMember::with_api_base(&(server.url() + "/lists"));
        (server, tool)
    }

    #[tokio::test]
    async fn test_remove_member_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("DELETE", "/lists/test_list_id/members/test_user_id")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "is_member": false
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { is_member } => assert!(!is_member),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_remove_member_rate_limit_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("DELETE", "/lists/test_list_id/members/test_user_id")
            .with_status(429)
            .with_body(
                json!({
                    "title": "Too Many Requests",
                    "detail": "Too Many Requests",
                    "status": 429
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => assert!(reason.contains("Too Many Requests")),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_remove_member_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("DELETE", "/lists/test_list_id/members/test_user_id")
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
            Output::Err { reason } => assert!(reason.contains("Unauthorized")),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_remove_member_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("DELETE", "/lists/test_list_id/members/test_user_id")
            .with_status(404)
            .with_body(
                json!({
                    "errors": [{
                        "message": "Not Found",
                        "code": 34
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => assert!(reason.contains("Not Found")),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_remove_member_invalid_response() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("DELETE", "/lists/test_list_id/members/test_user_id")
            .with_status(200)
            .with_body("invalid json")
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => assert!(reason.contains("Invalid JSON response")),
        }

        mock.assert_async().await;
    }
}
