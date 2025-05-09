//! # xyz.taluslabs.social.twitter.get-recent-search-tweets@1
//!
//! Standard Nexus Tool that retrieves tweets from the recent search API.

use {
    crate::{
        error::TwitterErrorKind,
        tweet::models::{
            ExpansionField,
            Includes,
            MediaField,
            Meta,
            PlaceField,
            PollField,
            Tweet,
            TweetField,
            TweetsResponse,
            UserField,
        },
        twitter_client::{TwitterClient, TWITTER_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json,
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Bearer Token for user's Twitter account
    bearer_token: String,

    /// Search query for matching tweets
    query: String,

    /// The oldest UTC timestamp from which the tweets will be provided (YYYY-MM-DDTHH:mm:ssZ)
    #[serde(skip_serializing_if = "Option::is_none")]
    start_time: Option<String>,

    /// The newest UTC timestamp to which the tweets will be provided (YYYY-MM-DDTHH:mm:ssZ)
    #[serde(skip_serializing_if = "Option::is_none")]
    end_time: Option<String>,

    /// Returns results with a tweet ID greater than (more recent than) the specified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    since_id: Option<String>,

    /// Returns results with a tweet ID less than (older than) the specified ID
    #[serde(skip_serializing_if = "Option::is_none")]
    until_id: Option<String>,

    /// The maximum number of search results to be returned (between 10 and 100)
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<i32>,

    /// Token for pagination to get the next page of results
    #[serde(skip_serializing_if = "Option::is_none")]
    next_token: Option<String>,

    /// Alternative parameter for pagination (same as next_token)
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination_token: Option<String>,

    /// Order in which to return results (recency or relevancy)
    #[serde(skip_serializing_if = "Option::is_none")]
    sort_order: Option<String>,

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

impl Input {
    /// Validate input parameters
    fn validate(&self) -> Result<(), String> {
        // Validate max_results (10-100)
        if let Some(max_results) = self.max_results {
            if max_results < 10 || max_results > 100 {
                return Err(format!(
                    "max_results must be between 10 and 100, got {}",
                    max_results
                ));
            }
        }

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

        // Validate sort_order
        if let Some(order) = &self.sort_order {
            if order != "recency" && order != "relevancy" {
                return Err(format!(
                    "sort_order must be either 'recency' or 'relevancy', got '{}'",
                    order
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

    // Could add further validation for range values (month 1-12, day 1-31, etc.)
    // but keeping it simple for this implementation

    true
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Array of tweet data
        data: Vec<Tweet>,
        /// Additional entities related to the tweets
        #[serde(skip_serializing_if = "Option::is_none")]
        includes: Option<Includes>,
        /// Metadata about the tweets request
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

pub(crate) struct GetRecentSearchTweets {
    api_base: String,
}

impl NexusTool for GetRecentSearchTweets {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.get-recent-search-tweets@1")
    }

    fn path() -> &'static str {
        "/get-recent-search-tweets"
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

        let client = match TwitterClient::new(Some("tweets/search/recent"), Some(&self.api_base)) {
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

        if let Some(max_results) = request.max_results {
            query_params.push(("max_results".to_string(), max_results.to_string()));
        }

        if let Some(token) = request
            .next_token
            .as_ref()
            .or(request.pagination_token.as_ref())
        {
            query_params.push(("next_token".to_string(), token.clone()));
        }

        if let Some(sort_order) = &request.sort_order {
            query_params.push(("sort_order".to_string(), sort_order.clone()));
        }

        if let Some(tweet_fields) = &request.tweet_fields {
            let fields: Vec<String> = tweet_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap_or_default()
                        .replace('"', "")
                        .to_lowercase()
                })
                .collect();
            if !fields.is_empty() {
                query_params.push(("tweet.fields".to_string(), fields.join(",")));
            }
        }

        if let Some(expansions) = &request.expansions {
            let fields: Vec<String> = expansions
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap_or_default()
                        .replace('"', "")
                        .to_lowercase()
                })
                .collect();
            if !fields.is_empty() {
                query_params.push(("expansions".to_string(), fields.join(",")));
            }
        }

        if let Some(media_fields) = &request.media_fields {
            let fields: Vec<String> = media_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap_or_default()
                        .replace('"', "")
                        .to_lowercase()
                })
                .collect();
            if !fields.is_empty() {
                query_params.push(("media.fields".to_string(), fields.join(",")));
            }
        }

        if let Some(poll_fields) = &request.poll_fields {
            let fields: Vec<String> = poll_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap_or_default()
                        .replace('"', "")
                        .to_lowercase()
                })
                .collect();
            if !fields.is_empty() {
                query_params.push(("poll.fields".to_string(), fields.join(",")));
            }
        }

        if let Some(user_fields) = &request.user_fields {
            let fields: Vec<String> = user_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap_or_default()
                        .replace('"', "")
                        .to_lowercase()
                })
                .collect();
            if !fields.is_empty() {
                query_params.push(("user.fields".to_string(), fields.join(",")));
            }
        }

        if let Some(place_fields) = &request.place_fields {
            let fields: Vec<String> = place_fields
                .iter()
                .map(|f| {
                    serde_json::to_string(f)
                        .unwrap_or_default()
                        .replace('"', "")
                        .to_lowercase()
                })
                .collect();
            if !fields.is_empty() {
                query_params.push(("place.fields".to_string(), fields.join(",")));
            }
        }

        match client
            .get::<TweetsResponse>(request.bearer_token, Some(query_params))
            .await
        {
            Ok((data, includes, meta)) => {
                // If data is empty, it means no tweets were found matching the criteria.
                // This aligns with the original test_recent_search_empty_results expectation.
                if data.is_empty() {
                    Output::Err {
                        kind: TwitterErrorKind::NotFound,
                        reason: "No search results found".to_string(),
                        status_code: Some(404), // To match original test logic for empty results
                    }
                } else {
                    Output::Ok {
                        data,
                        includes,
                        meta,
                    }
                }
            }
            Err(e) => {
                // Return the error from TwitterClient
                Output::Err {
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
    use {super::*, ::mockito::Server, serde_json::json};

    impl GetRecentSearchTweets {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetRecentSearchTweets) {
        let server = Server::new_async().await;
        let tool = GetRecentSearchTweets::with_api_base(&server.url());
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
            max_results: Some(10),
            next_token: None,
            pagination_token: None,
            sort_order: None,
            tweet_fields: Some(vec![
                TweetField::Text,
                TweetField::AuthorId,
                TweetField::CreatedAt,
            ]),
            expansions: Some(vec![ExpansionField::AuthorId]),
            media_fields: None,
            poll_fields: None,
            user_fields: Some(vec![UserField::Username, UserField::Name]),
            place_fields: None,
        }
    }

    #[tokio::test]
    async fn test_recent_search_successful() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/search/recent")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "from:TwitterDev".into()),
                mockito::Matcher::UrlEncoded("max_results".into(), "10".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "author_id": "2244994945",
                            "created_at": "Wed Jan 06 18:40:40 +0000 2021",
                            "id": "1346889436626259968",
                            "text": "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2 to explore Tweet\\u2026 https:\\/\\/t.co\\/56a0vZUx7i"
                        },
                        {
                            "author_id": "2244994945",
                            "created_at": "Mon Dec 21 14:29:48 +0000 2020",
                            "id": "1341033593901268992",
                            "text": "As a Token-Based Authentication user, you can now take advantage of OAuth 2.0. Check out our updated documentation to learn how to implement this new method."
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
                        "oldest_id": "1341033593901268992",
                        "result_count": 2,
                        "next_token": "b26v89c19zqg8o3fo7dpmyo3vz9wkxa5fiuds5yu3eewn"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(create_test_input()).await;

        match output {
            Output::Ok {
                data,
                includes,
                meta,
            } => {
                assert_eq!(data.len(), 2);

                // Check first tweet
                assert_eq!(data[0].id, "1346889436626259968");
                assert_eq!(data[0].text, "Learn how to use the user Tweet timeline and user mention timeline endpoints in the X API v2 to explore Tweet\\u2026 https:\\/\\/t.co\\/56a0vZUx7i");
                assert_eq!(data[0].author_id, Some("2244994945".to_string()));

                // Check second tweet
                assert_eq!(data[1].id, "1341033593901268992");

                // Check includes and meta
                assert!(includes.is_some());
                assert!(meta.is_some());
                if let Some(meta_data) = meta {
                    assert_eq!(meta_data.result_count, Some(2));
                    assert_eq!(
                        meta_data.next_token,
                        Some("b26v89c19zqg8o3fo7dpmyo3vz9wkxa5fiuds5yu3eewn".to_string())
                    );
                }
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_recent_search_empty_results() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/search/recent")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "query".into(),
                "from:TwitterDev".into(),
            )]))
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
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(kind, TwitterErrorKind::NotFound);
                assert_eq!(reason, "No search results found");
                assert_eq!(status_code, Some(404));
            }
            Output::Ok { .. } => panic!("Expected error due to no results, got success"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_recent_search_with_pagination() {
        let (mut server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.next_token = Some("b26v89c19zqg8o3fo7dpmyo3vz9wkxa5fiuds5yu3eewn".to_string());

        let mock = server
            .mock("GET", "/tweets/search/recent")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "from:TwitterDev".into()),
                mockito::Matcher::UrlEncoded("next_token".into(), "b26v89c19zqg8o3fo7dpmyo3vz9wkxa5fiuds5yu3eewn".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "author_id": "2244994945",
                            "created_at": "Mon Dec 14 18:30:45 +0000 2020",
                            "id": "1338542389136314371",
                            "text": "For #TweetDeck users: we've rolled out a new, clearer way to see when a tweet gets dropped from a column. Now, you'll see a notice when a tweet, like the one shown in this video, drops from your Home column."
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
                        "newest_id": "1338542389136314371",
                        "oldest_id": "1338542389136314371",
                        "result_count": 1
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { data, .. } => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].id, "1338542389136314371");
            }
            Output::Err { reason, .. } => {
                panic!("Expected success with pagination, got error: {}", reason)
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_recent_search_unauthorized() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/search/recent")
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
    async fn test_recent_search_invalid_query() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/search/recent")
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
    async fn test_max_results_lower_boundary() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/search/recent")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "from:TwitterDev".into()),
                mockito::Matcher::UrlEncoded("max_results".into(), "10".into()), // Lower boundary
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "123456789",
                            "text": "Test tweet"
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

        let mut input = create_test_input();
        input.max_results = Some(10); // Lower boundary

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { data, .. } => {
                assert_eq!(data.len(), 1);
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_max_results_upper_boundary() {
        let (mut server, tool) = create_server_and_tool().await;

        let mock = server
            .mock("GET", "/tweets/search/recent")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("query".into(), "from:TwitterDev".into()),
                mockito::Matcher::UrlEncoded("max_results".into(), "100".into()), // Upper boundary
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": [
                        {
                            "id": "123456789",
                            "text": "Test tweet"
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

        let mut input = create_test_input();
        input.max_results = Some(100); // Upper boundary

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { data, .. } => {
                assert_eq!(data.len(), 1);
            }
            Output::Err { reason, .. } => panic!("Expected success, got error: {}", reason),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_max_results_outside_boundary() {
        let (mut _server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        input.max_results = Some(101); // Above upper boundary

        let output = tool.invoke(input).await;

        match output {
            Output::Err { reason, kind, .. } => {
                assert_eq!(kind, TwitterErrorKind::Validation);
                assert!(
                    reason
                        .contains("Input validation error: max_results must be between 10 and 100"),
                    "Expected validation error message, got: {}",
                    reason
                );
            }
            Output::Ok { .. } => panic!("Expected error due to invalid max_results, got success"),
        }
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
            .mock("GET", "/tweets/search/recent")
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
                            "id": "123456789",
                            "text": "Test tweet"
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

    #[tokio::test]
    async fn test_sort_order_validation() {
        let (mut _server, tool) = create_server_and_tool().await;

        let mut input = create_test_input();
        // Invalid sort order
        input.sort_order = Some("invalid".to_string());

        let output = tool.invoke(input).await;

        match output {
            Output::Err { reason, kind, .. } => {
                assert_eq!(kind, TwitterErrorKind::Validation);
                assert!(
                    reason.contains("Input validation error: sort_order must be either 'recency' or 'relevancy'"),
                    "Expected validation error message, got: {}",
                    reason
                );
            }
            Output::Ok { .. } => panic!("Expected error due to invalid sort order, got success"),
        }
    }
}
