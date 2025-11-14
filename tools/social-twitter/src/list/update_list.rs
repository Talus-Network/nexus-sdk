// https://docs.x.com/x-api/lists/update-list
//! # `xyz.taluslabs.social.twitter.update-list@1`
//!
//! Standard Nexus Tool that updates a list metadata on Twitter.

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
    /// The ID of the list to update
    id: String,
    /// The name of the list to update
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// The description of the list to update
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// The privacy setting of the list to update
    /// - public: The list is public and can be viewed by anyone
    /// - private: The list is private and can only be viewed by the user who created it
    #[serde(skip_serializing_if = "Option::is_none")]
    private: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The successfully updated list data
        #[schemars(description = "Successfully updated list data")]
        updated: bool,
    },
    Err {
        /// Error message if the list update failed
        reason: String,
    },
}

pub(crate) struct UpdateList {
    api_base: String,
}

impl NexusTool for UpdateList {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/lists",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.update-list@1")
    }

    fn path() -> &'static str {
        "/update-list"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Check if at least one optional parameter is provided
        if request.name.is_none() && request.description.is_none() && request.private.is_none() {
            return Output::Err {
                reason: "At least one of name, description, or private must be provided"
                    .to_string(),
            };
        }

        // Validate name length (1-25 characters)
        if let Some(name) = &request.name {
            if name.is_empty() || name.len() > 25 {
                return Output::Err {
                    reason: "List name must be between 1 and 25 characters".to_string(),
                };
            }
        }

        // Validate description length (maximum 100 characters)
        if let Some(description) = &request.description {
            if description.len() > 100 {
                return Output::Err {
                    reason: "List description must not exceed 100 characters".to_string(),
                };
            }
        }

        // Generate OAuth authorization header using the auth helper
        let url = format!("{}/{}", self.api_base, request.id);
        let auth_header = request.auth.generate_auth_header_for_put(&url);

        // Initialize HTTP client
        let client = Client::new();

        // Build request body with only the fields that are provided
        let mut body = serde_json::Map::new();

        if let Some(name) = request.name {
            body.insert("name".to_string(), serde_json::Value::String(name));
        }

        if let Some(description) = request.description {
            body.insert(
                "description".to_string(),
                serde_json::Value::String(description),
            );
        }

        if let Some(private) = request.private {
            body.insert("private".to_string(), serde_json::Value::Bool(private));
        }

        let request_body = serde_json::Value::Object(body).to_string();

        let response = client
            .put(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await;

        match response {
            Err(e) => Output::Err {
                reason: format!("Failed to send request to Twitter API: {}", e),
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

                let updated = match data.get("updated") {
                    Some(updated) => updated.as_bool().unwrap_or(false),
                    None => false,
                };

                // For successful updates, Twitter API returns a data object with the updated list
                Output::Ok { updated }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl UpdateList {
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
            id: "123456".to_string(),
            name: Some("Updated Test List".to_string()),
            description: Some("Updated Test Description".to_string()),
            private: Some(true),
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, UpdateList) {
        let server = Server::new_async().await;
        let tool = UpdateList::with_api_base(&(server.url() + "/lists"));
        (server, tool)
    }

    #[tokio::test]
    async fn test_update_list_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("PUT", "/lists/123456")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "updated": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { updated } => assert!(updated),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_update_list_partial_fields() {
        let (mut server, tool) = create_server_and_tool().await;

        // Create a test input with only name updated
        let partial_input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            id: "123456".to_string(),
            name: Some("Only Name Updated".to_string()),
            description: None,
            private: None,
        };

        let mock = server
            .mock("PUT", "/lists/123456")
            .match_header("content-type", "application/json")
            .match_body(r#"{"name":"Only Name Updated"}"#)
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "updated": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(partial_input).await;

        match output {
            Output::Ok { updated } => assert!(updated),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_update_list_no_fields() {
        let (_, tool) = create_server_and_tool().await;

        // Create a test input with no updates
        let empty_input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            id: "123456".to_string(),
            name: None,
            description: None,
            private: None,
        };

        let output = tool.invoke(empty_input).await;

        match output {
            Output::Ok { .. } => panic!("Expected error for no fields to update"),
            Output::Err { reason } => {
                assert!(
                    reason
                        .contains("At least one of name, description, or private must be provided"),
                    "Expected error message about missing parameters, got: {}",
                    reason
                );
            }
        }
    }

    #[tokio::test]
    async fn test_update_list_name_too_long() {
        let (_, tool) = create_server_and_tool().await;

        // Create a test input with a name that exceeds 25 characters
        let long_name_input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            id: "123456".to_string(),
            name: Some("This name is definitely longer than twenty five characters".to_string()),
            description: None,
            private: None,
        };

        let output = tool.invoke(long_name_input).await;

        match output {
            Output::Ok { .. } => panic!("Expected error for name too long"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("List name must be between 1 and 25 characters"),
                    "Expected error message about name length, got: {}",
                    reason
                );
            }
        }
    }

    #[tokio::test]
    async fn test_update_list_empty_name() {
        let (_, tool) = create_server_and_tool().await;

        // Create a test input with an empty name
        let empty_name_input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            id: "123456".to_string(),
            name: Some("".to_string()),
            description: None,
            private: None,
        };

        let output = tool.invoke(empty_name_input).await;

        match output {
            Output::Ok { .. } => panic!("Expected error for empty name"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("List name must be between 1 and 25 characters"),
                    "Expected error message about name length, got: {}",
                    reason
                );
            }
        }
    }

    #[tokio::test]
    async fn test_update_list_description_too_long() {
        let (_, tool) = create_server_and_tool().await;

        // Create a test input with a description that exceeds 100 characters
        let long_description = "This description is definitely longer than one hundred characters. It goes on and on and on and on and on and on and on and on and on.".to_string();

        let long_description_input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            id: "123456".to_string(),
            name: None,
            description: Some(long_description),
            private: None,
        };

        let output = tool.invoke(long_description_input).await;

        match output {
            Output::Ok { .. } => panic!("Expected error for description too long"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("List description must not exceed 100 characters"),
                    "Expected error message about description length, got: {}",
                    reason
                );
            }
        }
    }

    #[tokio::test]
    async fn test_update_list_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("PUT", "/lists/123456")
            .match_header("content-type", "application/json")
            .with_status(404)
            .with_body(
                json!({
                    "detail": "Could not find list with ID: 123456",
                    "status": 404,
                    "title": "Not Found",
                    "type": "about:blank"
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
                    reason.contains("Could not find list with ID: 123456")
                        && reason.contains("Status: 404"),
                    "Error should indicate list not found. Got: {}",
                    reason
                );
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_update_list_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("PUT", "/lists/123456")
            .match_header("content-type", "application/json")
            .with_status(401)
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

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Unauthorized") && reason.contains("Status: 401"),
                    "Error should indicate unauthorized access. Got: {}",
                    reason
                );
            }
        }

        mock.assert_async().await;
    }
}
