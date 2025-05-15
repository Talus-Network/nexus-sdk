//! # `xyz.taluslabs.social.twitter.get-conversation-messages-by-id@1`
//!
//! Standard Nexus Tool that retrieves direct messages from a conversation by ID.

use {
    crate::{
        auth::TwitterAuth,
        direct_message::models::{
            DmEvent,
            DmEventField,
            DmEventsResponse,
            ExpansionField,
            Includes,
            MediaField,
            TweetField,
            UserField,
        },
        error::TwitterErrorKind,
        twitter_client::{TwitterClient, TWITTER_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json,
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,

    /// The ID of the DM Conversation
    /// Example: "123123123-456456456"
    conversation_id: String,

    /// The maximum number of results to return (1-100, default: 100)
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<i32>,

    /// This parameter is used to get a specified 'page' of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination_token: Option<String>,

    /// The set of event_types to include in the results
    #[serde(skip_serializing_if = "Option::is_none")]
    event_types: Option<Vec<String>>,

    /// A comma separated list of DM Event fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    dm_event_fields: Option<Vec<DmEventField>>,

    /// A comma separated list of fields to expand
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions: Option<Vec<ExpansionField>>,

    /// A comma separated list of Media fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    media_fields: Option<Vec<MediaField>>,

    /// A comma separated list of User fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,

    /// A comma separated list of Tweet fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    tweet_fields: Option<Vec<TweetField>>,
}

impl Input {
    /// Validate input parameters
    fn validate(&self) -> Result<(), String> {
        if let Some(max_results) = self.max_results {
            if !(1..=100).contains(&max_results) {
                return Err(format!(
                    "max_results must be between 1 and 100, got {}",
                    max_results
                ));
            }
        }

        if let Some(ref token) = self.pagination_token {
            if token.len() < 16 {
                return Err("pagination_token must be at least 16 characters".to_string());
            }
        }

        Ok(())
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The list of DM events in the conversation
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Vec<DmEvent>>,
        /// Additional information related to the events
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
        /// Pagination metadata
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<crate::tweet::models::Meta>,
    },
    Err {
        /// Type of error (network, server, auth, etc.)
        kind: TwitterErrorKind,
        /// Detailed error message
        reason: String,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

impl GetConversationMessagesById {
    fn add_fields_param<T: Serialize>(
        &self,
        params: &mut Vec<(String, String)>,
        param_name: &str,
        fields: &Option<Vec<T>>,
    ) {
        if let Some(field_values) = fields {
            if !field_values.is_empty() {
                let formatted_fields: Vec<String> = field_values
                    .iter()
                    .map(|f| {
                        serde_json::to_string(f)
                            .unwrap_or_default()
                            .replace("\"", "")
                            .to_lowercase()
                    })
                    .collect();

                params.push((param_name.to_string(), formatted_fields.join(",")));
            }
        }
    }

    fn add_param<T: ToString + Clone>(
        params: &mut Vec<(String, String)>,
        name: &str,
        value: &Option<T>,
    ) {
        if let Some(val) = value {
            params.push((name.to_string(), val.clone().to_string()));
        }
    }
}

pub(crate) struct GetConversationMessagesById {
    api_base: String,
}

impl NexusTool for GetConversationMessagesById {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-conversation-messages-by-id@1")
    }

    fn path() -> &'static str {
        "/get-conversation-messages-by-id"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Validate input parameters first
        if let Err(e) = request.validate() {
            return Output::Err {
                kind: TwitterErrorKind::Unknown,
                reason: format!("Input validation error: {}", e),
                status_code: None,
            };
        }

        // Construct the specific endpoint for this tool
        let suffix = format!("dm_conversations/{}/dm_events", request.conversation_id);

        let client = match TwitterClient::new(Some(&suffix), Some(&self.api_base)) {
            Ok(client) => client,
            Err(e) => {
                return Output::Err {
                    reason: e.to_string(),
                    kind: TwitterErrorKind::Network,
                    status_code: None,
                };
            }
        };

        let mut query_params: Vec<(String, String)> = Vec::new();

        Self::add_param(&mut query_params, "max_results", &request.max_results);
        Self::add_param(
            &mut query_params,
            "pagination_token",
            &request.pagination_token,
        );

        if let Some(event_types) = &request.event_types {
            query_params.push(("event_types".to_string(), event_types.join(",")));
        }

        self.add_fields_param(
            &mut query_params,
            "dm_event.fields",
            &request.dm_event_fields,
        );
        self.add_fields_param(&mut query_params, "expansions", &request.expansions);
        self.add_fields_param(&mut query_params, "media.fields", &request.media_fields);
        self.add_fields_param(&mut query_params, "user.fields", &request.user_fields);
        self.add_fields_param(&mut query_params, "tweet.fields", &request.tweet_fields);

        match client
            .get_with_auth::<DmEventsResponse>(&request.auth, Some(query_params))
            .await
        {
            Ok((data, includes, meta)) => {
                if data.is_empty() {
                    Output::Err {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No conversation data found".to_string(),
                        status_code: None,
                    }
                } else {
                    Output::Ok {
                        data: Some(data),
                        includes,
                        meta,
                    }
                }
            }
            Err(e) => Output::Err {
                kind: e.kind,
                reason: e.reason,
                status_code: e.status_code,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetConversationMessagesById {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetConversationMessagesById) {
        let server = Server::new_async().await;
        let tool = GetConversationMessagesById::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret_key",
                "test_access_token",
                "test_access_token_secret",
            ),
            conversation_id: "123123123-456456456".to_string(),
            max_results: Some(10),
            pagination_token: None,
            event_types: Some(vec!["MessageCreate".to_string()]),
            dm_event_fields: Some(vec![
                DmEventField::Id,
                DmEventField::Text,
                DmEventField::EventType,
            ]),
            expansions: Some(vec![ExpansionField::SenderId]),
            media_fields: None,
            user_fields: Some(vec![UserField::Username, UserField::Name]),
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_conversation_messages_successful() {
        let (mut server, tool) = create_server_and_tool().await;
        let conversation_id = "123123123-456456456";
        let expected_path = format!("/dm_conversations/{}/dm_events", conversation_id);

        let mock = server
            .mock("GET", expected_path.as_str())
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "123456789",
                            "event_type": "message_create",
                            "text": "Hello there!",
                            "sender_id": "111222333"
                        },
                        {
                            "id": "987654321",
                            "event_type": "message_create",
                            "text": "Hi, how are you?",
                            "sender_id": "2244994945"
                        }
                    ],
                    "includes": {
                        "users": [
                            {
                                "id": "111222333",
                                "name": "Test User",
                                "username": "testuser"
                            },
                            {
                                "id": "2244994945",
                                "name": "Other User",
                                "username": "otheruser"
                            }
                        ]
                    },
                    "meta": {
                        "result_count": 2,
                        "next_token": "next_page_token_123"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok {
                data,
                includes,
                meta,
            } => {
                assert!(data.is_some());
                let data_vec = data.unwrap();
                assert_eq!(data_vec.len(), 2);
                assert_eq!(data_vec[0].id, "123456789");
                assert_eq!(data_vec[0].text.as_ref().unwrap(), "Hello there!");
                assert_eq!(data_vec[1].id, "987654321");

                // Check includes
                let includes_unwrapped = includes.unwrap();
                let users = includes_unwrapped.users.unwrap();
                assert_eq!(users.len(), 2);
                assert_eq!(users[0].id, "111222333");
                assert_eq!(users[0].name.as_ref().unwrap(), "Test User");
                assert_eq!(users[0].username.as_ref().unwrap(), "testuser");
                assert_eq!(users[1].id, "2244994945");
                assert_eq!(users[1].name.as_ref().unwrap(), "Other User");
                assert_eq!(users[1].username.as_ref().unwrap(), "otheruser");

                // Check meta data
                let meta_unwrapped = meta.unwrap();
                assert_eq!(meta_unwrapped.result_count.unwrap(), 2);
                assert_eq!(meta_unwrapped.next_token.unwrap(), "next_page_token_123");
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_conversation_not_found() {
        let (mut server, tool) = create_server_and_tool().await;
        let conversation_id = "nonexistent-id";
        let expected_path = format!("/dm_conversations/{}/dm_events", conversation_id);

        let mock = server
            .mock("GET", expected_path.as_str())
            .match_query(mockito::Matcher::Any)
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "message": "Conversation not found",
                        "code": 34,
                        "title": "Not Found Error",
                        "type": "https://api.twitter.com/2/problems/resource-not-found",
                        "detail": "Could not find the specified conversation."
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut input = create_test_input();
        input.conversation_id = conversation_id.to_string();

        let output = tool.invoke(input).await;

        match output {
            Output::Err { kind, reason, .. } => {
                assert_eq!(kind, TwitterErrorKind::NotFound);
                assert!(
                    reason.contains("Not Found Error") || reason.contains("Conversation not found")
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_max_results() {
        let (_server, tool) = create_server_and_tool().await;
        let mut input = create_test_input();
        input.max_results = Some(150);

        let output = tool.invoke(input).await;

        match output {
            Output::Err { kind, reason, .. } => {
                assert_eq!(kind, TwitterErrorKind::Unknown);
                assert!(reason.contains("max_results must be between 1 and 100"));
            }
            Output::Ok { .. } => panic!("Expected validation error for max_results"),
        }
    }

    #[tokio::test]
    async fn test_invalid_pagination_token() {
        let (_server, tool) = create_server_and_tool().await;
        let mut input = create_test_input();
        input.pagination_token = Some("short".to_string());

        let output = tool.invoke(input).await;

        match output {
            Output::Err { kind, reason, .. } => {
                assert_eq!(kind, TwitterErrorKind::Unknown);
                assert!(reason.contains("pagination_token must be at least 16 characters"));
            }
            Output::Ok { .. } => panic!("Expected validation error for pagination_token"),
        }
    }
}
