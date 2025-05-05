//! # `xyz.taluslabs.social.twitter.get-user-by-id@1`
//!
//! Standard Nexus Tool that retrieves a user by their ID.

use {
    crate::{
        error::TwitterErrorKind,
        tweet::models::{ExpansionField, TweetField, UserField},
        twitter_client::{TwitterClient, TWITTER_API_BASE},
        user::models::{
            Affiliation, ConnectionStatus, Entities, PublicMetrics, SubscriptionType, UserResponse,
            VerifiedType, Withheld,
        },
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json,
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
            api_base: TWITTER_API_BASE.to_string(),
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
        // Build the endpoint for the Twitter API
        let suffix = format!("users/{}", request.user_id);

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

        // Build query parameters
        let mut query_params = Vec::new();

        // Add user fields if provided
        if let Some(user_fields) = request.user_fields {
            let fields: Vec<String> = user_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("user.fields".to_string(), fields.join(",")));
        }

        // Add expansions if provided
        if let Some(expansions) = request.expansions_fields {
            let fields: Vec<String> = expansions
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("expansions".to_string(), fields.join(",")));
        }

        // Add tweet fields if provided
        if let Some(tweet_fields) = request.tweet_fields {
            let fields: Vec<String> = tweet_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            query_params.push(("tweet.fields".to_string(), fields.join(",")));
        }

        match client
            .get::<UserResponse>(request.bearer_token, Some(query_params))
            .await
        {
            Ok(data) => Output::Ok {
                id: data.0.id,
                name: data.0.name,
                username: data.0.username,
                protected: data.0.protected,
                affiliation: data.0.affiliation,
                connection_status: data.0.connection_status,
                created_at: data.0.created_at,
                description: data.0.description,
                entities: data.0.entities,
                location: data.0.location,
                most_recent_tweet_id: data.0.most_recent_tweet_id,
                pinned_tweet_id: data.0.pinned_tweet_id,
                profile_banner_url: data.0.profile_banner_url,
                profile_image_url: data.0.profile_image_url,
                public_metrics: data.0.public_metrics,
                receives_your_dm: data.0.receives_your_dm,
                subscription_type: data.0.subscription_type,
                url: data.0.url,
                verified: data.0.verified,
                verified_type: data.0.verified_type,
                withheld: data.0.withheld,
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
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetUserById {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
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
