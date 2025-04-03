//! # `xyz.taluslabs.social.twitter.get-users-by-username@1`
//!
//! Standard Nexus Tool that retrieves users from the Twitter API by their usernames.

use {
    crate::{
        error::{parse_twitter_response, TwitterResult},
        list::models::Includes,
        tweet::{
            models::{ExpansionField, TweetField, UserField},
            TWITTER_API_BASE,
        },
        user::models::{UserData, UsersResponse},
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
        /// Error message if the request failed
        reason: String,
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
            api_base: TWITTER_API_BASE.to_string() + "/users/by",
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
        match self.fetch_users(&request).await {
            Ok(response) => {
                if let Some(users) = response.data {
                    if users.is_empty() {
                        return Output::Err {
                            reason: "No users found".to_string(),
                        };
                    }

                    Output::Ok {
                        users,
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

impl GetUsersByUsername {
    /// Fetch users from Twitter API by usernames
    async fn fetch_users(&self, request: &Input) -> TwitterResult<UsersResponse> {
        let client = Client::new();

        // Join usernames with commas for the query parameter
        let usernames = request.usernames.join(",");

        // Build request with query parameters
        let mut req_builder = client
            .get(&self.api_base)
            .header("Authorization", format!("Bearer {}", request.bearer_token))
            .query(&[("usernames", usernames)]);

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
        parse_twitter_response::<UsersResponse>(response).await
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
            .mock("GET", "/")
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
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
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
            .mock("GET", "/")
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
            Output::Err { reason } => {
                assert!(
                    reason.contains("Users not found") || reason.contains("50"),
                    "Expected users not found error, got: {}",
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
            .mock("GET", "/")
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
            Output::Err { reason } => {
                assert!(
                    reason.contains("Invalid token") || reason.contains("89"),
                    "Expected invalid token error, got: {}",
                    reason
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
            .mock("GET", "/")
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
            Output::Err { reason } => {
                assert!(
                    reason.contains("Rate limit exceeded") || reason.contains("88"),
                    "Expected rate limit error, got: {}",
                    reason
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
            .mock("GET", "/")
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
            Output::Err { reason } => {
                assert!(
                    reason.contains("No users found"),
                    "Expected empty users error, got: {}",
                    reason
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }
}
