//! # xyz.taluslabs.social.twitter.get-recent-tweet-count@1
//!
//! Standard Nexus Tool that retrieves tweet counts for queries from the Twitter API.

use {
    crate::{
        error::{
            parse_twitter_response,
            TwitterError,
            TwitterErrorKind,
            TwitterErrorResponse,
            TwitterResult,
        },
        tweet::{
            models::{Granularity, TweetCount, TweetCountMeta, TweetCountResponse},
            TWITTER_API_BASE,
        },
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
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
            api_base: TWITTER_API_BASE.to_string() + "/tweets/counts/recent",
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
        match self.fetch_tweet_counts(&request).await {
            Ok(response) => {
                if let Some(tweet_counts) = response.data {
                    Output::Ok {
                        data: tweet_counts,
                        meta: response.meta,
                    }
                } else {
                    let error_response = TwitterErrorResponse {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No tweet count data found".to_string(),
                        status_code: None,
                    };

                    Output::Err {
                        kind: error_response.kind,
                        reason: error_response.reason,
                        status_code: error_response.status_code,
                    }
                }
            }
            Err(e) => {
                // Use the centralized error conversion
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

impl GetRecentTweetCount {
    /// Fetch tweet counts from Twitter's recent counts API
    async fn fetch_tweet_counts(&self, request: &Input) -> TwitterResult<TweetCountResponse> {
        // Validate input parameters
        if let Err(e) = request.validate() {
            return Err(TwitterError::Other(format!("Validation error: {}", e)));
        }

        let client = Client::new();
        // Construct the URL with query parameters
        let mut url =
            reqwest::Url::parse(&self.api_base).map_err(|e| TwitterError::Other(e.to_string()))?;

        // Add the required query parameter
        url.query_pairs_mut().append_pair("query", &request.query);

        // Add optional query parameters if provided
        if let Some(start_time) = &request.start_time {
            url.query_pairs_mut().append_pair("start_time", start_time);
        }

        if let Some(end_time) = &request.end_time {
            url.query_pairs_mut().append_pair("end_time", end_time);
        }

        if let Some(since_id) = &request.since_id {
            url.query_pairs_mut().append_pair("since_id", since_id);
        }

        if let Some(until_id) = &request.until_id {
            url.query_pairs_mut().append_pair("until_id", until_id);
        }

        if let Some(next_token) = &request.next_token {
            url.query_pairs_mut().append_pair("next_token", next_token);
        } else if let Some(pagination_token) = &request.pagination_token {
            url.query_pairs_mut()
                .append_pair("pagination_token", pagination_token);
        }

        if let Some(granularity) = &request.granularity {
            let granularity_str = match granularity {
                Granularity::Minute => "minute",
                Granularity::Hour => "hour",
                Granularity::Day => "day",
            };
            url.query_pairs_mut()
                .append_pair("granularity", granularity_str);
        }

        if let Some(fields) = &request.search_count_fields {
            url.query_pairs_mut()
                .append_pair("search_count.fields", &fields.join(","));
        }

        // Make the request
        let response = client
            .get(url)
            .header("Authorization", format!("Bearer {}", request.bearer_token))
            .send()
            .await?;

        parse_twitter_response::<TweetCountResponse>(response).await
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
        let tool = GetRecentTweetCount::with_api_base(&(server.url() + "/tweets/counts/recent"));
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
                assert_eq!(kind, TwitterErrorKind::Auth);
                assert!(
                    reason.contains("Unauthorized"),
                    "Expected error message to contain 'Unauthorized', got: {}",
                    reason
                );
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
            Output::Err { reason, kind, .. } => {
                assert_eq!(kind, TwitterErrorKind::Api);
                assert!(
                    reason.contains("Invalid query"),
                    "Expected error message to contain 'Invalid query', got: {}",
                    reason
                );
            }
            Output::Ok { .. } => panic!("Expected error for invalid query, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_timestamp_format_validation() {
        let (mut _server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        // Invalid timestamp format
        input.start_time = Some("2023-01-01 12:00:00".to_string());

        let output = tool.invoke(input).await;

        match output {
            Output::Err { reason, kind, .. } => {
                assert_eq!(kind, TwitterErrorKind::Unknown);
                assert!(
                    reason.contains("Validation error"),
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
        // Valid timestamp format
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
