//! # `xyz.taluslabs.social.twitter.retweet-tweet@1`
//!
//! Standard Nexus Tool that retweets a tweet.

use {
    super::models::RetweetResponse,
    crate::{
        auth::TwitterAuth,
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
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,
    /// The ID of the authenticated source User that is requesting to repost the Post.
    user_id: String,
    /// Unique identifier of this Tweet. This is returned as a string in order to avoid complications with languages and tools that cannot handle large integers.
    tweet_id: String,
}

#[derive(Serialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Tweet ID to retweet
        tweet_id: String,
        /// Whether the tweet was retweeted
        retweeted: bool,
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

pub(crate) struct RetweetTweet {
    api_base: String,
}

impl NexusTool for RetweetTweet {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.retweet-tweet@1")
    }

    fn path() -> &'static str {
        "/retweet-tweet"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Build the endpoint for the Twitter API
        let suffix = format!("users/{}/retweets", request.user_id);

        // Create a Twitter client with the mock server URL
        let client = match TwitterClient::new(Some(&suffix), Some(&self.api_base)) {
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
            .post::<RetweetResponse, _>(
                &request.auth,
                Some(json!({ "tweet_id": request.tweet_id })),
                None,
            )
            .await
        {
            Ok(data) => Output::Ok {
                tweet_id: data.rest_id,
                retweeted: data.retweeted,
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
        ::{mockito::Server, serde_json::json},
    };

    impl RetweetTweet {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, RetweetTweet) {
        let server = Server::new_async().await;
        let tool = RetweetTweet::with_api_base(&server.url());
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
            user_id: "12345".to_string(),
            tweet_id: "67890".to_string(),
        }
    }

    #[tokio::test]
    async fn test_successful_retweet() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "rest_id": "67890",
                        "retweeted": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = tool.invoke(create_test_input()).await;

        match result {
            Output::Ok {
                tweet_id,
                retweeted,
            } => {
                assert_eq!(tweet_id, "67890");
                assert!(retweeted);
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
    async fn test_unauthorized_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "status": 401,
                    "title": "Unauthorized",
                    "type": "https://api.twitter.com/2/problems/unauthorized",
                    "detail": "Unauthorized"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = tool.invoke(create_test_input()).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
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
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "status": 429,
                    "title": "Too Many Requests",
                    "type": "https://api.twitter.com/2/problems/rate-limit-exceeded",
                    "detail": "Rate limit exceeded"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = tool.invoke(create_test_input()).await;
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::RateLimit,
                    "Expected error kind RateLimit, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Too Many Requests"),
                    "Expected error message to contain 'Too Many Requests', got: {}",
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
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_tweet_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "title": "Not Found Error",
                        "type": "https://api.twitter.com/2/problems/resource-not-found",
                        "detail": "Tweet not found"
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result = tool.invoke(create_test_input()).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
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

                // Check status code
                assert_eq!(
                    status_code,
                    Some(404),
                    "Expected status code 404, got: {:?}",
                    status_code
                );
            }
        }

        mock.assert_async().await;
    }
}
