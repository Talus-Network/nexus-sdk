//! # `xyz.taluslabs.social.twitter.get-users-by-id@1`
//!
//! Standard Nexus Tool that retrieves multiple users by their IDs.

use {
    crate::{
        error::TwitterErrorKind,
        list::models::Includes,
        tweet::models::{ExpansionField, TweetField, UserField},
        twitter_client::{TwitterClient, TWITTER_API_BASE},
        user::models::{UserData, UsersResponse},
    },
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

    /// A list of User IDs to lookup (up to 100)
    /// Example: "2244994945,6253282,12"
    ids: Vec<String>,

    /// A comma separated list of User fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,

    /// A comma separated list of fields to expand
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions: Option<Vec<ExpansionField>>,

    /// A comma separated list of Tweet fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    tweet_fields: Option<Vec<TweetField>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// List of users retrieved from the API
        users: Vec<UserData>,
        /// Includes data such as referenced tweets and users
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

impl GetUsersById {
    fn add_fields_param<T: Serialize>(
        &self,
        params: &mut Vec<(String, String)>,
        param_name: &str,
        fields: &Option<Vec<T>>,
    ) {
        if let Some(field_values) = fields {
            if !field_values.is_empty() {
                let formatted_fields: Vec<String> = field_values
                    .iter()
                    .map(|f| {
                        serde_json::to_string(f)
                            .unwrap_or_default()
                            .replace("\"", "")
                            .to_lowercase()
                    })
                    .collect();

                params.push((param_name.to_string(), formatted_fields.join(",")));
            }
        }
    }
}

pub(crate) struct GetUsersById {
    api_base: String,
}

impl NexusTool for GetUsersById {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-users-by-id@1")
    }

    fn path() -> &'static str {
        "/get-users-by-id"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Create a Twitter client with the mock server URL
        let client = match TwitterClient::new(Some("users"), Some(&self.api_base)) {
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

        // Add users id
        query_params.push(("ids".to_string(), request.ids.join(",")));

        // Use the add_fields_param method for all field parameters
        self.add_fields_param(&mut query_params, "user.fields", &request.user_fields);
        self.add_fields_param(&mut query_params, "expansions", &request.expansions);
        self.add_fields_param(&mut query_params, "tweet.fields", &request.tweet_fields);

        match client
            .get::<UsersResponse>(request.bearer_token, Some(query_params))
            .await
        {
            Ok(response) => {
                return Output::Ok {
                    users: response.0,
                    includes: response.1,
                }
            }
            Err(e) => {
                return Output::Err {
                    reason: e.reason,
                    kind: e.kind,
                    status_code: e.status_code,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ::mockito::{self, Server},
        mockito::Matcher,
        serde_json::json,
    };

    impl GetUsersById {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUsersById) {
        let server = Server::new_async().await;
        let tool = GetUsersById::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            ids: vec!["2244994945".to_string(), "6253282".to_string()],
            user_fields: None,
            expansions: None,
            tweet_fields: None,
        }
    }

    fn create_test_input_with_user_fields() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            ids: vec!["2244994945".to_string()],
            user_fields: Some(vec![
                UserField::Description,
                UserField::CreatedAt,
                UserField::PublicMetrics,
            ]),
            expansions: None,
            tweet_fields: None,
        }
    }

    fn create_test_input_with_expansions() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            ids: vec!["2244994945".to_string()],
            user_fields: None,
            expansions: Some(vec![ExpansionField::AuthorId]),
            tweet_fields: Some(vec![TweetField::AuthorId, TweetField::CreatedAt]),
        }
    }

    fn create_test_input_single_id() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            ids: vec!["2244994945".to_string()],
            user_fields: None,
            expansions: None,
            tweet_fields: None,
        }
    }

    #[tokio::test]
    async fn test_get_users_by_id_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input();
        let query_params = vec![("ids".to_string(), input.ids.join(","))];

        let mock = server
            .mock("GET", "/users")
            .match_query(Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "2244994945",
                            "name": "X Dev",
                            "username": "TwitterDev",
                            "created_at": "2013-12-14T04:35:55Z",
                            "protected": false
                        },
                        {
                            "id": "6253282",
                            "name": "X API",
                            "username": "TwitterAPI",
                            "created_at": "2007-05-23T06:01:13Z",
                            "protected": false
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { users, .. } => {
                assert_eq!(users.len(), 2);
                assert_eq!(users[0].id, "2244994945");
                assert_eq!(users[0].name, "X Dev");
                assert_eq!(users[0].username, "TwitterDev");
                assert_eq!(users[1].id, "6253282");
                assert_eq!(users[1].name, "X API");
                assert_eq!(users[1].username, "TwitterAPI");
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
    async fn test_get_users_with_user_fields() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input_with_user_fields();
        let query_params = vec![
            ("ids".to_string(), input.ids.join(",")),
            (
                "user.fields".to_string(),
                vec!["description", "created_at", "public_metrics"].join(","),
            ),
        ];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "2244994945",
                            "name": "X Dev",
                            "username": "TwitterDev",
                            "created_at": "2013-12-14T04:35:55Z",
                            "description": "The official account for the X Platform team, providing updates and news about our API and developer products.",
                            "public_metrics": {
                                "followers_count": 513868,
                                "following_count": 2018,
                                "tweet_count": 3961,
                                "listed_count": 1672
                            }
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { users, .. } => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].id, "2244994945");
                assert_eq!(users[0].name, "X Dev");
                assert!(users[0].description.is_some());
                assert!(users[0].public_metrics.is_some());
                if let Some(metrics) = &users[0].public_metrics {
                    assert_eq!(metrics.followers_count, 513868);
                    assert_eq!(metrics.tweet_count, 3961);
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
    async fn test_get_users_with_expansions() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input_with_expansions();
        let query_params = vec![
            ("ids".to_string(), input.ids.join(",")),
            ("expansions".to_string(), vec!["author_id"].join(",")),
            (
                "tweet.fields".to_string(),
                vec!["author_id", "created_at"].join(","),
            ),
        ];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "2244994945",
                            "name": "X Dev",
                            "username": "TwitterDev"
                        }
                    ],
                    "includes": {
                        "tweets": [
                            {
                                "id": "1354143047324299264",
                                "text": "Academics are one of the biggest users of the Twitter API",
                                "author_id": "2244994945",
                                "created_at": "2021-01-26T19:31:58.000Z"
                            }
                        ]
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { users, includes } => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].id, "2244994945");

                assert!(includes.is_some());
                if let Some(inc) = includes {
                    assert!(inc.tweets.is_some());
                    if let Some(tweets) = inc.tweets {
                        assert_eq!(tweets[0].id, "1354143047324299264");
                        assert_eq!(tweets[0].author_id, Some("2244994945".to_string()));
                    }
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
    async fn test_empty_results() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input();
        let query_params = vec![("ids".to_string(), input.ids.join(","))];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
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

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { users, .. } => {
                assert_eq!(users.len(), 0);
            }
            Output::Err {
                reason,
                kind: _,
                status_code,
            } => panic!(
                "Expected empty success, got error: {} ({})",
                reason,
                status_code.unwrap_or(0)
            ),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_single_user_id() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input_single_id();
        let query_params = vec![("ids".to_string(), input.ids.join(","))];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "2244994945",
                            "name": "X Dev",
                            "username": "TwitterDev"
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { users, .. } => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].id, "2244994945");
                assert_eq!(users[0].name, "X Dev");
                assert_eq!(users[0].username, "TwitterDev");
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
    async fn test_rate_limit_exceeded() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input();
        let query_params = vec![("ids".to_string(), input.ids.join(","))];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
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

        let output = tool.invoke(input).await;

        match output {
            Output::Err {
                reason,
                kind: _,
                status_code: _,
            } => {
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected rate limit error"
                );
            }
            Output::Ok { .. } => panic!("Expected rate limit error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_users_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input();
        let query_params = vec![("ids".to_string(), input.ids.join(","))];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "message": "Users not found",
                        "code": 50
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Err {
                reason,
                kind: _,
                status_code: _,
            } => {
                assert!(
                    reason.contains("Users not found"),
                    "Expected users not found error"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_bearer_token() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input();
        let query_params = vec![("ids".to_string(), input.ids.join(","))];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "status": 401,
                    "title": "Unauthorized",
                    "type": "https://api.twitter.com/2/problems/unauthorized",
                    "detail": "Unauthorized"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::Auth,
                    "Expected error kind Auth, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Unauthorized"),
                    "Expected error message to contain 'Unauthorized', got: {}",
                    reason
                );

                // Check status code
                assert_eq!(
                    status_code,
                    Some(401),
                    "Expected status code 401, got: {:?}",
                    status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_with_includes() {
        let (mut server, tool) = create_server_and_tool().await;

        let input = create_test_input();
        let query_params = vec![("ids".to_string(), input.ids.join(","))];

        let mock = server
            .mock("GET", "/users")
            .match_query(mockito::Matcher::AllOf(
                query_params
                    .iter()
                    .map(|(k, v)| Matcher::UrlEncoded(k.clone(), v.clone()))
                    .collect(),
            ))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "2244994945",
                            "name": "X Dev",
                            "username": "TwitterDev",
                            "protected": false
                        }
                    ],
                    "includes": {
                        "tweets": [
                            {
                                "author_id": "2244994945",
                                "created_at": "Wed Jan 06 18:40:40 +0000 2021",
                                "id": "1346889436626259968",
                                "text": "Learn how to use the user Tweet timeline"
                            }
                        ]
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { users, includes } => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].id, "2244994945");
                assert!(includes.is_some(), "Expected includes to be present");
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
