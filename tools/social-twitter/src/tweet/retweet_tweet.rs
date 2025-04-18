//! # `xyz.taluslabs.social.twitter.retweet-tweet@1`
//!
//! Standard Nexus Tool that retweets a tweet.

use {
    super::models::RetweetResponse,
    crate::{
        auth::TwitterAuth,
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
        /// Error message
        reason: String,
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
                }
            }
        };

        match client
            .post::<RetweetResponse, _>(&request.auth, json!({ "tweet_id": request.tweet_id }))
            .await
        {
            Ok(response) => {
                // Check if we have data
                if let Some(data) = response.data {
                    println!("Retweeted tweet: {:?}", data);
                    return Output::Ok {
                        tweet_id: data.rest_id,
                        retweeted: data.retweeted,
                    };
                } else if let Some(errors) = response.errors {
                    return Output::Err {
                        reason: errors.first().unwrap().detail.clone().unwrap_or_default(),
                    };
                } else {
                    return Output::Err {
                        reason: "Failed to retweet: Unknown error".to_string(),
                    };
                }
            }
            Err(e) => {
                let error_response = e.to_string();
                Output::Err {
                    reason: error_response,
                }
            }
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
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
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
            Output::Err { reason } => {
                assert!(reason.contains("Unauthorized"));
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
            Output::Err { reason } => {
                assert!(reason.contains("Too Many Requests"));
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
            Output::Err { reason } => {
                assert!(reason.contains("Not Found Error"));
            }
        }

        mock.assert_async().await;
    }
}
