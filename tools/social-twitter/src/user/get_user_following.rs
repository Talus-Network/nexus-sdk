//! # `xyz.taluslabs.social.twitter.get-user-following@1`
//!
//! Standard Nexus Tool that retrieves the users being followed by a specified user ID.

use {
    crate::{
        error::{parse_twitter_response, TwitterErrorKind, TwitterResult},
        list::models::{Includes, Meta},
        tweet::{
            models::{ExpansionField, TweetField, UserField},
            TWITTER_API_BASE,
        },
        user::models::{UserData, UsersResponse},
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

    /// The ID of the User to lookup their following
    /// Example: "2244994945"
    user_id: String,

    /// The maximum number of results to return per page
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<i32>,

    /// Token for pagination to get the next page of results
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
        /// List of users that the specified user is following
        users: Vec<UserData>,

        /// Expansion objects like tweets, users, etc.
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,

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

pub(crate) struct GetUserFollowing {
    api_base: String,
}

impl NexusTool for GetUserFollowing {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/users",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-user-following@1")
    }

    fn path() -> &'static str {
        "/get-user-following"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        match self.fetch_following(&request).await {
            Ok(response) => {
                if let Some(users) = response.data {
                    Output::Ok {
                        users,
                        includes: response.includes,
                        meta: response.meta,
                    }
                } else {
                    Output::Err {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No following data found in the response".to_string(),
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

impl GetUserFollowing {
    /// Fetch users followed by the specified user ID from Twitter API
    async fn fetch_following(&self, request: &Input) -> TwitterResult<UsersResponse> {
        let client = Client::new();

        // Construct URL with user ID and following endpoint
        let url = format!("{}/{}/following", self.api_base, request.user_id);

        // Build request with query parameters
        let mut req_builder = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", request.bearer_token));

        // Add pagination token if provided
        if let Some(token) = &request.pagination_token {
            req_builder = req_builder.query(&[("pagination_token", token)]);
        }

        // Add max results if provided
        if let Some(max_results) = &request.max_results {
            req_builder = req_builder.query(&[("max_results", max_results.to_string())]);
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

    impl GetUserFollowing {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string() + "/users",
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUserFollowing) {
        let server = Server::new_async().await;
        let tool = GetUserFollowing::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            user_id: "2244994945".to_string(),
            max_results: None,
            pagination_token: None,
            user_fields: None,
            expansions_fields: None,
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_following_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/following")
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "6253282",
                            "name": "X API",
                            "username": "XApi"
                        },
                        {
                            "id": "2244994945",
                            "name": "X Dev",
                            "username": "TwitterDev"
                        }
                    ],
                    "meta": {
                        "result_count": 2,
                        "next_token": "NEXT_TOKEN"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { users, meta, .. } => {
                assert_eq!(users.len(), 2);
                assert_eq!(users[0].id, "6253282");
                assert_eq!(users[0].name, "X API");
                assert_eq!(users[0].username, "XApi");
                assert_eq!(users[1].id, "2244994945");
                assert!(meta.is_some());
                if let Some(m) = meta {
                    assert_eq!(m.result_count, Some(2));
                    assert_eq!(m.next_token, Some("NEXT_TOKEN".to_string()));
                }
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} : {:?} : {:?}",
                reason, kind, status_code
            ),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_following_with_pagination() {
        let (mut server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.pagination_token = Some("PAGINATION_TOKEN".to_string());
        input.max_results = Some(5);

        let mock = server
            .mock("GET", "/users/2244994945/following")
            .match_query("pagination_token=PAGINATION_TOKEN&max_results=5")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "783214",
                            "name": "X",
                            "username": "X"
                        }
                    ],
                    "meta": {
                        "result_count": 1,
                        "previous_token": "PAGINATION_TOKEN"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { users, meta, .. } => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].id, "783214");
                assert_eq!(users[0].name, "X");
                assert!(meta.is_some());
                if let Some(m) = meta {
                    assert_eq!(m.result_count, Some(1));
                    assert_eq!(m.previous_token, Some("PAGINATION_TOKEN".to_string()));
                }
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} : {:?} : {:?}",
                reason, kind, status_code
            ),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_following_list() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/following")
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
            Output::Ok { users, meta, .. } => {
                assert!(users.is_empty());
                assert!(meta.is_some());
                if let Some(m) = meta {
                    assert_eq!(m.result_count, Some(0));
                }
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} : {:?} : {:?}",
                reason, kind, status_code
            ),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_user_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/following")
            .with_status(404)
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
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("User not found"),
                    "Expected user not found error"
                );
                assert_eq!(kind, TwitterErrorKind::NotFound);
                assert_eq!(status_code, Some(404));
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unauthorized_access() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/following")
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
                assert!(
                    reason.contains("Unauthorized"),
                    "Expected unauthorized error"
                );
                assert_eq!(kind, TwitterErrorKind::Auth);
                assert_eq!(status_code, Some(401));
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_handling() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/following")
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
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected rate limit error"
                );
                assert_eq!(kind, TwitterErrorKind::RateLimit);
                assert_eq!(status_code, Some(429));
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }
}
