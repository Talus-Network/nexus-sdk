//! # `xyz.taluslabs.social.twitter.retweet-tweet@1`
//!
//! Standard Nexus Tool that retweets a tweet.

use {
    super::models::ApiError,
    crate::{
        auth::TwitterAuth,
        error::{parse_twitter_response_v2, TwitterResult},
        tweet::TWITTER_API_BASE,
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
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
            api_base: TWITTER_API_BASE.to_string() + "/users",
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
        match self.retweet_tweet(&request).await {
            Ok(response) => Output::Ok {
                tweet_id: response.rest_id,
                retweeted: response.retweeted,
            },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RetweetData {
    pub rest_id: String,
    pub retweeted: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RetweetResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<RetweetData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
}

impl RetweetTweet {
    async fn retweet_tweet(&self, request: &Input) -> TwitterResult<RetweetData> {
        let client = Client::new();

        let url = format!("{}/{}/retweets", self.api_base, request.user_id);

        // Generate OAuth authorization header with the complete URL
        let auth_header = request.auth.generate_auth_header(&url);

        // Format the request body with the tweet_id
        let request_body = format!(r#"{{"tweet_id": "{}"}}"#, request.tweet_id);

        let response = client
            .post(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await?;

        let (data, _, _) = parse_twitter_response_v2::<RetweetData>(response).await?;
        Ok(data)
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
        let tool = RetweetTweet::with_api_base(&(server.url() + "/users"));
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
        println!("Test result: {:?}", result);

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
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for 401 Unauthorized response
        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(401)
            .with_header("content-type", "application/problem+json")
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

        // Test the retweet request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(reason.contains("Unauthorized"));
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(429)
            .with_header("content-type", "application/problem+json")
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
                assert!(reason.contains("Rate limit exceeded"));
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_tweet_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "title": "Not Found Error",
                        "type": "https://api.twitter.com/2/problems/resource-not-found",
                        "detail": "Tweet not found",
                        "value": "67890",
                        "resource_type": "tweet",
                        "parameter": "tweet_id",
                        "resource_id": "67890"
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
                assert!(reason.contains("Twitter API error: Not Found Error (type: https://api.twitter.com/2/problems/resource-not-found)"));
            }
        }

        mock.assert_async().await;
    }
}
