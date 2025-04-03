//! # `xyz.taluslabs.social.twitter.retweet-tweet@1`
//!
//! Standard Nexus Tool that retweets a tweet.

use {
    crate::{auth::TwitterAuth, tweet::TWITTER_API_BASE},
    reqwest::Client,
    ::{
        nexus_sdk::{fqn, ToolFqn},
        nexus_toolkit::*,
        schemars::JsonSchema,
        serde::{Deserialize, Serialize},
        serde_json::Value,
    },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,
    /// The id of authenticated user
    user_id: String,
    /// Tweet ID to retweet
    tweet_id: String,
}

#[derive(Serialize, JsonSchema)]
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
            .await;

        match response {
            Err(e) => Output::Err {
                reason: format!("Failed to send retweet request to Twitter API: {}", e),
            },
            Ok(result) => {
                let text = match result.text().await {
                    Err(e) => {
                        return Output::Err {
                            reason: format!("Failed to read Twitter API response: {}", e),
                        }
                    }
                    Ok(text) => text,
                };

                let json: Value = match serde_json::from_str(&text) {
                    Err(e) => {
                        return Output::Err {
                            reason: format!("Invalid JSON response: {}", e),
                        }
                    }
                    Ok(json) => json,
                };

                // Check for error response with code/message format
                if let Some(code) = json.get("code") {
                    let message = json
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");

                    return Output::Err {
                        reason: format!("Twitter API error: {} (Code: {})", message, code),
                    };
                }

                // Check for error response with detail/status/title format
                if let Some(detail) = json.get("detail") {
                    let status = json.get("status").and_then(|s| s.as_u64()).unwrap_or(0);
                    let title = json
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Unknown");

                    return Output::Err {
                        reason: format!(
                            "Twitter API error: {} (Status: {}, Title: {})",
                            detail.as_str().unwrap_or("Unknown error"),
                            status,
                            title
                        ),
                    };
                }

                // Check for errors array
                if let Some(errors) = json.get("errors") {
                    return Output::Err {
                        reason: format!("Twitter API returned errors: {}", errors),
                    };
                }

                // Check for success response format
                let data = match json.get("data") {
                    None => {
                        return Output::Err {
                            reason: format!(
                                "Unexpected response format from Twitter API: {}",
                                json
                            ),
                        }
                    }
                    Some(data) => data,
                };

                // Check for retweeted field
                let retweeted = match data.get("retweeted") {
                    None => {
                        return Output::Err {
                            reason: format!(
                                "Unexpected response format from Twitter API: {}",
                                json
                            ),
                        }
                    }
                    Some(retweeted) => retweeted.as_bool().unwrap_or(false),
                };

                // Check if the tweet was retweeted
                if !retweeted {
                    return Output::Err {
                        reason: format!(
                            "Twitter API indicated the tweet was not retweeted: {}",
                            json
                        ),
                    };
                }

                // Successfully retweeted the tweet
                Output::Ok {
                    tweet_id: request.tweet_id,
                    retweeted,
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
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for successful retweet
        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "retweeted": true
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the retweet request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok {
                tweet_id,
                retweeted,
            } => {
                assert_eq!(tweet_id, "67890");
                assert_eq!(retweeted, true);
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
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
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "detail": "Unauthorized",
                    "status": 401,
                    "title": "Unauthorized",
                    "type": "about:blank"
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
                assert!(
                    reason.contains("Unauthorized") && reason.contains("Status: 401"),
                    "Error should indicate unauthorized access. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_json_response() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for invalid JSON response
        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(200)
            .with_body("invalid json")
            .create_async()
            .await;

        // Test the retweet request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid JSON"),
                    "Error should indicate invalid JSON. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unexpected_format() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for unexpected response format
        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "some_other_field": true
                    }
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
                assert!(
                    reason.contains("Unexpected response format"),
                    "Error should indicate unexpected format. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_tweet_already_retweeted() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for already retweeted response
        let mock = server
            .mock("POST", "/users/12345/retweets")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "retweeted": false
                    }
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
                assert!(
                    reason.contains("not retweeted"),
                    "Error should indicate tweet was not retweeted. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
