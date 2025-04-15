//! # `xyz.taluslabs.social.twitter.get-user-by-id@1`
//!
//! Standard Nexus Tool that retrieves a user by their ID.

use {
    crate::{
        error::{parse_twitter_response, TwitterErrorKind, TwitterErrorResponse, TwitterResult},
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

    /// The ID of the User to lookup
    /// Example: "2244994945"
    user_id: String,

    /// A comma separated list of User fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,

    /// A comma separated list of fields to expand
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions_fields: Option<Vec<ExpansionField>>,

    /// A comma separated list of fields to display
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
    },
    Err {
        /// Error message if the user lookup failed
        reason: String,
        /// Error kind
        kind: TwitterErrorKind,
        /// Status code
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct GetUserById {
    api_base: String,
}

impl NexusTool for GetUserById {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/users",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-user-by-id@1")
    }

    fn path() -> &'static str {
        "/get-user-by-id"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        match self.fetch_user(&request).await {
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
                    }
                } else {
                    Output::Err {
                        reason: "No user data found in the response".to_string(),
                        kind: TwitterErrorKind::NotFound,
                        status_code: None,
                    }
                }
            }
            Err(e) => {
                let error_response: TwitterErrorResponse = e.to_error_response();

                Output::Err {
                    reason: error_response.reason,
                    kind: error_response.kind,
                    status_code: error_response.status_code,
                }
            }
        }
    }
}

impl GetUserById {
    /// Fetch user from Twitter API
    async fn fetch_user(&self, request: &Input) -> TwitterResult<UserResponse> {
        let client = Client::new();

        // Construct URL with user ID
        let url = format!("{}/{}", self.api_base, request.user_id);

        // Build request with query parameters
        let mut req_builder = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", request.bearer_token));

        // Add optional query parameters
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
            req_builder = req_builder.query(&[("user.fields", fields.join(","))]);
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
            req_builder = req_builder.query(&[("expansions", fields.join(","))]);
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
            req_builder = req_builder.query(&[("tweet.fields", fields.join(","))]);
        }

        // Send the request and parse the response
        let response = req_builder.send().await?;
        parse_twitter_response::<UserResponse>(response).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetUserById {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string() + "/users",
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUserById) {
        let server = Server::new_async().await;
        let tool = GetUserById::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            user_id: "2244994945".to_string(),
            user_fields: None,
            expansions_fields: None,
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_user_by_id_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945")
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "id": "2244994945",
                        "name": "X Dev",
                        "username": "TwitterDev",
                        "protected": false
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok {
                id, name, username, ..
            } => {
                assert_eq!(
                    id, "2244994945",
                    "Expected user ID to be '2244994945', got: {}",
                    id
                );
                assert_eq!(name, "X Dev", "Expected name to be 'X Dev', got: {}", name);
                assert_eq!(
                    username, "TwitterDev",
                    "Expected username to be 'TwitterDev', got: {}",
                    username
                );
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
    async fn test_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945")
            .with_status(404)
            .with_body(
                json!({
                    "errors": [{
                        "message": "User not found",
                        "code": 50
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
                // Accept either NotFound or Api error kinds
                if kind == TwitterErrorKind::NotFound || kind == TwitterErrorKind::Api {
                    println!("  Error kind is acceptable: {:?}", kind);
                } else {
                    panic!("Expected error kind NotFound or Api, got: {:?}", kind);
                }

                // Check error message
                assert!(
                    reason.contains("User not found") || reason.contains("Not Found"),
                    "Expected error message to contain 'User not found' or 'Not Found', got: {}",
                    reason
                );

                // Check status code - it might be 404 or None depending on the response structure
                if status_code != Some(404) && status_code.is_some() {
                    panic!("Expected status code 404 or None, got: {:?}", status_code);
                }
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945")
            .with_status(401)
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
                // Accept either Auth or Api error kinds
                if kind == TwitterErrorKind::Auth || kind == TwitterErrorKind::Api {
                    println!("  Error kind is acceptable: {:?}", kind);
                } else {
                    panic!("Expected error kind Auth or Api, got: {:?}", kind);
                }

                // Check error message
                assert!(
                    reason.contains("Unauthorized") || reason.contains("Authentication"),
                    "Expected error message to contain 'Unauthorized' or 'Authentication', got: {}",
                    reason
                );

                // Check status code - it might be 401 or None depending on the response structure
                if status_code != Some(401) && status_code.is_some() {
                    panic!("Expected status code 401 or None, got: {:?}", status_code);
                }
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945")
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                // Accept either RateLimit or Api error kinds
                if kind == TwitterErrorKind::RateLimit || kind == TwitterErrorKind::Api {
                    println!("  Error kind is acceptable: {:?}", kind);
                } else {
                    panic!("Expected error kind RateLimit or Api, got: {:?}", kind);
                }

                // Check error message
                assert!(
                    reason.contains("Rate limit") || reason.contains("rate limit"),
                    "Expected error message to contain 'Rate limit' or 'rate limit', got: {}",
                    reason
                );

                // Check status code - it might be 429 or None depending on the response structure
                if status_code != Some(429) && status_code.is_some() {
                    panic!("Expected status code 429 or None, got: {:?}", status_code);
                }
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_partial_response_handling() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945")
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
                id, name, username, ..
            } => {
                assert_eq!(
                    id, "2244994945",
                    "Expected user ID to be '2244994945', got: {}",
                    id
                );
                assert_eq!(name, "X Dev", "Expected name to be 'X Dev', got: {}", name);
                assert_eq!(
                    username, "TwitterDev",
                    "Expected username to be 'TwitterDev', got: {}",
                    username
                );
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
}
