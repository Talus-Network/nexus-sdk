//! # `xyz.taluslabs.social.twitter.add-member@1`
//!
//! Standard Nexus Tool that adds a member to a list on Twitter.

use {
    super::models::ListMemberResponse,
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
    /// List ID to add member to
    list_id: String,
    /// User ID to add to list
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
        /// Error message if the list member addition failed
        reason: String,
    },
}

pub(crate) struct AddMember {
    api_base: String,
}

impl NexusTool for AddMember {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/lists",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.add-member@1")
    }

    fn path() -> &'static str {
        "/add-member"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Add authentication header
        let url = format!("{}/{}/members", self.api_base, request.list_id);
        let auth_header = request.auth.generate_auth_header(&url);

        // Initialize HTTP client
        let client = Client::new();

        // Request body for adding a member to a list
        let request_body = format!(
            r#"{{
                "user_id": "{}"
            }}"#,
            request.user_id
        );

        // Make the request
        let response = client
            .post(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .body(request_body)
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

    impl AddMember {
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

    async fn create_server_and_tool() -> (mockito::ServerGuard, AddMember) {
        let server = Server::new_async().await;
        let tool = AddMember::with_api_base(&(server.url() + "/lists"));
        (server, tool)
    }

    #[tokio::test]
    async fn test_add_member_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists/test_list_id/members")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "is_member": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { is_member } => assert!(is_member),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_add_member_rate_limit_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists/test_list_id/members")
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
    async fn test_add_member_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists/test_list_id/members")
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
    async fn test_add_member_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists/test_list_id/members")
            .with_status(404)
            .with_body(
                json!({
                    "title": "Not Found",
                    "type": "about:blank",
                    "detail": "The specified list was not found",
                    "status": 404
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
    async fn test_add_member_invalid_json() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/lists/test_list_id/members")
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
                    "Expected error about invalid JSON, got: {}",
                    reason
                );
            }
        }

        mock.assert_async().await;
    }
}
