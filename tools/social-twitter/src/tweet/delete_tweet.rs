//! # `xyz.taluslabs.social.twitter.delete-tweet@1`
//!
//! Standard Nexus Tool that deletes a tweet.

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
    /// Tweet ID to delete
    tweet_id: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Whether the tweet was deleted
        deleted: bool,
    },
    Err {
        /// Error message
        reason: String,
    },
}

pub(crate) struct DeleteTweet {
    api_base: String,
}

impl NexusTool for DeleteTweet {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/tweets",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.delete-tweet@1")
    }

    fn path() -> &'static str {
        "/delete-tweet"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        let client = Client::new();

        let url = format!("{}/{}", self.api_base, request.tweet_id);

        // Generate OAuth authorization header with the complete URL
        let auth_header = request.auth.generate_auth_header_for_delete(&url);

        let response = client
            .delete(&url)
            .header("Authorization", auth_header)
            .send()
            .await;

        match response {
            Err(e) => Output::Err {
                reason: format!("Failed to send delete request to Twitter API: {}", e),
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

                let deleted = match data.get("deleted") {
                    None => {
                        return Output::Err {
                            reason: format!(
                                "Unexpected response format from Twitter API: {}",
                                json
                            ),
                        }
                    }
                    Some(deleted) => deleted.as_bool().unwrap_or(false),
                };

                if !deleted {
                    return Output::Err {
                        reason: format!(
                            "Twitter API indicated the tweet was not deleted: {}",
                            json
                        ),
                    };
                }

                Output::Ok { deleted }
            }
        }
    }
}
