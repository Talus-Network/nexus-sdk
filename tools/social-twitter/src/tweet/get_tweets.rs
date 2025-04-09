//! # xyz.taluslabs.social.twitter.get-tweets@1
//!
//! Standard Nexus Tool that retrieves tweets from a user's Twitter account.

use {
    crate::{
        error::{
            parse_twitter_response,
            TwitterError,
            TwitterErrorKind,
            TwitterErrorResponse,
            TwitterResult,
        },
        tweet::{
            models::{Includes, Meta, Tweet, TweetsResponse},
            TWITTER_API_BASE,
        },
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
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
    tweet_fields: Option<Vec<String>>,
    /// Optional: Fields to expand in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions: Option<Vec<String>>,
    /// Optional: Media fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    media_fields: Option<Vec<String>>,
    /// Optional: Poll fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    poll_fields: Option<Vec<String>>,
    /// Optional: User fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<String>>,
    /// Optional: Place fields to include in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    place_fields: Option<Vec<String>>,
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
            api_base: TWITTER_API_BASE.to_string() + "/tweets",
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
        let client = Client::new();

        // Make the request
        match self.fetch_tweets(client, &request).await {
            Ok(response) => {
                if let Some(tweets) = response.data {
                    Output::Ok {
                        data: tweets,
                        includes: response.includes,
                        meta: response.meta,
                    }
                } else {
                    let error_response = TwitterErrorResponse {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No tweet data or errors found in the response".to_string(),
                        status_code: None,
                    };

                    // Return an error if there's no tweet data and no errors
                    Output::Err {
                        kind: error_response.kind,
                        reason: error_response.reason,
                        status_code: error_response.status_code,
                    }
                }
            }
            Err(e) => {
                // Use the centralized error conversion
                let error_response = e.to_error_response();

                Output::Err {
                    kind: error_response.kind,
                    reason: error_response.reason,
                    status_code: error_response.status_code,
                }
            }
        }
    }
}

impl GetTweets {
    /// Fetch multiple tweets from Twitter API
    async fn fetch_tweets(&self, client: Client, request: &Input) -> TwitterResult<TweetsResponse> {
        // Construct the URL with query parameters
        let mut url =
            reqwest::Url::parse(&self.api_base).map_err(|e| TwitterError::Other(e.to_string()))?;

        // Add the tweet IDs
        let ids = request.ids.join(",");
        url.query_pairs_mut().append_pair("ids", &ids);

        // Add optional query parameters if provided
        if let Some(tweet_fields) = &request.tweet_fields {
            url.query_pairs_mut()
                .append_pair("tweet.fields", &tweet_fields.join(","));
        }

        if let Some(expansions) = &request.expansions {
            url.query_pairs_mut()
                .append_pair("expansions", &expansions.join(","));
        }

        if let Some(media_fields) = &request.media_fields {
            url.query_pairs_mut()
                .append_pair("media.fields", &media_fields.join(","));
        }

        if let Some(poll_fields) = &request.poll_fields {
            url.query_pairs_mut()
                .append_pair("poll.fields", &poll_fields.join(","));
        }

        if let Some(user_fields) = &request.user_fields {
            url.query_pairs_mut()
                .append_pair("user.fields", &user_fields.join(","));
        }

        if let Some(place_fields) = &request.place_fields {
            url.query_pairs_mut()
                .append_pair("place.fields", &place_fields.join(","));
        }

        // Make the request
        let response = client
            .get(url)
            .header("Authorization", format!("Bearer {}", request.bearer_token))
            .send()
            .await?;

        parse_twitter_response::<TweetsResponse>(response).await
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
        let tool = GetTweets::with_api_base(&(server.url() + "/tweets"));
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
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ids".into(), "1346889436626259968,1346889436626259969".into()),
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
    async fn test_get_tweets_partial_success() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets")
            .match_query(mockito::Matcher::Any)
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
                        }
                    ],
                    "errors": [
                        {
                            "detail": "Could not find tweet with id: [1346889436626259969].",
                            "title": "Not Found Error",
                            "type": "https://api.twitter.com/2/problems/resource-not-found"
                        }
                    ]
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
                    TwitterErrorKind::NotFound,
                    "Expected error kind NotFound, got: {:?}",
                    kind
                );

                assert!(
                    reason.contains("Not Found Error"),
                    "Expected error message to contain 'Not Found Error', got: {}",
                    reason
                );
                assert!(
                    reason.contains("Could not find tweet"),
                    "Expected error message to contain details about missing tweet, got: {}",
                    reason
                );

                assert_eq!(
                    status_code,
                    Some(404),
                    "Expected status code 404, got: {:?}",
                    status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error with missing tweets, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_tweets_with_field_parameters() {
        let (mut server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.tweet_fields = Some(vec!["created_at".to_string(), "author_id".to_string()]);
        input.expansions = Some(vec!["author_id".to_string()]);
        input.user_fields = Some(vec!["username".to_string(), "name".to_string()]);

        let mock = server
            .mock("GET", "/tweets")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ids".into(), "1346889436626259968,1346889436626259969".into()),
                mockito::Matcher::UrlEncoded("tweet.fields".into(), "created_at,author_id".into()),
                mockito::Matcher::UrlEncoded("expansions".into(), "author_id".into()),
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
