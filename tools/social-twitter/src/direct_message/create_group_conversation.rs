//! # `xyz.taluslabs.social.twitter.create-group-conversation@1`
//!
//! Standard Nexus Tool that creates a group DM conversation.

use {
    crate::{
        auth::TwitterAuth,
        direct_message::models::{ConversationType, DmConversationResponse, Message},
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
        /// Detailed error message
        reason: String,
        /// Type of error (network, server, auth, etc.)
        kind: TwitterErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
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
            api_base: TWITTER_API_BASE.to_string(),
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
        // Validate the message
        if let Err(e) = request.message.validate() {
            return Output::Err {
                reason: e,
                kind: TwitterErrorKind::Validation,
                status_code: None,
            };
        }

        // Create a Twitter client with the mock server URL
        let client = match TwitterClient::new(Some("dm_conversations"), Some(&self.api_base)) {
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
            .post::<DmConversationResponse, _>(
                &request.auth,
                Some(json!({
                    "conversation_type": request.conversation_type,
                    "message": request.message,
                    "participant_ids": request.participant_ids,
                })),
                None,
            )
            .await
        {
            Ok(data) => Output::Ok {
                dm_conversation_id: data.dm_conversation_id,
                dm_event_id: data.dm_event_id,
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
        crate::direct_message::models::MediaIdsBag,
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
                media_ids: Some(MediaIdsBag::Many(vec![
                    "12345".to_string(),
                    "67890".to_string(),
                ])),
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        }
    }

    fn create_test_input_text_only() -> Input {
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
                media_ids: None,
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        }
    }

    fn create_test_input_attachments_only() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            conversation_type: ConversationType::Group,
            message: Message {
                text: None,
                media_ids: Some(MediaIdsBag::Many(vec![
                    "12345".to_string(),
                    "67890".to_string(),
                ])),
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        }
    }

    fn create_test_input_empty_text() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            conversation_type: ConversationType::Group,
            message: Message {
                text: Some("".to_string()),
                media_ids: None,
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        }
    }

    fn create_test_input_empty_attachments() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            conversation_type: ConversationType::Group,
            message: Message {
                text: None,
                media_ids: Some(MediaIdsBag::Many(vec![])),
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        }
    }

    fn create_test_input_no_content() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            conversation_type: ConversationType::Group,
            message: Message {
                text: None,
                media_ids: None,
            },
            participant_ids: vec!["12345".to_string(), "67890".to_string()],
        }
    }

    #[tokio::test]
    async fn test_create_group_conversation_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/dm_conversations")
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} (kind: {:?}, status_code: {:?})",
                reason, kind, status_code
            ),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_group_conversation_text_only() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/dm_conversations")
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

        let output = tool.invoke(create_test_input_text_only()).await;

        match output {
            Output::Ok {
                dm_conversation_id,
                dm_event_id,
            } => {
                assert_eq!(dm_conversation_id, "123123123-456456456");
                assert_eq!(dm_event_id, "1146654567674912769");
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

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_group_conversation_attachments_only() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/dm_conversations")
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

        let output = tool.invoke(create_test_input_attachments_only()).await;

        match output {
            Output::Ok {
                dm_conversation_id,
                dm_event_id,
            } => {
                assert_eq!(dm_conversation_id, "123123123-456456456");
                assert_eq!(dm_event_id, "1146654567674912769");
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

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_create_group_conversation_empty_text() {
        let tool = CreateGroupDmConversation::new().await;
        let output = tool.invoke(create_test_input_empty_text()).await;

        match output {
            Output::Ok { .. } => panic!("Expected validation error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(kind, TwitterErrorKind::Validation);
                assert!(reason.contains("Either text or media_ids must be provided and non-empty"));
                assert_eq!(status_code, None);
            }
        }
    }

    #[tokio::test]
    async fn test_create_group_conversation_empty_attachments() {
        let tool = CreateGroupDmConversation::new().await;
        let output = tool.invoke(create_test_input_empty_attachments()).await;

        match output {
            Output::Ok { .. } => panic!("Expected validation error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(kind, TwitterErrorKind::Validation);
                assert!(reason.contains("Either text or media_ids must be provided and non-empty"));
                assert_eq!(status_code, None);
            }
        }
    }

    #[tokio::test]
    async fn test_create_group_conversation_no_content() {
        let tool = CreateGroupDmConversation::new().await;
        let output = tool.invoke(create_test_input_no_content()).await;

        match output {
            Output::Ok { .. } => panic!("Expected validation error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(kind, TwitterErrorKind::Validation);
                assert!(reason.contains("Either text or media_ids must be provided and non-empty"));
                assert_eq!(status_code, None);
            }
        }
    }

    #[tokio::test]
    async fn test_create_group_conversation_invalid_json() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/dm_conversations")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_body("invalid json")
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("Response parsing error"),
                    "Expected error message to contain 'Response parsing error', got: {} (kind: {:?}, status_code: {:?})",
                    reason, kind, status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }
}
