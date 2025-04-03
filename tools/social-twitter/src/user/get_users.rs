//! # `xyz.taluslabs.social.twitter.get-users@1`
//!
//! Standard Nexus Tool that retrieves multiple users by their IDs.

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
        /// Error message if the request failed
        reason: String,
    },
}

pub(crate) struct GetUsers {
    api_base: String,
}

impl NexusTool for GetUsers {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/users",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-users@1")
    }

    fn path() -> &'static str {
        "/get-users"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        match self.fetch_users(&request).await {
            Ok(response) => {
                if let Some(users) = response.data {
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

impl GetUsers {
    /// Fetch users from Twitter API
    async fn fetch_users(&self, request: &Input) -> TwitterResult<UsersResponse> {
        let client = Client::new();

        // Convert the Vec<String> of IDs to a comma-separated string
        let ids = request.ids.join(",");

        // Build request with query parameters
        let mut req_builder = client
            .get(&self.api_base)
            .header("Authorization", format!("Bearer {}", request.bearer_token))
            .query(&[("ids", &ids)]);

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

        if let Some(expansions) = &request.expansions {
            let fields: Vec<String> = expansions
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

    impl GetUsers {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUsers) {
        let server = Server::new_async().await;
        let tool = GetUsers::with_api_base(&server.url());
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
    async fn test_get_users_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "ids".into(),
                "2244994945,6253282".into(),
            )]))
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

        let output = tool.invoke(create_test_input()).await;

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
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_users_with_user_fields() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ids".into(), "2244994945".into()),
                mockito::Matcher::UrlEncoded("user.fields".into(), "description,created_at,public_metrics".into()),
            ]))
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

        let output = tool.invoke(create_test_input_with_user_fields()).await;

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
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_users_with_expansions() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("ids".into(), "2244994945".into()),
                mockito::Matcher::UrlEncoded("expansions".into(), "author_id".into()),
                mockito::Matcher::UrlEncoded("tweet.fields".into(), "author_id,created_at".into()),
            ]))
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

        let output = tool.invoke(create_test_input_with_expansions()).await;

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
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_results() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "ids".into(),
                "2244994945,6253282".into(),
            )]))
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

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { users, .. } => {
                assert_eq!(users.len(), 0);
            }
            Output::Err { reason } => panic!("Expected empty success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_single_user_id() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "ids".into(),
                "2244994945".into(),
            )]))
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

        let output = tool.invoke(create_test_input_single_id()).await;

        match output {
            Output::Ok { users, .. } => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].id, "2244994945");
                assert_eq!(users[0].name, "X Dev");
                assert_eq!(users[0].username, "TwitterDev");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "ids".into(),
                "2244994945,6253282".into(),
            )]))
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

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
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

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "ids".into(),
                "2244994945,6253282".into(),
            )]))
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

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
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

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "ids".into(),
                "2244994945,6253282".into(),
            )]))
            .match_header("Authorization", "Bearer test_bearer_token")
            .with_status(401)
            .with_header("content-type", "application/json")
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
    async fn test_with_includes() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "ids".into(),
                "2244994945,6253282".into(),
            )]))
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

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { users, includes } => {
                assert_eq!(users.len(), 1);
                assert_eq!(users[0].id, "2244994945");
                assert!(includes.is_some(), "Expected includes to be present");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }
}
