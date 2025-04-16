//! # `xyz.taluslabs.social.twitter.get-user-followers@1`
//!
//! Standard Nexus Tool that retrieves followers of a user by their ID.

use {
    crate::{
        error::{parse_twitter_response, TwitterErrorKind, TwitterResult},
        list::models::Meta,
        tweet::{
            models::{ExpansionField, TweetField, UserField},
            TWITTER_API_BASE,
        },
        user::models::UsersResponse,
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,

    /// The ID of the User to lookup.
    /// Example: "2244994945"
    user_id: String,

    /// The maximum number of results to return per page
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 100))]
    max_results: Option<i32>,

    /// This parameter is used to get a specified 'page' of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination_token: Option<String>,

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
        /// Array of user data for followers
        followers: Vec<UserData>,

        /// Pagination metadata
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
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

/// User data structure for followers
#[derive(Serialize, JsonSchema)]
pub(crate) struct UserData {
    /// The user's unique identifier
    id: String,
    /// The user's display name
    name: String,
    /// The user's @username
    username: String,
    /// Whether the user's account is protected
    #[serde(skip_serializing_if = "Option::is_none")]
    protected: Option<bool>,
    /// When the user's account was created
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    /// The user's profile description/bio
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// The user's location
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    /// URL of the user's profile image
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_image_url: Option<String>,
    /// Whether the user is verified
    #[serde(skip_serializing_if = "Option::is_none")]
    verified: Option<bool>,
}

pub(crate) struct GetUserFollowers {
    api_base: String,
}

impl NexusTool for GetUserFollowers {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/users",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-user-followers@1")
    }

    fn path() -> &'static str {
        "/get-user-followers"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        match self.fetch_followers(&request).await {
            Ok(response) => {
                if let Some(users) = response.data {
                    let followers = users
                        .into_iter()
                        .map(|user| UserData {
                            id: user.id,
                            name: user.name,
                            username: user.username,
                            protected: user.protected,
                            created_at: user.created_at,
                            description: user.description,
                            location: user.location,
                            profile_image_url: user.profile_image_url,
                            verified: user.verified,
                        })
                        .collect();

                    Output::Ok {
                        followers,
                        meta: response.meta,
                    }
                } else {
                    Output::Err {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No followers found".to_string(),
                        status_code: None,
                    }
                }
            }
            Err(e) => {
                let error_response = e.to_error_response();

                Output::Err {
                    kind: error_response.kind,
                    reason: error_response.reason,
                    status_code: error_response.status_code,
                }
            }
        }
    }
}

impl GetUserFollowers {
    /// Fetch followers from Twitter API
    async fn fetch_followers(&self, request: &Input) -> TwitterResult<UsersResponse> {
        let client = Client::new();

        // Construct URL with user ID
        let url = format!("{}/{}/followers", self.api_base, request.user_id);

        // Build request with query parameters
        let mut req_builder = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", request.bearer_token));

        // Add max_results parameter if provided
        if let Some(max_results) = request.max_results {
            req_builder = req_builder.query(&[("max_results", max_results.to_string())]);
        }

        // Add pagination_token parameter if provided
        if let Some(ref pagination_token) = request.pagination_token {
            req_builder = req_builder.query(&[("pagination_token", pagination_token)]);
        }

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

    impl GetUserFollowers {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUserFollowers) {
        let server = Server::new_async().await;
        let tool = GetUserFollowers::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            user_id: "2244994945".to_string(),
            max_results: Some(10),
            pagination_token: None,
            user_fields: None,
            expansions_fields: None,
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_followers_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/2244994945/followers")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::UrlEncoded(
                "max_results".into(),
                "10".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "1234567890",
                            "name": "Follower 1",
                            "username": "follower1",
                            "protected": false
                        },
                        {
                            "id": "0987654321",
                            "name": "Follower 2",
                            "username": "follower2",
                            "protected": true
                        }
                    ],
                    "meta": {
                        "result_count": 2,
                        "next_token": "next_cursor_value"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok {
                followers, meta, ..
            } => {
                assert_eq!(followers.len(), 2);
                assert_eq!(followers[0].id, "1234567890");
                assert_eq!(followers[0].name, "Follower 1");
                assert_eq!(followers[0].username, "follower1");
                assert_eq!(followers[0].protected, Some(false));

                assert_eq!(followers[1].id, "0987654321");
                assert_eq!(followers[1].name, "Follower 2");
                assert_eq!(followers[1].username, "follower2");
                assert_eq!(followers[1].protected, Some(true));

                assert!(meta.is_some());
                if let Some(meta_data) = meta {
                    assert_eq!(meta_data.result_count, Some(2));
                    assert_eq!(meta_data.next_token, Some("next_cursor_value".to_string()));
                }
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

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_user_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/2244994945/followers")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::UrlEncoded(
                "max_results".into(),
                "10".into(),
            ))
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "message": "User not found",
                        "code": 34
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
                kind: _,
                status_code,
            } => {
                assert!(
                    reason.contains("User not found"),
                    "Expected user not found error"
                );
                assert_eq!(status_code, Some(404));
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pagination() {
        let (mut server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.pagination_token = Some("test_pagination_token".to_string());

        let mock = server
            .mock("GET", "/2244994945/followers")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("max_results".into(), "10".into()),
                mockito::Matcher::UrlEncoded(
                    "pagination_token".into(),
                    "test_pagination_token".into(),
                ),
            ]))
            .with_status(200)
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "9876543210",
                            "name": "Follower 3",
                            "username": "follower3"
                        }
                    ],
                    "meta": {
                        "result_count": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { followers, .. } => {
                assert_eq!(followers.len(), 1);
                assert_eq!(followers[0].id, "9876543210");
                assert_eq!(followers[0].name, "Follower 3");
                assert_eq!(followers[0].username, "follower3");
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
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_handling() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/2244994945/followers")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::UrlEncoded(
                "max_results".into(),
                "10".into(),
            ))
            .with_status(429)
            .with_header("content-type", "application/json")
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
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::RateLimit,
                    "Expected error kind RateLimit, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected error message to contain 'Rate limit exceeded', got: {}",
                    reason
                );

                // Check status code
                assert_eq!(
                    status_code,
                    Some(429),
                    "Expected status code 429, got: {:?}",
                    status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_followers() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/2244994945/followers")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::UrlEncoded(
                "max_results".into(),
                "10".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [],
                    "meta": {
                        "result_count": 0
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok {
                followers, meta, ..
            } => {
                assert_eq!(followers.len(), 0);
                assert!(meta.is_some());
                if let Some(meta_data) = meta {
                    assert_eq!(meta_data.result_count, Some(0));
                }
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
        mock.assert_async().await;
    }
}
