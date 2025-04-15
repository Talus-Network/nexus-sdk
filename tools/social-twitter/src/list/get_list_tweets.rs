//! # `xyz.taluslabs.social.twitter.get-list-tweets@1`
//!
//! Standard Nexus Tool that retrieves tweets from a list.

use {
    crate::{
        error::{parse_twitter_response, TwitterErrorKind, TwitterErrorResponse, TwitterResult},
        list::models::{Expansion, Includes, ListTweet, ListTweetsResponse, Meta},
        tweet::{
            models::{MediaField, PlaceField, PollField, TweetField, UserField},
            TWITTER_API_BASE,
        },
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,
    /// List ID to retrieve
    list_id: String,

    /// The maximum number of results
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<i32>,

    /// The pagination token
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination_token: Option<String>,

    /// A comma separated list of Tweet fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    tweet_fields: Option<Vec<TweetField>>,

    /// A comma separated list of fields to expand
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions: Option<Vec<Expansion>>,

    /// A comma separated list of Media fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    media_fields: Option<Vec<MediaField>>,

    /// A comma separated list of Poll fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    poll_fields: Option<Vec<PollField>>,

    /// A comma separated list of User fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,

    /// A comma separated list of Place fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    place_fields: Option<Vec<PlaceField>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The list of tweets
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Vec<ListTweet>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
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

pub(crate) struct GetListTweets {
    api_base: String,
}

impl NexusTool for GetListTweets {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/lists",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-list-tweets@1")
    }

    fn path() -> &'static str {
        "/get-list-tweets"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        match self.fetch_list_tweets(&request).await {
            Ok(list_response) => Output::Ok {
                data: list_response.data,
                meta: list_response.meta,
                includes: list_response.includes,
            },
            Err(e) => {
                let error_response: TwitterErrorResponse = e.to_error_response();

                Output::Err {
                    reason: error_response.reason,
                    kind: error_response.kind,
                    status_code: error_response.status_code,
                }
            }
        }
    }
}

impl GetListTweets {
    /// Fetch tweets from a list using the Twitter API
    async fn fetch_list_tweets(&self, request: &Input) -> TwitterResult<ListTweetsResponse> {
        let client = Client::new();

        let url = format!("{}/{}/tweets", self.api_base, request.list_id);
        let mut req_builder = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", request.bearer_token));

        if let Some(max_results) = request.max_results {
            req_builder = req_builder.query(&[("max_results", max_results.to_string())]);
        }

        if let Some(pagination_token) = &request.pagination_token {
            req_builder = req_builder.query(&[("pagination_token", pagination_token)]);
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
            req_builder = req_builder.query(&[("tweet.fields", fields.join(","))]);
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
            req_builder = req_builder.query(&[("expansions", fields.join(","))]);
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
            req_builder = req_builder.query(&[("media.fields", fields.join(","))]);
        }

        if let Some(poll_fields) = &request.poll_fields {
            let fields: Vec<String> = poll_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            req_builder = req_builder.query(&[("poll.fields", fields.join(","))]);
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
            req_builder = req_builder.query(&[("user.fields", fields.join(","))]);
        }

        if let Some(place_fields) = &request.place_fields {
            let fields: Vec<String> = place_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            req_builder = req_builder.query(&[("place.fields", fields.join(","))]);
        }

        let response = req_builder.send().await?;
        parse_twitter_response::<ListTweetsResponse>(response).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetListTweets {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetListTweets) {
        let server = Server::new_async().await;
        let tool = GetListTweets::with_api_base(&(server.url() + "/lists"));
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            list_id: "test_list_id".to_string(),
            max_results: Some(10),
            pagination_token: None,
            tweet_fields: Some(vec![TweetField::Text, TweetField::AuthorId]),
            expansions: Some(vec![Expansion::OwnerId]),
            user_fields: Some(vec![UserField::Username, UserField::Name]),
            media_fields: None,
            poll_fields: None,
            place_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_list_tweets_successful() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/lists/test_list_id/tweets")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "author_id": "12345",
                            "id": "111222333444",
                            "text": "This is a test tweet in the list"
                        },
                        {
                            "author_id": "67890",
                            "id": "555666777888",
                            "text": "Another test tweet in the list"
                        }
                    ],
                    "includes": {
                        "users": [
                            {
                                "id": "12345",
                                "name": "Test User 1",
                                "username": "testuser1"
                            },
                            {
                                "id": "67890",
                                "name": "Test User 2",
                                "username": "testuser2"
                            }
                        ]
                    },
                    "meta": {
                        "result_count": 2,
                        "next_token": "next_page_token"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the list tweets request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Ok {
                data,
                meta,
                includes,
            } => {
                assert!(data.is_some());
                let data = data.unwrap();
                assert_eq!(data.len(), 2);
                assert_eq!(data[0].id, "111222333444");
                assert_eq!(data[0].text, "This is a test tweet in the list");
                assert_eq!(data[1].id, "555666777888");

                // Check that includes contains the right users
                let includes = includes.unwrap();
                let users = includes.users.unwrap();
                assert_eq!(users.len(), 2);
                assert_eq!(users[0].username, "testuser1");
                assert_eq!(users[1].username, "testuser2");

                // Check meta data
                let meta = meta.unwrap();
                assert_eq!(meta.result_count.unwrap(), 2);
                assert_eq!(meta.next_token.unwrap(), "next_page_token");
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
    async fn test_get_list_tweets_not_found() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for not found
        let mock = server
            .mock("GET", "/lists/test_list_id/tweets")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "message": "List not found",
                            "code": 34,
                            "title": "Not Found",
                            "type": "about:blank"
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the list tweets request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("List not found"),
                    "Expected error message to contain 'List not found', got: {}",
                    reason
                );

                // Accept either NotFound or Api error kinds
                if kind == TwitterErrorKind::NotFound || kind == TwitterErrorKind::Api {
                    // Good
                } else {
                    panic!("Expected error kind NotFound or Api, got: {:?}", kind);
                }

                // Check status code - it might be 404 or None depending on the response structure
                if status_code != Some(404) && status_code.is_some() {
                    panic!("Expected status code 404 or None, got: {:?}", status_code);
                }
            }
            Output::Ok {
                data,
                meta: _,
                includes: _,
            } => panic!("Expected error, got success: {:?}", data),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unauthorized_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/lists/test_list_id/tweets")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "message": "Unauthorized",
                        "code": 32,
                        "title": "Unauthorized",
                        "type": "https://api.twitter.com/2/problems/not-authorized"
                    }]
                })
                .to_string(),
            )
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
                    reason.contains("Unauthorized"),
                    "Expected error message to contain 'Unauthorized', got: {}",
                    reason
                );

                // Accept either Auth or Api error kinds
                if kind == TwitterErrorKind::Auth || kind == TwitterErrorKind::Api {
                    // Good
                } else {
                    panic!("Expected error kind Auth or Api, got: {:?}", kind);
                }

                // Check status code - it might be 401 or None depending on the response structure
                if status_code != Some(401) && status_code.is_some() {
                    panic!("Expected status code 401 or None, got: {:?}", status_code);
                }
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/lists/test_list_id/tweets")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "message": "Rate limit exceeded",
                        "code": 88,
                        "title": "Rate Limit Exceeded",
                        "type": "https://api.twitter.com/2/problems/rate-limit-exceeded"
                    }]
                })
                .to_string(),
            )
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
                    reason.contains("Rate limit exceeded")
                        || reason.contains("Rate Limit Exceeded"),
                    "Expected error message to contain rate limit information, got: {}",
                    reason
                );

                // Accept either RateLimit or Api error kinds
                if kind == TwitterErrorKind::RateLimit || kind == TwitterErrorKind::Api {
                    // Good
                } else {
                    panic!("Expected error kind RateLimit or Api, got: {:?}", kind);
                }

                // Check status code - it might be 429 or None depending on the response structure
                if status_code != Some(429) && status_code.is_some() {
                    panic!("Expected status code 429 or None, got: {:?}", status_code);
                }
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_response() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/lists/test_list_id/tweets")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
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
                meta,
                includes: _,
            } => {
                assert!(data.is_none() || data.unwrap().is_empty());
                assert!(meta.is_some());
                if let Some(meta) = meta {
                    assert_eq!(meta.result_count.unwrap(), 0);
                }
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
    async fn test_invalid_json_response() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/lists/test_list_id/tweets")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
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
                    "Expected error message to contain 'Response parsing error', got: {}",
                    reason
                );

                // Check for Parse error kind
                assert_eq!(kind, TwitterErrorKind::Parse, "Expected Parse error kind");

                // Parsing errors typically don't have status codes
                assert_eq!(
                    status_code, None,
                    "Parsing errors should not have status codes"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }
}
