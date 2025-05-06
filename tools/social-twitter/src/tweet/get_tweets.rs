//! # xyz.taluslabs.social.twitter.get-tweets@1
//!
//! Standard Nexus Tool that retrieves tweets from a user's Twitter account.

use {
    crate::{
        error::{TwitterErrorKind, TwitterErrorResponse},
        list::models::Expansion,
        tweet::models::{
            Includes,
            MediaField,
            Meta,
            PlaceField,
            PollField,
            Tweet,
            TweetField,
            TweetsResponse,
            UserField,
        },
        twitter_client::{TwitterClient, TWITTER_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,
    /// A comma separated list of Tweet IDs. Up to 100 are allowed in a single request.
    ids: Vec<String>,
    /// Optional: Tweet fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    tweet_fields: Option<Vec<TweetField>>,
    /// Optional: Fields to expand in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions: Option<Vec<Expansion>>,
    /// Optional: Media fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    media_fields: Option<Vec<MediaField>>,
    /// Optional: Poll fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    poll_fields: Option<Vec<PollField>>,
    /// Optional: User fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,
    /// Optional: Place fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    place_fields: Option<Vec<PlaceField>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Array of tweet data
        data: Vec<Tweet>,
        /// Additional entities related to the tweets
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
        /// Metadata about the tweets request
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
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

pub(crate) struct GetTweets {
    api_base: String,
}

impl NexusTool for GetTweets {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-tweets@1")
    }

    fn path() -> &'static str {
        "/get-tweets"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Create a Twitter client with the mock server URL
        let client = match TwitterClient::new(Some("tweets"), Some(&self.api_base)) {
            Ok(client) => client,
            Err(e) => {
                return Output::Err {
                    reason: e.to_string(),
                    kind: TwitterErrorKind::Network,
                    status_code: None,
                }
            }
        };

        // Build query parameters
        let mut query_params = Vec::new();

        // Add tweet IDs
        query_params.push(("ids".to_string(), request.ids.join(",")));

        // Add tweet fields if provided
        if let Some(tweet_fields) = request.tweet_fields {
            let fields: Vec<String> = tweet_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("tweet.fields".to_string(), fields.join(",")));
        }

        // Add expansions if provided
        if let Some(expansions) = request.expansions {
            let fields: Vec<String> = expansions
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("expansions".to_string(), fields.join(",")));
        }

        // Add media fields if provided
        if let Some(media_fields) = request.media_fields {
            let fields: Vec<String> = media_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("media.fields".to_string(), fields.join(",")));
        }

        // Add poll fields if provided
        if let Some(poll_fields) = request.poll_fields {
            let fields: Vec<String> = poll_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("poll.fields".to_string(), fields.join(",")));
        }

        // Add user fields if provided
        if let Some(user_fields) = request.user_fields {
            let fields: Vec<String> = user_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("user.fields".to_string(), fields.join(",")));
        }

        // Add place fields if provided
        if let Some(place_fields) = request.place_fields {
            let fields: Vec<String> = place_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("place.fields".to_string(), fields.join(",")));
        }

        // Make the request with the client
        match client
            .get::<TweetsResponse>(request.bearer_token, Some(query_params))
            .await
        {
            Ok((data, includes, meta)) => {
                if data.is_empty() {
                    Output::Err {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No tweet data found in the response".to_string(),
                        status_code: None,
                    }
                } else {
                    // Return an error if there's no tweet data
                    Output::Ok {
                        data,
                        includes,
                        meta,
                    }
                }
            }
            Err(e) => {
                // Return the error
                Output::Err {
                    reason: e.reason,
                    kind: e.kind,
                    status_code: e.status_code,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetTweets {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetTweets) {
        let server = Server::new_async().await;
        let tool = GetTweets::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            ids: vec![
                "1346889436626259968".to_string(),
                "1346889436626259969".to_string(),
            ],
            tweet_fields: None,
            expansions: None,
            media_fields: None,
            poll_fields: None,
            user_fields: None,
            place_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_tweets_successful() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/tweets")
            .match_query(mockito::Matcher::UrlEncoded("ids".into(), "1346889436626259968,1346889436626259969".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                   "data": [
                        {
                            "author_id": "2244994945",
                            "created_at": "Wed Jan 06 18:40:40 +0000 2021",
                            "id": "1346889436626259968",
                            "text": "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2 to explore Tweet\\u2026 https:\\/\\/t.co\\/56a0vZUx7i",
                            "username": "XDevelopers"
                        },
                        {
                            "author_id": "2244994946",
                            "created_at": "Wed Jan 06 18:41:40 +0000 2021",
                            "id": "1346889436626259969",
                            "text": "Another tweet example",
                            "username": "XDevExample"
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the tweets request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Ok { data, .. } => {
                assert_eq!(data.len(), 2);

                // Check first tweet
                assert_eq!(data[0].id, "1346889436626259968");
                assert_eq!(data[0].text, "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2 to explore Tweet\\u2026 https:\\/\\/t.co\\/56a0vZUx7i");
                assert_eq!(data[0].author_id, Some("2244994945".to_string()));
                assert_eq!(data[0].username, Some("XDevelopers".to_string()));

                // Check second tweet
                assert_eq!(data[1].id, "1346889436626259969");
                assert_eq!(data[1].text, "Another tweet example");
                assert_eq!(data[1].author_id, Some("2244994946".to_string()));
                assert_eq!(data[1].username, Some("XDevExample".to_string()));
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_tweets_with_field_parameters() {
        let (mut server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.tweet_fields = Some(vec![TweetField::CreatedAt, TweetField::AuthorId]);
        input.expansions = Some(vec![Expansion::OwnerId]);
        input.user_fields = Some(vec![UserField::Username, UserField::Name]);

        let mock = server
            .mock("GET", "/tweets")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ids".into(), "1346889436626259968,1346889436626259969".into()),
                mockito::Matcher::UrlEncoded("tweet.fields".into(), "created_at,author_id".into()),
                mockito::Matcher::UrlEncoded("expansions".into(), "owner_id".into()),
                mockito::Matcher::UrlEncoded("user.fields".into(), "username,name".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "author_id": "2244994945",
                            "created_at": "Wed Jan 06 18:40:40 +0000 2021",
                            "id": "1346889436626259968",
                            "text": "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2"
                        }
                    ],
                    "includes": {
                        "users": [
                            {
                                "id": "2244994945",
                                "name": "X Dev",
                                "username": "TwitterDev",
                                "protected": false
                            }
                        ]
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { data, includes, .. } => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].id, "1346889436626259968");

                // Verify includes contains user data
                assert!(includes.is_some());
                let includes = includes.unwrap();
                assert!(includes.users.is_some());
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_tweets_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets")
            .match_query(mockito::Matcher::Any)
            .with_status(401)
            .with_header("content-type", "application/json")
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(
                    kind,
                    TwitterErrorKind::Auth,
                    "Expected error kind Auth, got: {:?}",
                    kind
                );

                assert!(
                    reason.contains("Unauthorized"),
                    "Expected error message to contain 'Unauthorized', got: {}",
                    reason
                );

                assert_eq!(
                    status_code,
                    Some(401),
                    "Expected status code 401, got: {:?}",
                    status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_tweets_rate_limit() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets")
            .match_query(mockito::Matcher::Any)
            .with_status(429)
            .with_header("content-type", "application/json")
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(
                    kind,
                    TwitterErrorKind::RateLimit,
                    "Expected error kind RateLimit, got: {:?}",
                    kind
                );

                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected error message to contain 'Rate limit exceeded', got: {}",
                    reason
                );

                assert_eq!(
                    status_code,
                    Some(429),
                    "Expected status code 429, got: {:?}",
                    status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }
}
