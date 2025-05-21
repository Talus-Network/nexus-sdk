//! # `xyz.taluslabs.social.twitter.follow-user@1`
//!
//! Standard Nexus Tool that follows a user.

use {
    super::models::FollowUserResponse,
    crate::{
        auth::TwitterAuth,
        error::TwitterErrorKind,
        twitter_client::{TwitterClient, TWITTER_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::json,
};

#[derive(Deserialize, JsonSchema)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,
    /// The id of authenticated user
    user_id: String,
    /// Target user id to follow
    target_user_id: String,
}

#[derive(Serialize, JsonSchema)]
pub(crate) enum Output {
    Followed {
        /// Whether the user was followed
        result: bool,
    },
    Pending {
        /// Whether the follow request is pending
        result: bool,
    },
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (network, server, auth, etc.)
        kind: TwitterErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct FollowUser {
    api_base: String,
}

impl NexusTool for FollowUser {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.follow-user@1")
    }

    fn path() -> &'static str {
        "/follow-user"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Build the endpoint for the Twitter API
        let suffix = format!("users/{}/following", request.user_id);

        // Create a Twitter client with the mock server URL
        let client = match TwitterClient::new(Some(&suffix), Some(&self.api_base)) {
            Ok(client) => client,
            Err(e) => {
                return Output::Err {
                    reason: e.to_string(),
                    kind: TwitterErrorKind::Network,
                    status_code: None,
                }
            }
        };

        match client
            .post::<FollowUserResponse, _>(
                &request.auth,
                Some(json!({ "target_user_id": request.target_user_id })),
                None,
            )
            .await
        {
            Ok(data) => {
                if data.pending_follow {
                    Output::Pending { result: true }
                } else if data.following {
                    Output::Followed { result: true }
                } else {
                    Output::Followed { result: false }
                }
            }
            Err(e) => Output::Err {
                reason: e.reason,
                kind: e.kind,
                status_code: e.status_code,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ::{mockito::Server, serde_json::json},
    };

    impl FollowUser {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, FollowUser) {
        let server = Server::new_async().await;
        let tool = FollowUser::with_api_base(&server.url());
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
            user_id: "12345".to_string(),
            target_user_id: "67890".to_string(),
        }
    }

    #[tokio::test]
    async fn test_successful_follow() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for successful follow
        let mock = server
            .mock("POST", "/users/12345/following")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "following": true,
                        "pending_follow": false
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the follow request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Followed { result: followed } => {
                assert_eq!(followed, true);
            }
            Output::Pending { result: pending } => {
                panic!("Expected followed, got pending: {}", pending);
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} (kind: {:?}, status_code: {:?})",
                reason, kind, status_code
            ),
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
            .mock("POST", "/users/12345/following")
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

        // Test the follow request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Followed { .. } => panic!("Expected error, got followed success"),
            Output::Pending { .. } => panic!("Expected error, got pending success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::Auth,
                    "Expected error kind Auth, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Unauthorized"),
                    "Expected error message to contain 'Unauthorized', got: {}",
                    reason
                );

                // Check status code
                assert_eq!(
                    status_code,
                    Some(401),
                    "Expected status code 401, got: {:?}",
                    status_code
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
            .mock("POST", "/users/12345/following")
            .with_status(400)
            .with_body("invalid json")
            .create_async()
            .await;

        // Test the follow request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Followed { .. } => panic!("Expected error, got followed success"),
            Output::Pending { .. } => panic!("Expected error, got pending success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::Unknown,
                    "Expected error kind Unknown, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Twitter API status error: 400 Bad Request"),
                    "Expected error message to contain 'Twitter API status error: 400 Bad Request', got: {}",
                    reason
                );

                // Check status code
                assert_eq!(
                    status_code,
                    Some(400),
                    "Expected status code 400, got: {:?}",
                    status_code
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
            .mock("POST", "/users/12345/following")
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

        // Test the follow request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Followed { .. } => panic!("Expected error, got followed success"),
            Output::Pending { .. } => panic!("Expected error, got pending success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::Parse,
                    "Expected error kind Parse, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Response parsing error: missing field `following`"),
                    "Expected error message to contain 'Response parsing error: missing field `following`', got: {}",
                    reason
                );

                // Check status code
                assert_eq!(
                    status_code, None,
                    "Expected status code None, got: {:?}",
                    status_code
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pending_follow() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for pending follow response
        let mock = server
            .mock("POST", "/users/12345/following")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "following": false,
                        "pending_follow": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the follow request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Pending { result } => {
                assert_eq!(result, true);
            }
            Output::Followed { result } => {
                panic!("Expected pending, got followed: {}", result);
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} (kind: {:?}, status_code: {:?})",
                reason, kind, status_code
            ),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
