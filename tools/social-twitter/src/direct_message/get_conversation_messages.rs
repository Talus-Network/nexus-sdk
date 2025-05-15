//! # `xyz.taluslabs.social.twitter.get-conversation-messages@1`
//!
//! Standard Nexus Tool that retrieves direct messages from a conversation.

use {
    super::models::EventType,
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

    /// The ID of the participant user for the One to One DM conversation
    /// Example: "2244994945"
    participant_id: String,

    /// The maximum number of results to return (1-100, default: 100)
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<i32>,

    /// This parameter is used to get a specified 'page' of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination_token: Option<String>,

    /// The set of event_types to include in the results
    #[serde(skip_serializing_if = "Option::is_none")]
    event_types: Option<Vec<EventType>>,

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

impl GetConversationMessages {
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

pub(crate) struct GetConversationMessages {
    api_base: String,
}

impl NexusTool for GetConversationMessages {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-conversation-messages@1")
    }

    fn path() -> &'static str {
        "/get-conversation-messages"
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
        let suffix = format!("dm_conversations/with/{}/dm_events", request.participant_id);

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

        self.add_fields_param(&mut query_params, "event_types", &request.event_types);
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
    use {super::*, crate::direct_message::models::EventType, ::mockito::Server, serde_json::json};

    impl GetConversationMessages {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetConversationMessages) {
        let server = Server::new_async().await;
        let tool = GetConversationMessages::with_api_base(&server.url());
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
            participant_id: "2244994945".to_string(),
            max_results: Some(10),
            pagination_token: None,
            event_types: Some(vec![EventType::MessageCreate]),
            dm_event_fields: Some(vec![
                DmEventField::Id,
                DmEventField::Text,
                DmEventField::EventType,
            ]),
            expansions: Some(vec![ExpansionField::SenderId]),
            media_fields: None,
            user_fields: Some(vec![UserField::Id, UserField::Name, UserField::Username]),
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_conversation_messages_successful() {
        let (mut server, tool) = create_server_and_tool().await;
        let participant_id = "2244994945";
        let expected_path = format!("/dm_conversations/with/{}/dm_events", participant_id);

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
                        }
                    ],
                    "includes": {
                        "users": [
                            {
                                "id": "111222333",
                                "name": "Test User",
                                "username": "testuser"
                            }
                        ]
                    },
                    "meta": {
                        "result_count": 1,
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
                assert_eq!(data_vec.len(), 1);
                assert_eq!(data_vec[0].id, "123456789");
                assert!(includes.is_some());
                assert!(meta.is_some());
                assert_eq!(meta.unwrap().result_count, Some(1));
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_conversation_not_found_error() {
        let (mut server, tool) = create_server_and_tool().await;
        let participant_id = "nonexistentuser";
        let expected_path = format!("/dm_conversations/with/{}/dm_events", participant_id);

        let mock = server
            .mock("GET", expected_path.as_str())
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(json!({ "data": [], "meta": { "result_count": 0 } }).to_string())
            .create_async()
            .await;

        let mut input = create_test_input();
        input.participant_id = participant_id.to_string();

        let output = tool.invoke(input).await;

        match output {
            Output::Err { kind, reason, .. } => {
                assert_eq!(kind, TwitterErrorKind::NotFound);
                assert!(reason.contains("No conversation data found"));
            }
            Output::Ok { .. } => panic!("Expected NotFound error, got success"),
        }
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_input_validation_max_results() {
        let (_server, tool) = create_server_and_tool().await;
        let mut input = create_test_input();
        input.max_results = Some(200);

        let output = tool.invoke(input).await;

        match output {
            Output::Err { kind, reason, .. } => {
                assert_eq!(kind, TwitterErrorKind::Unknown);
                assert!(reason.contains("max_results must be between 1 and 100"));
            }
            Output::Ok { .. } => panic!("Expected validation error for max_results"),
        }
    }
}
