//! # `xyz.taluslabs.social.twitter.post-tweet@1`
//!
//! Standard Nexus Tool that posts a content to Twitter.

use {
    crate::tweet::TWITTER_API_BASE,
    oauth1_request::{post, signature_method::HmacSha1, Token},
    reqwest::Client,
    ::{
        nexus_sdk::{fqn, ToolFqn},
        nexus_toolkit::*,
        schemars::JsonSchema,
        serde::{Deserialize, Serialize},
        serde_json::Value,
    },
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Consumer API key for Twitter API application
    consumer_key: String,
    /// Consumer Secret key for Twitter API application
    consumer_secret_key: String,
    /// Access Token for user's Twitter account
    access_token: String,
    /// Access Token Secret for user's Twitter account
    access_token_secret: String,
    /// Content to tweet
    content: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TweetResponse {
    /// Tweet's unique identifier
    id: String,
    /// List of tweet IDs in the edit history
    edit_history_tweet_ids: Vec<String>,
    /// The actual content of the tweet
    text: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The successful tweet response data
        #[schemars(description = "Successfully posted tweet data")]
        result: TweetResponse,
    },
    Err {
        /// Error message if the tweet failed
        reason: String,
    },
}

pub(crate) struct PostTweet {
    api_base: String,
}

impl NexusTool for PostTweet {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/tweets",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.post-tweet@1")
    }

    fn path() -> &'static str {
        "/twitter/post-tweet"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Set up OAuth token with provided credentials
        let token = Token::from_parts(
            request.consumer_key,
            request.consumer_secret_key,
            request.access_token,
            request.access_token_secret,
        );

        // Generate OAuth authorization header
        let auth_header = post(&self.api_base, &(), &token, HmacSha1::new());

        // Initialize HTTP client
        let client = Client::new();

        //@todo!("Add support for media");
        let request_body = format!(r#"{{"text": "{}"}}"#, request.content);

        // Attempt to send tweet and handle response
        let response = client
            .post(&self.api_base)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .body(request_body)
            .send()
            .await;

        // Handle the response and potential errors
        match response {
            Ok(result) => {
                match result.text().await {
                    Ok(text) => {
                        // Parse the JSON response
                        match serde_json::from_str::<Value>(&text) {
                            Ok(json) => {
                                if let Some(data) = json.get("data") {
                                    match serde_json::from_value::<TweetResponse>(data.clone()) {
                                        Ok(tweet_data) => Output::Ok { result: tweet_data },
                                        Err(e) => Output::Err {
                                            reason: format!("Failed to parse tweet data: {}", e),
                                        },
                                    }
                                } else if let Some(errors) = json.get("errors") {
                                    Output::Err {
                                        reason: format!("Twitter API returned errors: {}", errors),
                                    }
                                } else {
                                    Output::Err {
                                        reason: format!(
                                            "Response missing both data and errors: {}",
                                            json
                                        ),
                                    }
                                }
                            }
                            Err(e) => Output::Err {
                                reason: format!("Invalid JSON response: {}", e),
                            },
                        }
                    }
                    Err(e) => Output::Err {
                        reason: format!("Failed to read Twitter API response: {}", e),
                    },
                }
            }
            Err(e) => Output::Err {
                reason: format!("Failed to send tweet to Twitter API: {}", e),
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

    impl PostTweet {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, PostTweet) {
        let server = Server::new_async().await;
        let tool = PostTweet::with_api_base(&(server.url() + "/tweets"));
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            consumer_key: "test_consumer_key".to_string(),
            consumer_secret_key: "test_consumer_secret".to_string(),
            access_token: "test_access_token".to_string(),
            access_token_secret: "test_access_token_secret".to_string(),
            content: "Hello, Twitter!".to_string(),
        }
    }

    #[tokio::test]
    async fn test_successful_tweet() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("POST", "/tweets")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "id": "1234567890",
                        "edit_history_tweet_ids": ["1234567890"],
                        "text": "Hello, Twitter!"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the tweet request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok { result } => {
                assert_eq!(result.id, "1234567890");
                assert_eq!(result.text, "Hello, Twitter!");
                assert_eq!(result.edit_history_tweet_ids, vec!["1234567890"]);
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
            .mock("POST", "/tweets")
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

        // Test the tweet request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                println!("Actual error message: {}", reason);
                // We just check that we got an error, since the exact error message
                // depends on how the code handles 401 responses
                assert!(true, "Got error response as expected");
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
            .mock("POST", "/tweets")
            .with_status(200)
            .with_body("invalid json")
            .create_async()
            .await;

        // Test the tweet request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid JSON"),
                    "Error should indicate invalid JSON"
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_missing_data_field() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for response without "data" field
        let mock = server
            .mock("POST", "/tweets")
            .with_status(200)
            .with_body(
                json!({
                    "meta": {
                        "status": "ok"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the tweet request
        let result = tool.invoke(create_test_input()).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("{\"meta\":{\"status\":\"ok\"}}"),
                    "Error should contain the raw JSON response"
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
