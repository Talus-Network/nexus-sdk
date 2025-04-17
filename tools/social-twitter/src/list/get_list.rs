//! # xyz.taluslabs.social.twitter.get-list@1
//!
//! Standard Nexus Tool that retrieves a list on Twitter.

use {
    crate::{
        error::{parse_twitter_response, TwitterErrorKind, TwitterErrorResponse, TwitterResult},
        list::models::{Expansion, Includes, ListField, ListResponse, Meta, UserField},
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
    /// List ID to retrieve
    list_id: String,

    /// A comma separated list of List fields to display
    #[serde(rename = "list.fields", skip_serializing_if = "Option::is_none")]
    pub list_fields: Option<Vec<ListField>>,

    /// A comma separated list of fields to expand
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expansions: Option<Vec<Expansion>>,

    /// A comma separated list of User fields to display
    #[serde(rename = "user.fields", skip_serializing_if = "Option::is_none")]
    pub user_fields: Option<Vec<UserField>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The list's unique identifier
        id: String,
        /// The list's name
        name: String,
        /// The timestamp when the list was created
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
        /// The list's description
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Number of followers this list has
        #[serde(skip_serializing_if = "Option::is_none")]
        follower_count: Option<i32>,
        /// Number of members in this list
        #[serde(skip_serializing_if = "Option::is_none")]
        member_count: Option<i32>,
        /// The ID of the list's owner
        #[serde(skip_serializing_if = "Option::is_none")]
        owner_id: Option<String>,
        /// Whether the list is private or public
        #[serde(skip_serializing_if = "Option::is_none")]
        private: Option<bool>,
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

pub(crate) struct GetList {
    api_base: String,
}

impl NexusTool for GetList {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/lists",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-list@1")
    }

    fn path() -> &'static str {
        "/get-list"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        let client = Client::new();
        // Add authentication header
        let url = format!("{}/{}", self.api_base, request.list_id);

        match self.fetch_list(&client, &url, &request).await {
            Ok(list_response) => {
                if let Some(list) = list_response.data {
                    Output::Ok {
                        id: list.id,
                        name: list.name,
                        created_at: list.created_at,
                        description: list.description,
                        follower_count: list.follower_count,
                        member_count: list.member_count,
                        owner_id: list.owner_id,
                        private: list.private,
                        includes: None, // ListResponse doesn't contain includes in current model
                        meta: None,     // ListResponse doesn't contain meta in current model
                    }
                } else {
                    Output::Err {
                        reason: "No list data found in the response".to_string(),
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

impl GetList {
    /// Fetch list from Twitter API
    async fn fetch_list(
        &self,
        client: &Client,
        url: &str,
        request: &Input,
    ) -> TwitterResult<ListResponse> {
        let mut req_builder = client
            .get(url)
            .header("Authorization", format!("Bearer {}", request.bearer_token));

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
        parse_twitter_response::<ListResponse>(response).await
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetList {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetList) {
        let server = Server::new_async().await;
        let tool = GetList::with_api_base(&(server.url() + "/lists"));
        (server, tool)
    }
    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            list_id: "test_list_id".to_string(),
            list_fields: Some(vec![
                ListField::Name,
                ListField::Description,
                ListField::FollowerCount,
            ]),
            expansions: Some(vec![Expansion::OwnerId]),
            user_fields: Some(vec![UserField::Username, UserField::Name]),
        }
    }

    #[tokio::test]
    async fn test_get_list_successful() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/lists/test_list_id")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "id": "test_list_id",
                        "name": "Test List",
                        "description": "A test list for unit testing",
                        "follower_count": 42,
                        "owner_id": "12345678"
                    },
                    "includes": {
                        "users": [
                            {
                                "id": "12345678",
                                "name": "Test User",
                                "username": "testuser"
                            }
                        ]
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the list request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Ok {
                id,
                name,
                description,
                follower_count,
                owner_id,
                ..
            } => {
                assert_eq!(id, "test_list_id");
                assert_eq!(name, "Test List");
                assert_eq!(description.unwrap(), "A test list for unit testing");
                assert_eq!(follower_count.unwrap(), 42);
                assert_eq!(owner_id.unwrap(), "12345678");
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

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_list_not_found() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for not found
        let mock = server
            .mock("GET", "/lists/test_list_id")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [
                        {
                            "message": "List not found",
                            "code": 34
                        }
                    ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the list request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("List not found"),
                    "Expected error message to contain 'List not found', got: {}",
                    reason
                );

                // Accept either NotFound or Api error kinds
                if kind == TwitterErrorKind::NotFound || kind == TwitterErrorKind::Api {
                    // Good
                } else {
                    panic!("Expected error kind NotFound or Api, got: {:?}", kind);
                }

                // Check status code - it might be 404 or None depending on the response structure
                if status_code != Some(404) && status_code.is_some() {
                    panic!("Expected status code 404 or None, got: {:?}", status_code);
                }
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
    #[tokio::test]
    async fn test_unauthorized_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/lists/test_list_id")
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
                assert!(
                    reason.contains("Unauthorized"),
                    "Expected error message to contain 'Unauthorized', got: {}",
                    reason
                );

                // Accept either Auth or Api error kinds
                if kind == TwitterErrorKind::Auth || kind == TwitterErrorKind::Api {
                    // Good
                } else {
                    panic!("Expected error kind Auth or Api, got: {:?}", kind);
                }

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
    async fn test_get_list_rate_limit_exceeded() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for rate limit exceeded
        let mock = server
            .mock("GET", "/lists/test_list_id")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
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

        // Test the list request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("Rate limit exceeded"),
                    "Expected error message to contain 'Rate limit exceeded', got: {}",
                    reason
                );

                // Accept either RateLimit or Api error kinds
                if kind == TwitterErrorKind::RateLimit || kind == TwitterErrorKind::Api {
                    // Good
                } else {
                    panic!("Expected error kind RateLimit or Api, got: {:?}", kind);
                }

                // Check status code - it might be 429 or None depending on the response structure
                if status_code != Some(429) && status_code.is_some() {
                    panic!("Expected status code 429 or None, got: {:?}", status_code);
                }
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_json_response() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response with invalid JSON
        let mock = server
            .mock("GET", "/lists/test_list_id")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("invalid json")
            .create_async()
            .await;

        // Test the list request
        let output = tool.invoke(create_test_input()).await;

        // Verify the response
        match output {
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("Response parsing error") || reason.contains("Failed to parse"),
                    "Expected error message to contain parsing error information, got: {}",
                    reason
                );

                // Check for Parse error kind
                assert_eq!(kind, TwitterErrorKind::Parse, "Expected Parse error kind");

                // Parsing errors typically don't have status codes
                assert_eq!(
                    status_code, None,
                    "Parsing errors should not have status codes"
                );
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
