//! # `xyz.taluslabs.social.twitter.get-conversation-messages@1`
//!
//! Standard Nexus Tool that retrieves direct messages from a conversation.

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
        error::{parse_twitter_response, TwitterResult},
        tweet::TWITTER_API_BASE,
    },
    reqwest::Client,
    ::{
        nexus_sdk::{fqn, ToolFqn},
        nexus_toolkit::*,
        schemars::JsonSchema,
        serde::{Deserialize, Serialize},
    },
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
        /// Error message if the request failed
        reason: String,
    },
}

pub(crate) struct GetConversationMessages {
    api_base: String,
}

impl NexusTool for GetConversationMessages {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/dm_conversations/with",
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
        // Validate range for max_results if provided
        if let Some(max_results) = request.max_results {
            if max_results < 1 || max_results > 100 {
                return Output::Err {
                    reason: "max_results must be between 1 and 100".to_string(),
                };
            }
        }

        // Validate pagination_token length if provided
        if let Some(ref token) = request.pagination_token {
            if token.len() < 16 {
                return Output::Err {
                    reason: "pagination_token must be at least 16 characters".to_string(),
                };
            }
        }

        match self.fetch_conversation_messages(&request).await {
            Ok(response) => Output::Ok {
                data: response.data,
                includes: response.includes,
                meta: response.meta,
            },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

impl GetConversationMessages {
    /// Fetch DM conversation messages from Twitter API
    async fn fetch_conversation_messages(
        &self,
        request: &Input,
    ) -> TwitterResult<DmEventsResponse> {
        let client = Client::new();

        // Construct base URL with participant_id (without query parameters)
        let base_url = format!("{}/{}/dm_events", self.api_base, request.participant_id);

        // Prepare query parameters
        let mut query_params = Vec::new();

        if let Some(max_results) = request.max_results {
            query_params.push(format!("max_results={}", max_results));
        }

        if let Some(pagination_token) = &request.pagination_token {
            query_params.push(format!("pagination_token={}", pagination_token));
        }

        if let Some(event_types) = &request.event_types {
            query_params.push(format!("event_types={}", event_types.join(",")));
        }

        if let Some(dm_event_fields) = &request.dm_event_fields {
            let fields: Vec<String> = dm_event_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(format!("dm_event.fields={}", fields.join(",")));
        }

        if let Some(expansions) = &request.expansions {
            let fields: Vec<String> = expansions
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(format!("expansions={}", fields.join(",")));
        }

        if let Some(media_fields) = &request.media_fields {
            let fields: Vec<String> = media_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(format!("media.fields={}", fields.join(",")));
        }

        if let Some(user_fields) = &request.user_fields {
            let fields: Vec<String> = user_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(format!("user.fields={}", fields.join(",")));
        }

        if let Some(tweet_fields) = &request.tweet_fields {
            let fields: Vec<String> = tweet_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(format!("tweet.fields={}", fields.join(",")));
        }

        // Generate the OAuth1 authorization header using only the base URL (no query params)
        let auth_header = request.auth.generate_auth_header(&base_url);

        // Now construct the full URL with query parameters
        let full_url = if !query_params.is_empty() {
            format!("{}?{}", base_url, query_params.join("&"))
        } else {
            base_url
        };

        // Build the request with OAuth authorization
        let req_builder = client.get(&full_url).header("Authorization", auth_header);

        // Send the request and parse the response
        let response = req_builder.send().await?;
        parse_twitter_response::<DmEventsResponse>(response).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetConversationMessages {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetConversationMessages) {
        let server = Server::new_async().await;
        let tool =
            GetConversationMessages::with_api_base(&(server.url() + "/dm_conversations/with"));
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
            event_types: Some(vec!["MessageCreate".to_string()]),
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

        let mock = server
            .mock("GET", "/dm_conversations/with/2244994945/dm_events")
            .match_header("Authorization", mockito::Matcher::Any) // OAuth header will be different each time
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
                let data = data.unwrap();
                assert_eq!(data.len(), 2);
                assert_eq!(data[0].id, "123456789");
                assert_eq!(data[0].text.as_ref().unwrap(), "Hello there!");
                assert_eq!(data[1].id, "987654321");

                // Check includes
                let includes = includes.unwrap();
                let users = includes.users.unwrap();
                assert_eq!(users.len(), 2);
                assert_eq!(users[0].id, "111222333");
                assert_eq!(users[0].username.as_ref().unwrap(), "testuser");
                assert_eq!(users[1].id, "2244994945");
                assert_eq!(users[1].username.as_ref().unwrap(), "otheruser");

                // Check meta data
                let meta = meta.unwrap();
                assert_eq!(meta.result_count.unwrap(), 2);
                assert_eq!(meta.next_token.unwrap(), "next_page_token_123");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_conversation_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/dm_conversations/with/2244994945/dm_events")
            .match_header("Authorization", mockito::Matcher::Any)
            .match_query(mockito::Matcher::Any)
            .with_status(404)
            .with_body(
                json!({
                    "errors": [{
                        "message": "Conversation not found",
                        "code": 50
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
                assert!(
                    reason.contains("Conversation not found"),
                    "Expected conversation not found error"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_auth() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/dm_conversations/with/2244994945/dm_events")
            .match_header("Authorization", mockito::Matcher::Any)
            .match_query(mockito::Matcher::Any)
            .with_status(401)
            .with_body(
                json!({
                    "errors": [{
                        "message": "Invalid or expired token",
                        "code": 89
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid or expired token"),
                    "Expected invalid token error"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_handling() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/dm_conversations/with/2244994945/dm_events")
            .match_header("Authorization", mockito::Matcher::Any)
            .match_query(mockito::Matcher::Any)
            .with_status(429)
            .with_body(
                json!({
                    "errors": [{
                        "message": "Rate limit exceeded",
                        "code": 88
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected rate limit error"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_conversation() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/dm_conversations/with/2244994945/dm_events")
            .match_header("Authorization", mockito::Matcher::Any)
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(
                json!({
                    "meta": {
                        "result_count": 0
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
                assert!(data.is_none() || data.unwrap().is_empty());
                assert!(meta.is_some());
                let meta = meta.unwrap();
                assert_eq!(meta.result_count.unwrap(), 0);
                assert!(includes.is_none());
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_max_results() {
        let (_, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.max_results = Some(150); // Over the 100 limit

        let output = tool.invoke(input).await;

        match output {
            Output::Err { reason } => {
                assert!(
                    reason.contains("max_results must be between 1 and 100"),
                    "Expected max_results validation error"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }
    }
}
