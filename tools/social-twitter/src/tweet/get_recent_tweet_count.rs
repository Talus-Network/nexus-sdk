//! # xyz.taluslabs.social.twitter.get-recent-tweet-count@1
//!
//! Standard Nexus Tool that retrieves tweet counts for queries from the Twitter API.

use {
    crate::{
        error::TwitterErrorKind,
        tweet::models::{Granularity, TweetCount, TweetCountMeta, TweetCountResponse},
        twitter_client::{TwitterClient, TWITTER_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

impl Default for Granularity {
    fn default() -> Self {
        Granularity::Hour
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,

    /// Search query for counting tweets
    query: String,

    /// The oldest UTC timestamp from which the tweets will be counted (YYYY-MM-DDTHH:mm:ssZ)
    #[serde(skip_serializing_if = "Option::is_none")]
    start_time: Option<String>,

    /// The newest UTC timestamp to which the tweets will be counted (YYYY-MM-DDTHH:mm:ssZ)
    #[serde(skip_serializing_if = "Option::is_none")]
    end_time: Option<String>,

    /// Returns results with a tweet ID greater than (more recent than) the specified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    since_id: Option<String>,

    /// Returns results with a tweet ID less than (older than) the specified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    until_id: Option<String>,

    /// Token for pagination to get the next page of results
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,

    /// Alternative parameter for pagination (same as next_token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination_token: Option<String>,

    /// Time granularity for the counts (minute, hour, day)
    #[serde(skip_serializing_if = "Option::is_none")]
    granularity: Option<Granularity>,

    /// A comma separated list of SearchCount fields to display
    #[serde(skip_serializing_if = "Option::is_none")]
    search_count_fields: Option<Vec<String>>,
}

impl Input {
    /// Validate input parameters
    fn validate(&self) -> Result<(), String> {
        // Validate timestamp format
        if let Some(ts) = &self.start_time {
            if !is_valid_timestamp_format(ts) {
                return Err(format!(
                    "Invalid start_time format: {}. Expected format: YYYY-MM-DDTHH:mm:ssZ",
                    ts
                ));
            }
        }

        if let Some(ts) = &self.end_time {
            if !is_valid_timestamp_format(ts) {
                return Err(format!(
                    "Invalid end_time format: {}. Expected format: YYYY-MM-DDTHH:mm:ssZ",
                    ts
                ));
            }
        }

        Ok(())
    }
}

/// Check if a string is a valid ISO 8601 timestamp (YYYY-MM-DDTHH:mm:ssZ)
fn is_valid_timestamp_format(timestamp: &str) -> bool {
    // Basic validation without regex - expecting exact format: YYYY-MM-DDTHH:mm:ssZ

    // Check length
    if timestamp.len() != 20 {
        return false;
    }

    // Check structure
    if !timestamp.chars().nth(4).map_or(false, |c| c == '-')
        || !timestamp.chars().nth(7).map_or(false, |c| c == '-')
        || !timestamp.chars().nth(10).map_or(false, |c| c == 'T')
        || !timestamp.chars().nth(13).map_or(false, |c| c == ':')
        || !timestamp.chars().nth(16).map_or(false, |c| c == ':')
        || !timestamp.chars().nth(19).map_or(false, |c| c == 'Z')
    {
        return false;
    }

    // Check digits
    let parts = &[
        &timestamp[0..4],   // Year
        &timestamp[5..7],   // Month
        &timestamp[8..10],  // Day
        &timestamp[11..13], // Hour
        &timestamp[14..16], // Minute
        &timestamp[17..19], // Second
    ];

    for part in parts {
        if !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }

    true
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Array of tweet count data
        data: Vec<TweetCount>,
        /// Metadata about the tweet counts request
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<TweetCountMeta>,
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

pub(crate) struct GetRecentTweetCount {
    api_base: String,
}

impl NexusTool for GetRecentTweetCount {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-recent-tweet-count@1")
    }

    fn path() -> &'static str {
        "/get-recent-tweet-count"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Validate input parameters first
        if let Err(e) = request.validate() {
            return Output::Err {
                kind: TwitterErrorKind::Validation,
                reason: format!("Input validation error: {}", e),
                status_code: None,
            };
        }

        let client = match TwitterClient::new(Some("tweets/counts/recent"), Some(&self.api_base)) {
            Ok(client) => client,
            Err(e) => {
                return Output::Err {
                    reason: e.to_string(),
                    kind: TwitterErrorKind::Network,
                    status_code: None,
                };
            }
        };

        let mut query_params: Vec<(String, String)> = Vec::new();

        query_params.push(("query".to_string(), request.query.clone()));

        if let Some(start_time) = &request.start_time {
            query_params.push(("start_time".to_string(), start_time.clone()));
        }

        if let Some(end_time) = &request.end_time {
            query_params.push(("end_time".to_string(), end_time.clone()));
        }

        if let Some(since_id) = &request.since_id {
            query_params.push(("since_id".to_string(), since_id.clone()));
        }

        if let Some(until_id) = &request.until_id {
            query_params.push(("until_id".to_string(), until_id.clone()));
        }

        if let Some(token) = request
            .next_token
            .as_ref()
            .or(request.pagination_token.as_ref())
        {
            query_params.push(("next_token".to_string(), token.clone()));
        }

        if let Some(granularity) = &request.granularity {
            let granularity_str = match granularity {
                Granularity::Minute => "minute",
                Granularity::Hour => "hour",
                Granularity::Day => "day",
            };
            query_params.push(("granularity".to_string(), granularity_str.to_string()));
        }

        if let Some(fields) = &request.search_count_fields {
            if !fields.is_empty() {
                query_params.push(("search_count.fields".to_string(), fields.join(",")));
            }
        }

        match client
            .get::<TweetCountResponse>(request.bearer_token, Some(query_params))
            .await
        {
            Ok((data, meta)) => {
                if data.is_empty() {
                    Output::Err {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No tweet count data found".to_string(),
                        status_code: None,
                    }
                } else {
                    Output::Ok { data, meta }
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

    impl GetRecentTweetCount {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetRecentTweetCount) {
        let server = Server::new_async().await;
        let tool = GetRecentTweetCount::with_api_base(&server.url());
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            bearer_token: "test_bearer_token".to_string(),
            query: "from:TwitterDev".to_string(),
            start_time: None,
            end_time: None,
            since_id: None,
            until_id: None,
            next_token: None,
            pagination_token: None,
            granularity: None,
            search_count_fields: None,
        }
    }

    #[tokio::test]
    async fn test_recent_tweet_count_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/counts/recent")
            .match_query(mockito::Matcher::UrlEncoded(
                "query".into(),
                "from:TwitterDev".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "end": "2023-01-01T01:00:00Z",
                            "start": "2023-01-01T00:00:00Z",
                            "tweet_count": 12
                        },
                        {
                            "end": "2023-01-01T02:00:00Z",
                            "start": "2023-01-01T01:00:00Z",
                            "tweet_count": 5
                        }
                    ],
                    "meta": {
                        "total_tweet_count": 17
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok { data, meta } => {
                assert_eq!(data.len(), 2);
                assert_eq!(data[0].tweet_count, 12);
                assert_eq!(data[1].tweet_count, 5);

                assert!(meta.is_some());
                if let Some(meta_data) = meta {
                    assert_eq!(meta_data.total_tweet_count, Some(17));
                }
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_recent_tweet_count_empty_results() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/counts/recent")
            .match_query(mockito::Matcher::UrlEncoded(
                "query".into(),
                "from:TwitterDev".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [],
                    "meta": {
                        "total_tweet_count": 0
                    }
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
                assert_eq!(reason, "No tweet count data found");
                assert_eq!(status_code, None);
            }
            Output::Ok { .. } => panic!("Expected error due to no results, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_recent_tweet_count_with_granularity() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/counts/recent")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "from:TwitterDev".into()),
                mockito::Matcher::UrlEncoded("granularity".into(), "day".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "end": "2023-01-02T00:00:00Z",
                            "start": "2023-01-01T00:00:00Z",
                            "tweet_count": 45
                        }
                    ],
                    "meta": {
                        "total_tweet_count": 45
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut input = create_test_input();
        input.granularity = Some(Granularity::Day);

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { data, .. } => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].tweet_count, 45);
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_recent_tweet_count_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/counts/recent")
            .match_query(mockito::Matcher::Any)
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "title": "Unauthorized",
                    "type": "about:blank",
                    "status": 401,
                    "detail": "Client Forbidden"
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
                assert_eq!(kind, TwitterErrorKind::Auth);
                assert!(reason.contains("Unauthorized") || reason.contains("Client Forbidden"));
                assert_eq!(status_code, Some(401));
            }
            Output::Ok { .. } => panic!("Expected error, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_recent_tweet_count_invalid_query() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/counts/recent")
            .match_query(mockito::Matcher::Any)
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                     "errors": [
                         {
                             "parameters": {
                                 "query": [
                                     "from:TwitterDev OR"
                                 ]
                             },
                            "message": "Invalid query",
                            "title": "Invalid Request",
                            "detail": "One or more parameters to your request was invalid.",
                            "type": "https://api.twitter.com/2/problems/invalid-request"
                         }
                     ]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut input = create_test_input();
        input.query = "from:TwitterDev OR".to_string();

        let output = tool.invoke(input).await;

        match output {
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(kind, TwitterErrorKind::Api);
                assert!(reason.contains("Invalid Request") || reason.contains("Invalid query"));
                assert_eq!(status_code, None);
            }
            Output::Ok { .. } => panic!("Expected error for invalid query, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_timestamp_format_validation() {
        let (mut _server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.start_time = Some("2023-01-01 12:00:00".to_string());

        let output = tool.invoke(input).await;

        match output {
            Output::Err { reason, kind, .. } => {
                assert_eq!(kind, TwitterErrorKind::Validation);
                assert!(
                    reason.contains("Input validation error: Invalid start_time format"),
                    "Expected validation error message, got: {}",
                    reason
                );
            }
            Output::Ok { .. } => {
                panic!("Expected error due to invalid timestamp format, got success")
            }
        }
    }

    #[tokio::test]
    async fn test_valid_timestamp_format() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/counts/recent")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "from:TwitterDev".into()),
                mockito::Matcher::UrlEncoded("start_time".into(), "2023-01-01T12:00:00Z".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "end": "2023-01-01T13:00:00Z",
                            "start": "2023-01-01T12:00:00Z",
                            "tweet_count": 10
                        }
                    ],
                    "meta": {
                        "total_tweet_count": 10
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let mut input = create_test_input();
        input.start_time = Some("2023-01-01T12:00:00Z".to_string());

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { data, .. } => {
                assert_eq!(data.len(), 1);
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }
}
