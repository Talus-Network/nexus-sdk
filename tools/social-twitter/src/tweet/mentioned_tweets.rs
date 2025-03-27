//! # `xyz.taluslabs.social.twitter.mentioned-tweets@1`
//!
//! Standard Nexus Tool that retrieves tweets mentioning a specific user.

use {
    crate::tweet::{
        models::{
            ExpansionField,
            MediaField,
            PlaceField,
            PollField,
            TweetField,
            TweetsResponse,
            UserField,
        },
        TWITTER_API_BASE,
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
    id: String,

    /// The minimum Post ID to be included in the result set
    /// Takes precedence over start_time if both are specified
    #[serde(skip_serializing_if = "Option::is_none")]
    since_id: Option<String>,

    /// The maximum Post ID to be included in the result set
    /// Takes precedence over end_time if both are specified
    /// Example: "1346889436626259968"
    #[serde(skip_serializing_if = "Option::is_none")]
    until_id: Option<String>,

    /// The maximum number of results
    /// Required range: 5 <= x <= 100
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 5, max = 100))]
    max_results: Option<i32>,

    /// This parameter is used to get the next 'page' of results
    /// Minimum length: 1
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(min = 1))]
    pagination_token: Option<String>,

    /// YYYY-MM-DDTHH:mm:ssZ. The earliest UTC timestamp from which the Posts will be provided
    /// The since_id parameter takes precedence if it is also specified
    /// Example: "2021-02-01T18:40:40.000Z"
    #[serde(skip_serializing_if = "Option::is_none")]
    start_time: Option<String>,

    /// YYYY-MM-DDTHH:mm:ssZ. The latest UTC timestamp to which the Posts will be provided
    /// The until_id parameter takes precedence if it is also specified
    /// Example: "2021-02-14T18:40:40.000Z"
    #[serde(skip_serializing_if = "Option::is_none")]
    end_time: Option<String>,

    /// A comma separated list of Tweet fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    tweet_fields: Option<Vec<TweetField>>,

    /// A comma separated list of fields to expand
    #[serde(skip_serializing_if = "Option::is_none")]
    expansions: Option<Vec<ExpansionField>>,

    /// A comma separated list of Media fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    media_fields: Option<Vec<MediaField>>,

    /// A comma separated list of Poll fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    poll_fields: Option<Vec<PollField>>,

    /// A comma separated list of User fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    user_fields: Option<Vec<UserField>>,

    /// A comma separated list of Place fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    place_fields: Option<Vec<PlaceField>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The successful tweet response data
        result: TweetsResponse,
    },
    Err {
        /// Error message if the tweet failed
        reason: String,
    },
}

pub(crate) struct MentionedTweets {
    api_base: String,
}

impl NexusTool for MentionedTweets {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string() + "/users",
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.mentioned-tweets@1")
    }

    fn path() -> &'static str {
        "/mentioned-tweets"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        let client = Client::new();

        // Construct URL with user ID
        let url = format!("{}/{}/mentions", self.api_base, request.id);
        let mut req_builder = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", request.bearer_token));

        // Add optional query parameters if they exist
        if let Some(since_id) = &request.since_id {
            req_builder = req_builder.query(&[("since_id", since_id)]);
        }
        if let Some(until_id) = &request.until_id {
            req_builder = req_builder.query(&[("until_id", until_id)]);
        }
        if let Some(max_results) = request.max_results {
            req_builder = req_builder.query(&[("max_results", max_results.to_string())]);
        }
        if let Some(pagination_token) = &request.pagination_token {
            req_builder = req_builder.query(&[("pagination_token", pagination_token)]);
        }
        if let Some(start_time) = &request.start_time {
            req_builder = req_builder.query(&[("start_time", start_time)]);
        }
        if let Some(end_time) = &request.end_time {
            req_builder = req_builder.query(&[("end_time", end_time)]);
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
        if let Some(media_fields) = &request.media_fields {
            let fields: Vec<String> = media_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            req_builder = req_builder.query(&[("media.fields", fields.join(","))]);
        }
        if let Some(poll_fields) = &request.poll_fields {
            let fields: Vec<String> = poll_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            req_builder = req_builder.query(&[("poll.fields", fields.join(","))]);
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
        if let Some(place_fields) = &request.place_fields {
            let fields: Vec<String> = place_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap()
                        .replace("\"", "")
                        .to_lowercase()
                })
                .collect();
            req_builder = req_builder.query(&[("place.fields", fields.join(","))]);
        }

        match req_builder.send().await {
            Ok(response) => {
                let status = response.status();

                match response.text().await {
                    Ok(text) => {
                        // First check if response contains error
                        if let Ok(error) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(errors) = error.get("errors") {
                                if let Some(first_error) = errors.as_array().and_then(|e| e.first())
                                {
                                    let message = first_error
                                        .get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown error");
                                    let code = first_error
                                        .get("code")
                                        .and_then(|c| c.as_u64())
                                        .unwrap_or(0);
                                    return Output::Err {
                                        reason: format!(
                                            "Twitter API error: {} (code: {})",
                                            message, code
                                        ),
                                    };
                                }
                            }
                        }

                        // If no explicit error was found but status is not success
                        if !status.is_success() {
                            return Output::Err {
                                reason: format!("Twitter API returned error status: {}", status),
                            };
                        }

                        // If no error and status is success, try to parse as successful response
                        match serde_json::from_str::<TweetsResponse>(&text) {
                            Ok(response) => Output::Ok { result: response },
                            Err(e) => Output::Err {
                                reason: format!("Failed to parse Twitter API response: {}", e),
                            },
                        }
                    }
                    Err(e) => Output::Err {
                        reason: format!("Failed to read Twitter API response: {}", e),
                    },
                }
            }
            Err(e) => Output::Err {
                reason: format!("Failed to send request: {}", e),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, ::mockito::Server, serde_json::json};

    impl MentionedTweets {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string() + "/users",
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, MentionedTweets) {
        let server = Server::new_async().await;
        let tool = MentionedTweets::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            id: "2244994945".to_string(),
            since_id: None,
            until_id: None,
            max_results: Some(10),
            pagination_token: None,
            start_time: None,
            end_time: None,
            tweet_fields: Some(vec![TweetField::Text, TweetField::AuthorId]),
            expansions: Some(vec![ExpansionField::AuthorId]),
            media_fields: None,
            poll_fields: None,
            user_fields: Some(vec![UserField::Username, UserField::Name]),
            place_fields: None,
        }
    }

    #[tokio::test]
    async fn test_mentioned_tweets_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        // Match any query parameters
        let mock = server
            .mock("GET", "/users/2244994945/mentions")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "author_id": "2244994945",
                            "id": "1346889436626259968",
                            "text": "Learn how to use the user Tweet timeline"
                        }
                    ],
                    "includes": {
                        "users": [
                            {
                                "id": "2244994945",
                                "name": "X Dev",
                                "username": "TwitterDev",
                                "protected": false
                            }
                        ]
                    },
                    "meta": {
                        "newest_id": "1346889436626259968",
                        "oldest_id": "1346889436626259968",
                        "result_count": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { result } => {
                assert!(result.data.is_some());
                let data = result.data.unwrap();
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].id, "1346889436626259968");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_unauthorized_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/mentions")
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
            Output::Err { reason } => {
                assert!(reason.contains("Twitter API error: Unauthorized (code: 32)"), "Expected error message to contain 'Twitter API error: Unauthorized (code: 32)', got: {}", reason);
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_rate_limit_error() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/mentions")
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
            Output::Err { reason } => {
                assert!(reason.contains("Twitter API error: Rate limit exceeded (code: 88)"), "Expected error message to contain 'Twitter API error: Rate limit exceeded (code: 88)', got: {}", reason);
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_json_response() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/mentions")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("invalid json")
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Err { reason } => {
                assert!(reason.contains("Failed to parse Twitter API response"), 
                    "Expected error message to contain 'Failed to parse Twitter API response', got: {}", reason);
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_no_data_in_response() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/users/2244994945/mentions")
            .match_header("Authorization", "Bearer test_bearer_token")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
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
            Output::Ok { result } => {
                assert!(result.data.is_none() || result.data.unwrap().is_empty());
                assert!(result.meta.is_some());
                if let Some(meta) = result.meta {
                    assert_eq!(meta.result_count, Some(0));
                }
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }
}
