//! # `xyz.taluslabs.social.twitter.get-tweet@1`
//!
//! Standard Nexus Tool that retrieves a single tweet from the Twitter API.

use {
    crate::{
        error::{parse_twitter_response, TwitterErrorKind, TwitterErrorResponse, TwitterResult},
        tweet::{
            models::{
                Attachments, ContextAnnotation, EditControls, Entities, Geo, GetTweetResponse,
                Includes, Meta, NonPublicMetrics, NoteTweet, OrganicMetrics, PromotedMetrics,
                PublicMetrics, ReferencedTweet, Scopes, Withheld,
            },
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
    /// Tweet ID to retrieve
    tweet_id: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The tweet's unique identifier
        id: String, // mandatory
        /// The tweet's content text
        text: String, // mandatory
        /// The ID of the tweet's author
        #[serde(skip_serializing_if = "Option::is_none")]
        author_id: Option<String>,
        /// The timestamp when the tweet was created
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
        /// The username of the tweet's author
        #[serde(skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        /// Media and polls attached to the tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        attachments: Option<Attachments>,
        /// Community ID if the tweet belongs to a community
        #[serde(skip_serializing_if = "Option::is_none")]
        community_id: Option<String>,
        /// Annotations about the tweet content
        #[serde(skip_serializing_if = "Option::is_none")]
        context_annotations: Option<Vec<ContextAnnotation>>,
        /// ID of the conversation this tweet belongs to
        #[serde(skip_serializing_if = "Option::is_none")]
        conversation_id: Option<String>,
        /// Controls for editing the tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        edit_controls: Option<EditControls>,
        /// IDs of tweets in the edit history
        #[serde(skip_serializing_if = "Option::is_none")]
        edit_history_tweet_ids: Option<Vec<String>>,
        /// Entities in the tweet (hashtags, mentions, URLs)
        #[serde(skip_serializing_if = "Option::is_none")]
        entities: Option<Entities>,
        /// Geographic information
        #[serde(skip_serializing_if = "Option::is_none")]
        geo: Option<Geo>,
        /// ID of the user being replied to
        #[serde(skip_serializing_if = "Option::is_none")]
        in_reply_to_user_id: Option<String>,
        /// Language of the tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        lang: Option<String>,
        /// Private metrics about the tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        non_public_metrics: Option<NonPublicMetrics>,
        /// Extended note content
        #[serde(skip_serializing_if = "Option::is_none")]
        note_tweet: Option<NoteTweet>,
        /// Organic engagement metrics
        #[serde(skip_serializing_if = "Option::is_none")]
        organic_metrics: Option<OrganicMetrics>,
        /// Whether the tweet might contain sensitive content
        #[serde(skip_serializing_if = "Option::is_none")]
        possibly_sensitive: Option<bool>,
        /// Metrics from promoted content
        #[serde(skip_serializing_if = "Option::is_none")]
        promoted_metrics: Option<PromotedMetrics>,
        /// Public engagement metrics (likes, retweets, etc.)
        #[serde(skip_serializing_if = "Option::is_none")]
        public_metrics: Option<PublicMetrics>,
        /// Tweets referenced by this tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        referenced_tweets: Option<Vec<ReferencedTweet>>,
        /// Who can reply to this tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        reply_settings: Option<String>,
        /// Visibility scopes
        #[serde(skip_serializing_if = "Option::is_none")]
        scopes: Option<Scopes>,
        /// Source of the tweet (client application)
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        /// Withholding information
        #[serde(skip_serializing_if = "Option::is_none")]
        withheld: Option<Withheld>,
        /// Additional entities related to the tweet:
        /// - media: Images and videos
        /// - places: Geographic locations
        /// - polls: Twitter polls
        /// - topics: Related topics
        /// - tweets: Referenced tweets
        /// - users: Mentioned users
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
        /// Metadata about the tweet request:
        /// - newest_id: Newest tweet ID in a collection
        /// - next_token: Pagination token for next results
        /// - oldest_id: Oldest tweet ID in a collection
        /// - previous_token: Pagination token for previous results
        /// - result_count: Number of results returned
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

pub(crate) struct GetTweet {
    api_base: String,
}

impl NexusTool for GetTweet {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/tweets",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-tweet@1")
    }

    fn path() -> &'static str {
        "/get-tweet"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        let client = Client::new();

        // Add authentication header
        let url = format!("{}/{}", self.api_base, request.tweet_id);

        // Make the request
        match self.fetch_tweet(&client, &url, &request.bearer_token).await {
            Ok(response) => {
                if let Some(tweet) = response.data {
                    Output::Ok {
                        id: tweet.id,
                        text: tweet.text,
                        author_id: tweet.author_id,
                        created_at: tweet.created_at,
                        username: tweet.username,
                        attachments: tweet.attachments,
                        community_id: tweet.community_id,
                        context_annotations: tweet.context_annotations,
                        conversation_id: tweet.conversation_id,
                        edit_controls: tweet.edit_controls,
                        edit_history_tweet_ids: tweet.edit_history_tweet_ids,
                        entities: tweet.entities,
                        geo: tweet.geo,
                        in_reply_to_user_id: tweet.in_reply_to_user_id,
                        lang: tweet.lang,
                        non_public_metrics: tweet.non_public_metrics,
                        note_tweet: tweet.note_tweet,
                        organic_metrics: tweet.organic_metrics,
                        possibly_sensitive: tweet.possibly_sensitive,
                        promoted_metrics: tweet.promoted_metrics,
                        public_metrics: tweet.public_metrics,
                        referenced_tweets: tweet.referenced_tweets,
                        reply_settings: tweet.reply_settings,
                        scopes: tweet.scopes,
                        source: tweet.source,
                        withheld: tweet.withheld,
                        includes: response.includes,
                        meta: response.meta,
                    }
                } else {
                    // Return an error if there's no tweet data
                    let error_response = TwitterErrorResponse {
                        reason: "No tweet data found in the response".to_string(),
                        kind: TwitterErrorKind::NotFound,
                        status_code: None,
                    };

                    Output::Err {
                        reason: error_response.reason,
                        kind: error_response.kind,
                        status_code: error_response.status_code,
                    }
                }
            }
            Err(e) => {
                // Use the centralized error conversion
                let error_response = e.to_error_response();

                Output::Err {
                    reason: error_response.reason,
                    kind: error_response.kind,
                    status_code: error_response.status_code,
                }
            }
        }
    }
}

impl GetTweet {
    /// Fetch tweet from Twitter API
    async fn fetch_tweet(
        &self,
        client: &Client,
        url: &str,
        bearer_token: &str,
    ) -> TwitterResult<GetTweetResponse> {
        let response = client
            .get(url)
            .header("Authorization", format!("Bearer {}", bearer_token))
            .send()
            .await?;

        parse_twitter_response::<GetTweetResponse>(response).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetTweet {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetTweet) {
        let server = Server::new_async().await;
        let tool = GetTweet::with_api_base(&(server.url() + "/tweets"));
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            tweet_id: "test_tweet_id".to_string(),
        }
    }

    #[tokio::test]
    async fn test_get_tweet_successful() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/tweets/test_tweet_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                   "data": {
                        "author_id": "2244994945",
                        "created_at": "Wed Jan 06 18:40:40 +0000 2021",
                        "id": "1346889436626259968",
                        "text": "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2 to explore Tweet\\u2026 https:\\/\\/t.co\\/56a0vZUx7i",
                        "username": "XDevelopers"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the tweet request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Ok {
                id,
                text,
                author_id,
                created_at,
                username,
                ..
            } => {
                assert_eq!(id, "1346889436626259968");
                assert_eq!(text, "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2 to explore Tweet\\u2026 https:\\/\\/t.co\\/56a0vZUx7i");
                assert_eq!(author_id, Some("2244994945".to_string()));
                assert!(created_at.is_some());
                assert_eq!(username, Some("XDevelopers".to_string()));
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
    async fn test_get_tweet_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/test_tweet_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "value": "test_tweet_id",
                            "detail": "Could not find tweet with id: [test_tweet_id].",
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
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::NotFound,
                    "Expected error kind NotFound, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Not Found Error"),
                    "Expected error message to contain 'Not Found Error', got: {}",
                    reason
                );
                assert!(
                    reason.contains("Could not find tweet with id"),
                    "Expected error message to contain tweet ID details, got: {}",
                    reason
                );

                // Check status code
                assert_eq!(
                    status_code,
                    Some(404),
                    "Expected status code 404, got: {:?}",
                    status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_tweet_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/test_tweet_id")
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
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_tweet_rate_limit() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/test_tweet_id")
            .match_header("Authorization", "Bearer test_bearer_token")
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
                println!("Kind: {:?}", kind);
                println!("reason: {:?}", reason);
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::RateLimit,
                    "Expected error kind RateLimit, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected error message to contain 'Rate limit exceeded', got: {}",
                    reason
                );

                // Check status code
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
