//! # `xyz.taluslabs.social.twitter.get-user-lists@1`
//!
//! Standard Nexus Tool that retrieves a list of lists for a user.

use {
    super::models::ListData,
    crate::{
        error::{parse_twitter_response, TwitterErrorKind, TwitterResult},
        list::models::{Expansion, Includes, ListField, ListsResponse, Meta, UserField},
        tweet::TWITTER_API_BASE,
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,
    /// The ID of the user to retrieve lists for
    user_id: String,
    /// The maximum number of lists to retrieve
    /// Required range: 5 <= x <= 100
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 5, max = 100))]
    max_results: Option<i32>,

    /// The cursor to use for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(min = 1, max = 19))]
    pagination_token: Option<String>,

    /// A comma separated list of fields to display
    #[serde(rename = "list.fields", skip_serializing_if = "Option::is_none")]
    list_fields: Option<Vec<ListField>>,

    /// A comma separated list of fields to expand  
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions: Option<Vec<Expansion>>,

    /// A comma separated list of User fields to display
    #[serde(rename = "user.fields", skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The list of lists
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Vec<ListData>>,
        /// Additional entities related to the list
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
        /// Metadata about the list request
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
    },
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (network, server, auth, etc.)
        kind: TwitterErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct GetUserLists {
    api_base: String,
}

impl NexusTool for GetUserLists {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/users",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-user-lists@1")
    }

    fn path() -> &'static str {
        "/get-user-lists"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        match self.fetch_user_lists(&request).await {
            Ok(list_response) => {
                if let Some(lists) = list_response.data {
                    Output::Ok {
                        data: Some(lists),
                        includes: list_response.includes,
                        meta: list_response.meta,
                    }
                } else {
                    Output::Err {
                        reason: "No lists found".to_string(),
                        kind: TwitterErrorKind::NotFound,
                        status_code: Some(404),
                    }
                }
            }
            Err(e) => {
                let error_response = e.to_error_response();

                Output::Err {
                    reason: error_response.reason,
                    kind: error_response.kind,
                    status_code: error_response.status_code,
                }
            }
        }
    }
}

impl GetUserLists {
    async fn fetch_user_lists(&self, request: &Input) -> TwitterResult<ListsResponse> {
        // Create a client
        let client = Client::new();

        let url = format!("{}/{}/owned_lists", self.api_base, request.user_id);

        let mut req_builder = client
            .get(url)
            .header("Authorization", format!("Bearer {}", request.bearer_token));

        if let Some(max_results) = request.max_results {
            req_builder = req_builder.query(&[("max_results", max_results.to_string())]);
        }

        if let Some(pagination_token) = &request.pagination_token {
            req_builder = req_builder.query(&[("pagination_token", pagination_token)]);
        }

        // Add optional query parameters if they exist
        if let Some(list_fields) = &request.list_fields {
            let fields: Vec<String> = list_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            req_builder = req_builder.query(&[("list.fields", fields.join(","))]);
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

        // Make the request
        let response = req_builder.send().await?;
        parse_twitter_response::<ListsResponse>(response).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetUserLists {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string() + "/users",
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetUserLists) {
        let server = Server::new_async().await;
        let tool = GetUserLists::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            user_id: "12345".to_string(),
            max_results: Some(10),
            pagination_token: None,
            list_fields: Some(vec![ListField::Name, ListField::Description]),
            expansions: Some(vec![Expansion::OwnerId]),
            user_fields: Some(vec![UserField::Username, UserField::Name]),
        }
    }

    #[tokio::test]
    async fn test_get_user_lists_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/12345/owned_lists")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "list1",
                            "name": "Test List 1",
                            "description": "First test list",
                            "owner_id": "12345"
                        },
                        {
                            "id": "list2",
                            "name": "Test List 2",
                            "description": "Second test list",
                            "owner_id": "12345"
                        }
                    ],
                    "meta": {
                        "result_count": 2
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { data, meta, .. } => {
                assert!(data.is_some());
                let data = data.unwrap();
                assert_eq!(data.len(), 2);
                assert_eq!(data[0].id, "list1");
                assert_eq!(data[0].name, "Test List 1");
                assert_eq!(data[1].id, "list2");
                assert_eq!(data[1].name, "Test List 2");

                assert!(meta.is_some());
                let meta = meta.unwrap();
                assert_eq!(meta.result_count.unwrap(), 2);
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
    async fn test_get_user_lists_not_found() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/12345/owned_lists")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "value": "owned_lists",
                            "detail": "Could not find lists for user with id: [12345].",
                            "title": "Not Found Error",
                            "type": "https://api.twitter.com/2/problems/resource-not-found"
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
                kind,
                status_code,
            } => {
                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::NotFound,
                    "Expected error kind NotFound, got: {:?}",
                    kind
                );

                // Check error message
                assert!(
                    reason.contains("Not Found Error"),
                    "Expected error message to contain 'Not Found Error', got: {}",
                    reason
                );
                assert!(
                    reason.contains("Could not find lists for user with id"),
                    "Expected error message to contain user ID details, got: {}",
                    reason
                );

                // Check status code
                assert_eq!(
                    status_code,
                    Some(404),
                    "Expected status code 404, got: {:?}",
                    status_code
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_user_lists_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/12345/owned_lists")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(401)
            .with_header("content-type", "application/json")
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
                // Check error message
                assert!(
                    reason.contains("Unauthorized"),
                    "Expected error message to contain 'Unauthorized', got: {}",
                    reason
                );

                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::Auth,
                    "Expected error kind Auth, got: {:?}",
                    kind
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
    async fn test_get_user_lists_rate_limit() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/12345/owned_lists")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
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
                // Check error message
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected error message to contain 'Rate limit exceeded', got: {}",
                    reason
                );

                // Check error type
                assert_eq!(
                    kind,
                    TwitterErrorKind::RateLimit,
                    "Expected error kind RateLimit, got: {:?}",
                    kind
                );

                // Check status code
                assert_eq!(
                    status_code,
                    Some(429),
                    "Expected status code 429, got: {:?}",
                    status_code
                )
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_user_lists_empty_response() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/12345/owned_lists")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
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
            Output::Ok { data, meta, .. } => {
                assert!(data.is_some());
                assert!(data.unwrap().is_empty());
                assert!(meta.is_some());
                assert_eq!(meta.unwrap().result_count.unwrap(), 0);
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
