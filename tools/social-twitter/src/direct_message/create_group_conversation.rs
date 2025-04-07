//! # `xyz.taluslabs.social.twitter.create-group-conversation@1`
//!
//! Standard Nexus Tool that creates a group DM conversation.

use {
    super::models::{ConversationType, Message},
    crate::{auth::TwitterAuth, tweet::TWITTER_API_BASE},
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

#[derive(Deserialize, JsonSchema)]
pub(crate) struct Input {
    /// The access token received from the authorization server in the OAuth 2.0 flow.
    #[serde(flatten)]
    auth: TwitterAuth,
    /// The conversation type that is being created.
    conversation_type: ConversationType,
    /// The message to be sent to the conversation.
    message: Message,
    /// Participants for the DM Conversation.
    /// Unique identifier of this User.
    /// This is returned as a string in order to avoid complications with languages and tools that cannot handle large integers.
    participant_ids: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) enum Output {
    Ok {
        /// Unique identifier of a DM conversation.
        /// This can either be a numeric string, or a pair of numeric strings separated by a '-' character in the case of one-on-one DM Conversations.
        dm_conversation_id: String,
        /// Unique identifier of a DM Event.
        dm_event_id: String,
    },
    Err {
        /// The error message.
        reason: String,
    },
}

pub(crate) struct CreateGroupDmConversation {
    api_base: String,
}

impl NexusTool for CreateGroupDmConversation {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/dm_conversations",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.create-group-conversation@1")
    }

    fn path() -> &'static str {
        "/create-group-conversation"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        let client = Client::new();

        let url = format!("{}", self.api_base);

        // Generate OAuth authorization header with the complete URL
        let auth_header = request.auth.generate_auth_header(&url);

        // Construct request body based on input
        let mut request_body = serde_json::json!({});

        if request.conversation_type == ConversationType::Group {
            request_body["conversation_type"] = serde_json::Value::String("group".to_string());
        }

        request_body["participant_ids"] = serde_json::Value::Array(
            request
                .participant_ids
                .into_iter()
                .map(|id| serde_json::Value::String(id))
                .collect(),
        );

        if let Some(text) = request.message.text {
            request_body["text"] = serde_json::Value::String(text);
        }

        if let Some(attachments) = request.message.attachments {
            let media_attachments: Vec<serde_json::Value> = attachments
                .into_iter()
                .map(|a| serde_json::json!({ "media_id": a.media_id }))
                .collect();
            request_body["attachments"] = serde_json::Value::Array(media_attachments);
        }

        let response = client
            .post(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;

        match response {
            Err(e) => Output::Err {
                reason: format!("Failed to send direct message: {}", e),
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

                let dm_conversation_id = match data.get("dm_conversation_id") {
                    None => {
                        return Output::Err {
                            reason: format!(
                                "Unexpected response format from Twitter API: {}",
                                json
                            ),
                        }
                    }
                    Some(dm_conversation_id) => dm_conversation_id.as_str().unwrap_or("Unknown"),
                };

                let dm_event_id = match data.get("dm_event_id") {
                    None => {
                        return Output::Err {
                            reason: format!(
                                "Unexpected response format from Twitter API: {}",
                                json
                            ),
                        }
                    }
                    Some(dm_event_id) => dm_event_id.as_str().unwrap_or("Unknown"),
                };

                Output::Ok {
                    dm_conversation_id: dm_conversation_id.to_string(),
                    dm_event_id: dm_event_id.to_string(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::direct_message::models::Attachment,
        ::mockito::Server,
        serde_json::json,
    };

    impl CreateGroupDmConversation {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, CreateGroupDmConversation) {
        let server = Server::new_async().await;
        let tool = CreateGroupDmConversation::with_api_base(&server.url());
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
            conversation_type: ConversationType::Group,
            message: Message {
                text: Some("Test group message".to_string()),
                attachments: Some(vec![
                    Attachment {
                        media_id: "12345".to_string(),
                    },
                    Attachment {
                        media_id: "67890".to_string(),
                    },
                ]),
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        }
    }

    #[tokio::test]
    async fn test_create_group_conversation_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "dm_conversation_id": "123123123-456456456",
                        "dm_event_id": "1146654567674912769"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok {
                dm_conversation_id,
                dm_event_id,
            } => {
                assert_eq!(dm_conversation_id, "123123123-456456456");
                assert_eq!(dm_event_id, "1146654567674912769");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_group_conversation_with_attachments() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            conversation_type: ConversationType::Group,
            message: Message {
                text: Some("Test group message with attachment".to_string()),
                attachments: Some(vec![
                    Attachment {
                        media_id: "1146654567674912769".to_string(),
                    },
                    Attachment {
                        media_id: "1146654567674912770".to_string(),
                    },
                ]),
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        };

        let mock = server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "dm_conversation_id": "123123123-456456456",
                        "dm_event_id": "1146654567674912769"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok {
                dm_conversation_id,
                dm_event_id,
            } => {
                assert_eq!(dm_conversation_id, "123123123-456456456");
                assert_eq!(dm_event_id, "1146654567674912769");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_group_conversation_invalid_json() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body("invalid json")
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid JSON response"),
                    "Expected error message to contain 'Invalid JSON response', got: {}",
                    reason
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }
}
