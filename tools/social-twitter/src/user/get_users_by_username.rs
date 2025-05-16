//! # `xyz.taluslabs.social.twitter.get-users-by-username@1`
//!
//! Standard Nexus Tool that retrieves users from the Twitter API by their usernames.

use {
    crate::{
        error::TwitterErrorKind,
        list::models::Includes,
        tweet::models::{ExpansionField, TweetField, UserField},
        twitter_client::{TwitterClient, TWITTER_API_BASE},
        user::models::{UserData, UsersResponse},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,

    /// A list of usernames to retrieve (comma-separated)
    usernames: Vec<String>,

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
        /// Array of user data objects
        users: Vec<UserData>,

        /// Expanded objects referenced in the response
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
    },
    Err {
        /// Type of error (network, server, auth, etc.)
        kind: TwitterErrorKind,
        /// Detailed error message
        reason: String,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct GetUsersByUsername {
    api_base: String,
}

impl NexusTool for GetUsersByUsername {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-users-by-username@1")
    }

    fn path() -> &'static str {
        "/get-users-by-username"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Create a Twitter client with the mock server URL
        let client = match TwitterClient::new(Some("users/by"), Some(&self.api_base)) {
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

        // Add usernames
        query_params.push(("usernames".to_string(), request.usernames.join(",")));

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
            .get::<UsersResponse>(request.bearer_token, Some(query_params))
            .await
        {
            Ok((data, includes, _)) => {
                if data.is_empty() {
                    Output::Err {
                        reason: "No users found".to_string(),
                        kind: TwitterErrorKind::NotFound,
                        status_code: Some(404),
                    }
                } else {
                    Output::Ok {
                        users: data,
                        includes,
                    }
                }
            }
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

    impl GetUsersByUsername {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUsersByUsername) {
        let server = Server::new_async().await;
        let tool = GetUsersByUsername::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            usernames: vec!["TwitterDev".to_string(), "XDevelopers".to_string()],
            user_fields: None,
            expansions_fields: None,
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_users_successful() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response with the complete data as provided in the example
        let mock = server
            .mock("GET", "/users/by")
            .match_query(mockito::Matcher::UrlEncoded(
                "usernames".into(),
                "TwitterDev,XDevelopers".into(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "created_at": "2013-12-14T04:35:55Z",
                            "id": "2244994945",
                            "name": "X Dev",
                            "protected": false,
                            "username": "TwitterDev"
                        },
                        {
                            "created_at": "2021-01-06T18:40:40Z",
                            "id": "123456789",
                            "name": "X Developers",
                            "protected": false,
                            "username": "XDevelopers"
                        }
                    ],
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

        // Test the users request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Ok { users, .. } => {
                assert_eq!(users.len(), 2);
                assert_eq!(users[0].id, "2244994945");
                assert_eq!(users[0].name, "X Dev");
                assert_eq!(users[0].username, "TwitterDev");
                assert_eq!(users[1].id, "123456789");
                assert_eq!(users[1].name, "X Developers");
                assert_eq!(users[1].username, "XDevelopers");
            }
            Output::Err {
                reason,
                kind: _,
                status_code,
            } => panic!(
                "Expected success, got error: {} ({})",
                reason,
                status_code.unwrap_or(0)
            ),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_users_not_found() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for not found using the error structure provided
        let mock = server
            .mock("GET", "/users/by")
            .match_query(mockito::Matcher::UrlEncoded(
                "usernames".into(),
                "TwitterDev,XDevelopers".into(),
            ))
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "message": "Users not found",
                            "code": 50
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the users request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Err {
                reason,
                kind: _,
                status_code,
            } => {
                assert!(
                    reason.contains("Users not found"),
                    "Expected users not found error, got: {} ({})",
                    reason,
                    status_code.unwrap_or(0)
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
            .mock("GET", "/users/by")
            .match_query(mockito::Matcher::UrlEncoded(
                "usernames".into(),
                "TwitterDev,XDevelopers".into(),
            ))
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "message": "Invalid token",
                            "code": 89
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err {
                reason,
                kind: _,
                status_code,
            } => {
                assert!(
                    reason.contains("Invalid token"),
                    "Expected invalid token error, got: {} ({})",
                    reason,
                    status_code.unwrap_or(0)
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
            .mock("GET", "/users/by")
            .match_query(mockito::Matcher::UrlEncoded(
                "usernames".into(),
                "TwitterDev,XDevelopers".into(),
            ))
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "message": "Rate limit exceeded",
                            "code": 88
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err {
                reason,
                kind: _,
                status_code,
            } => {
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected rate limit error, got: {} ({})",
                    reason,
                    status_code.unwrap_or(0)
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_response_handling() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/by")
            .match_query(mockito::Matcher::UrlEncoded(
                "usernames".into(),
                "TwitterDev,XDevelopers".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": []
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
                assert_eq!(kind, TwitterErrorKind::NotFound);
                assert!(reason.contains("No users found"));
                assert_eq!(status_code, Some(404));
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }
}
