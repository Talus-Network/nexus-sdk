//! # `xyz.taluslabs.social.twitter.delete-list@1`
//!
//! Standard Nexus Tool that deletes a list on Twitter.

use {
    crate::{
        auth::TwitterAuth,
        error::TwitterErrorKind,
        list::models::DeleteListResponse,
        twitter_client::{TwitterClient, TWITTER_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
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
        /// Detailed error message
        reason: String,
        /// Type of error (network, server, auth, etc.)
        kind: TwitterErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
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
            api_base: TWITTER_API_BASE.to_string(),
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
        // Build the endpoint for the Twitter API
        let suffix = format!("lists/{}", request.list_id);

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

        match client.delete::<DeleteListResponse>(&request.auth).await {
            Ok(data) => Output::Ok {
                deleted: data.deleted,
            },
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

    impl DeleteList {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, DeleteList) {
        let server = Server::new_async().await;
        let tool = DeleteList::with_api_base(&server.url());
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} kind: {:?} status_code: {:?}",
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("Response parsing error"),
                    "Error should indicate invalid JSON. Got: {} kind: {:?} status_code: {:?}",
                    reason,
                    kind,
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("Response parsing error"),
                    "Error should indicate unexpected format. Got: {} kind: {:?} status_code: {:?}",
                    reason,
                    kind,
                    status_code
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} kind: {:?} status_code: {:?}",
                reason, kind, status_code
            ),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
