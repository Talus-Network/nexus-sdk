//! # xyz.taluslabs.social.twitter.get-user-by-username@1
//!
//! Standard Nexus Tool that retrieves a user from the Twitter API by username.

use {
    crate::{
        error::{parse_twitter_response, TwitterResult},
        list::models::Includes,
        tweet::{
            models::{ExpansionField, TweetField, UserField},
            TWITTER_API_BASE,
        },
        user::models::{
            Affiliation,
            ConnectionStatus,
            Entities,
            PublicMetrics,
            SubscriptionType,
            UserResponse,
            VerifiedType,
            Withheld,
        },
    },
    reqwest::Client,
    ::{
        nexus_sdk::{fqn, ToolFqn},
        nexus_toolkit::*,
        schemars::JsonSchema,
        serde::{Deserialize, Serialize},
    },
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,
    /// Username to retrieve
    username: String,
    /// A comma separated list of User fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,
    /// A comma separated list of fields to expand
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions_fields: Option<Vec<ExpansionField>>,
    /// A comma separated list of Tweet fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    tweet_fields: Option<Vec<TweetField>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The user's unique identifier
        id: String,
        /// The user's display name
        name: String,
        /// The user's @username
        username: String,
        /// Whether the user's account is protected
        #[serde(skip_serializing_if = "Option::is_none")]
        protected: Option<bool>,
        /// The user's affiliation information
        #[serde(skip_serializing_if = "Option::is_none")]
        affiliation: Option<Affiliation>,
        /// The user's connection status
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_status: Option<Vec<ConnectionStatus>>,
        /// When the user's account was created
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
        /// The user's profile description/bio
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Entities found in the user's description (hashtags, mentions, URLs)
        #[serde(skip_serializing_if = "Option::is_none")]
        entities: Option<Entities>,
        /// The user's location
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<String>,
        /// ID of the user's most recent tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        most_recent_tweet_id: Option<String>,
        /// ID of the user's pinned tweet
        #[serde(skip_serializing_if = "Option::is_none")]
        pinned_tweet_id: Option<String>,
        /// URL of the user's profile banner image
        #[serde(skip_serializing_if = "Option::is_none")]
        profile_banner_url: Option<String>,
        /// URL of the user's profile image
        #[serde(skip_serializing_if = "Option::is_none")]
        profile_image_url: Option<String>,
        /// Public metrics about the user (followers, following, tweet count)
        #[serde(skip_serializing_if = "Option::is_none")]
        public_metrics: Option<PublicMetrics>,
        /// Whether the user accepts direct messages
        #[serde(skip_serializing_if = "Option::is_none")]
        receives_your_dm: Option<bool>,
        /// The user's subscription type
        #[serde(skip_serializing_if = "Option::is_none")]
        subscription_type: Option<SubscriptionType>,
        /// The user's website URL
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// Whether the user is verified
        #[serde(skip_serializing_if = "Option::is_none")]
        verified: Option<bool>,
        /// The user's verification type
        #[serde(skip_serializing_if = "Option::is_none")]
        verified_type: Option<VerifiedType>,
        /// Withholding information for the user
        #[serde(skip_serializing_if = "Option::is_none")]
        withheld: Option<Withheld>,

        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
    },
    Err {
        /// Error message if the request failed
        reason: String,
    },
}

pub(crate) struct GetUserByUsername {
    api_base: String,
}

impl NexusTool for GetUserByUsername {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/users/by/username",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-user-by-username@1")
    }

    fn path() -> &'static str {
        "/get-user-by-username"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        let client = Client::new();

        // Format the URL with the username
        let url = format!("{}/{}", self.api_base, request.username);

        // Build query string
        let mut query_parts = Vec::new();

        if let Some(user_fields) = &request.user_fields {
            let fields: Vec<String> = user_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_parts.push(format!("user.fields={}", fields.join(",")));
        }

        if let Some(expansions_fields) = &request.expansions_fields {
            let fields: Vec<String> = expansions_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_parts.push(format!("expansions={}", fields.join(",")));
        }

        if let Some(tweet_fields) = &request.tweet_fields {
            let fields: Vec<String> = tweet_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_parts.push(format!("tweet.fields={}", fields.join(",")));
        }

        // Append query params to URL if any
        let full_url = if !query_parts.is_empty() {
            format!("{}?{}", url, query_parts.join("&"))
        } else {
            url
        };

        // Make the request using centralized error handling
        match self
            .fetch_user(&client, &full_url, &request.bearer_token)
            .await
        {
            Ok(response) => {
                if let Some(user) = response.data {
                    Output::Ok {
                        id: user.id,
                        name: user.name,
                        username: user.username,
                        protected: user.protected,
                        affiliation: user.affiliation,
                        connection_status: user.connection_status,
                        created_at: user.created_at,
                        description: user.description,
                        entities: user.entities,
                        location: user.location,
                        most_recent_tweet_id: user.most_recent_tweet_id,
                        pinned_tweet_id: user.pinned_tweet_id,
                        profile_banner_url: user.profile_banner_url,
                        profile_image_url: user.profile_image_url,
                        public_metrics: user.public_metrics,
                        receives_your_dm: user.receives_your_dm,
                        subscription_type: user.subscription_type,
                        url: user.url,
                        verified: user.verified,
                        verified_type: user.verified_type,
                        withheld: user.withheld,
                        includes: response.includes,
                    }
                } else {
                    Output::Err {
                        reason: "No user data found in the response".to_string(),
                    }
                }
            }
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

impl GetUserByUsername {
    /// Fetch user from Twitter API
    async fn fetch_user(
        &self,
        client: &Client,
        url: &str,
        bearer_token: &str,
    ) -> TwitterResult<UserResponse> {
        let response = client
            .get(url)
            .header("Authorization", format!("Bearer {}", bearer_token))
            .send()
            .await?;

        parse_twitter_response::<UserResponse>(response).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetUserByUsername {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUserByUsername) {
        let server = Server::new_async().await;
        let tool = GetUserByUsername::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            username: "TwitterDev".to_string(),
            user_fields: None,
            expansions_fields: None,
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_user_successful() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;
        // Set up mock response with the complete data as provided in the example
        let mock = server
            .mock("GET", "/TwitterDev")
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "created_at": "2013-12-14T04:35:55Z",
                        "id": "2244994945",
                        "name": "X Dev",
                        "protected": false,
                        "username": "TwitterDev"
                    },
                    "includes": {
                        "users": [
                            {
                                "created_at": "2013-12-14T04:35:55Z",
                                "id": "2244994945",
                                "name": "X Dev",
                                "protected": false,
                                "username": "TwitterDev"
                            }
                        ],
                        "tweets": [
                            {
                                "author_id": "2244994945",
                                "created_at": "Wed Jan 06 18:40:40 +0000 2021",
                                "id": "1346889436626259968",
                                "text": "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2 to explore Tweet\\u2026 https:\\/\\/t.co\\/56a0vZUx7i",
                                "username": "XDevelopers"
                            }
                        ]
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the user request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response based on the models.rs structure
        match output {
            Output::Ok {
                id,
                name,
                username,
                protected,
                created_at,
                ..
            } => {
                assert_eq!(id, "2244994945");
                assert_eq!(name, "X Dev");
                assert_eq!(username, "TwitterDev");
                assert_eq!(protected, Some(false));
                assert_eq!(created_at, Some("2013-12-14T04:35:55Z".to_string()));
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for not found using the error structure provided
        let mock = server
            .mock("GET", "/TwitterDev")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "message": "User not found",
                            "code": 50
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the user request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Err { reason } => {
                // Hata mesajının 'User not found' içerdiğini kontrol et
                assert!(
                    reason.contains("User not found"),
                    "Expected user not found error, got: {}",
                    reason
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_bearer_token() {
        let (mut server, tool) = create_server_and_tool().await;
        let mock = server
            .mock("GET", "/TwitterDev")
            .with_status(401)
            .with_body(
                json!({
                    "errors": [{
                        "message": "Invalid token",
                        "code": 89
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid token"),
                    "Expected invalid token error"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_handling() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/TwitterDev")
            .with_status(429)
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
            Output::Err { reason } => {
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected rate limit error"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_partial_response_handling() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/TwitterDev")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "id": "2244994945",
                        "name": "X Dev",
                        "username": "TwitterDev"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok {
                id,
                name,
                username,
                protected,
                ..
            } => {
                assert_eq!(id, "2244994945");
                assert_eq!(name, "X Dev");
                assert_eq!(username, "TwitterDev");
                assert_eq!(protected, None); // Optional field missing
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }
}
