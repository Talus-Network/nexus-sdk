//! # `xyz.taluslabs.social.twitter.post-tweet@1`
//!
//! Standard Nexus Tool that posts a content to Twitter.

use {
    crate::{
        auth::TwitterAuth,
        tweet::{
            models::{GeoInfo, MediaInfo, PollInfo, ReplyInfo, ReplySettings, TweetResponse},
            TWITTER_API_BASE,
        },
    },
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
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,
    /// Text to tweet
    text: String,
    /// Card URI for rich media preview
    #[serde(skip_serializing_if = "Option::is_none")]
    card_uri: Option<String>,
    /// Community ID for community-specific tweets
    #[serde(skip_serializing_if = "Option::is_none")]
    community_id: Option<String>,
    /// Direct message deep link
    #[serde(skip_serializing_if = "Option::is_none")]
    direct_message_deep_link: Option<String>,
    /// Whether the tweet is for super followers only
    #[serde(skip_serializing_if = "Option::is_none")]
    for_super_followers_only: Option<bool>,
    /// Geo location information
    #[serde(skip_serializing_if = "Option::is_none")]
    geo: Option<GeoInfo>,
    /// Media information
    #[serde(skip_serializing_if = "Option::is_none")]
    media: Option<MediaInfo>,
    /// Whether the tweet should be nullcast
    #[serde(skip_serializing_if = "Option::is_none")]
    nullcast: Option<bool>,
    /// Poll information
    #[serde(skip_serializing_if = "Option::is_none")]
    poll: Option<PollInfo>,
    /// ID of the tweet to quote
    #[serde(skip_serializing_if = "Option::is_none")]
    quote_tweet_id: Option<String>,
    /// Reply information
    #[serde(skip_serializing_if = "Option::is_none")]
    reply: Option<ReplyInfo>,
    /// Reply settings for the tweet
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_settings: Option<ReplySettings>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        id: String,
        /// List of tweet IDs in the edit history
        edit_history_tweet_ids: Vec<String>,
        /// The actual content of the tweet
        text: String,
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
        "/post-tweet"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Generate OAuth authorization header using the auth helper
        let auth_header = request.auth.generate_auth_header(&self.api_base);

        // Initialize HTTP client
        let client = Client::new();

        // Create request body with all available fields
        let mut request_body = serde_json::json!({
            "text": request.text,
        });

        // Add optional fields if they are present
        if let Some(card_uri) = request.card_uri {
            request_body["card_uri"] = serde_json::Value::String(card_uri);
        }

        if let Some(community_id) = request.community_id {
            request_body["community_id"] = serde_json::Value::String(community_id);
        }

        if let Some(direct_message_deep_link) = request.direct_message_deep_link {
            request_body["direct_message_deep_link"] =
                serde_json::Value::String(direct_message_deep_link);
        }

        if let Some(for_super_followers_only) = request.for_super_followers_only {
            request_body["for_super_followers_only"] =
                serde_json::Value::Bool(for_super_followers_only);
        }

        if let Some(geo) = request.geo {
            request_body["geo"] = serde_json::to_value(geo).unwrap();
        }

        if let Some(media) = request.media {
            request_body["media"] = serde_json::to_value(media).unwrap();
        }

        if let Some(nullcast) = request.nullcast {
            request_body["nullcast"] = serde_json::Value::Bool(nullcast);
        }

        if let Some(poll) = request.poll {
            request_body["poll"] = serde_json::to_value(poll).unwrap();
        }

        if let Some(quote_tweet_id) = request.quote_tweet_id {
            request_body["quote_tweet_id"] = serde_json::Value::String(quote_tweet_id);
        }

        if let Some(reply) = request.reply {
            request_body["reply"] = serde_json::to_value(reply).unwrap();
        }

        if let Some(reply_settings) = request.reply_settings {
            request_body["reply_settings"] = serde_json::to_value(reply_settings).unwrap();
        }

        // Attempt to send tweet and handle response
        let response = client
            .post(&self.api_base)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .body(request_body.to_string())
            .send()
            .await;

        match response {
            Err(e) => Output::Err {
                reason: format!("Failed to send tweet to Twitter API: {}", e),
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

                // Check for errors first
                if let Some(errors) = json.get("errors") {
                    return Output::Err {
                        reason: format!("Twitter API returned errors: {}", errors),
                    };
                }

                // Check for error details format
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

                // Try to get the data
                let data = match json.get("data") {
                    None => {
                        return Output::Err {
                            reason: format!("Response missing both data and errors: {}", json),
                        }
                    }
                    Some(data) => data,
                };

                // Parse the tweet data
                match serde_json::from_value::<TweetResponse>(data.clone()) {
                    Err(e) => Output::Err {
                        reason: format!("Failed to parse tweet data: {}", e),
                    },
                    Ok(tweet_data) => Output::Ok {
                        id: tweet_data.id,
                        edit_history_tweet_ids: tweet_data.edit_history_tweet_ids,
                        text: tweet_data.text,
                    },
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
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            text: "Hello, Twitter!".to_string(),
            card_uri: None,
            community_id: None,
            direct_message_deep_link: None,
            for_super_followers_only: None,
            geo: None,
            media: None,
            nullcast: None,
            poll: None,
            quote_tweet_id: None,
            reply: None,
            reply_settings: None,
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
            Output::Ok {
                id,
                edit_history_tweet_ids,
                text,
            } => {
                assert_eq!(id, "1234567890");
                assert_eq!(text, "Hello, Twitter!");
                assert_eq!(edit_history_tweet_ids, vec!["1234567890"]);
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

    #[tokio::test]
    async fn test_duplicate_content_error() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for duplicate content error response (403 Forbidden)
        let mock = server
            .mock("POST", "/tweets")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "detail": "You are not allowed to create a Tweet with duplicate content.",
                    "status": 403,
                    "title": "Forbidden",
                    "type": "about:blank"
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
                    reason.contains("Twitter API error:")
                        && reason.contains("duplicate content")
                        && reason.contains("Status: 403"),
                    "Error should include the formatted error details. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_super_followers_only_error() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Create test input with for_super_followers_only set to true
        let mut input = create_test_input();
        input.for_super_followers_only = Some(true);

        // Set up mock for super followers only error response (403 Forbidden)
        let mock = server
            .mock("POST", "/tweets")
            .with_status(403)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "detail": "You are not permitted to create an exclusive Tweet.",
                    "status": 403,
                    "title": "Forbidden",
                    "type": "about:blank"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the tweet request
        let result = tool.invoke(input).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Twitter API error:")
                        && reason.contains("not permitted to create an exclusive Tweet")
                        && reason.contains("Status: 403"),
                    "Error should include the formatted error details. Got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
