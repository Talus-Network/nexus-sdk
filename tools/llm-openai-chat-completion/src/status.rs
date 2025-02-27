//! This module provides functionality for checking the health of the OpenAI API.
//!
//! It defines the structures for parsing the response from the OpenAI status API
//! and provides a function to check if the API is currently operational.

use {
    chrono::{DateTime, FixedOffset},
    nexus_toolkit::{AnyResult, StatusCode},
    reqwest::Client,
    serde::Deserialize,
};
// use {
//     chrono::Utc,
//     chrono_tz::Tz,
// }

/// The URL of the OpenAI status API.
const HEALTH_URL: &str = "https://status.openai.com/api/v2/status.json";
/// The expected status indicator for a healthy API.
const HEALTH_OK: &str = "none";

/// Represents the overall response from the OpenAI status API.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ApiResponse {
    /// Information about the status page.
    page: PageInfo,
    /// The current status of the API.
    status: StatusInfo,
}

/// Represents information about the status page.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PageInfo {
    /// The status.io ID of the page.
    id: String,
    /// The name of the page.
    name: String,
    /// The URL of the page.
    url: String,
    /// The time zone of the page.
    time_zone: String,
    /// The last time the page was updated.
    updated_at: DateTime<FixedOffset>, // Parsed with original offset
}

/// Represents the status of the API.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct StatusInfo {
    /// The status indicator (e.g., "none", "minor", "major", ...).
    indicator: String,
    /// A description of the current status.
    description: String,
}

/// Checks the health of the OpenAI API by querying its status endpoint.
///
/// This function sends a GET request to the OpenAI status API and parses the
/// response. It checks if the `status.indicator` is equal to `HEALTH_OK`.
///
/// # Returns
///
/// *   `Ok(StatusCode::OK)` if the API is healthy.
/// *   `Ok(StatusCode::SERVICE_UNAVAILABLE)` if the API is not healthy.
/// *   `Err(e)` if there is an error sending the request or parsing the response.
pub async fn check_api_health() -> AnyResult<StatusCode> {
    let client = Client::new();

    // Send GET request and parse JSON response
    let response: ApiResponse = client.get(HEALTH_URL).send().await?.json().await?;

    // TODO: Maybe we should communicate the update time in the response.

    // Convert updated_at to Utc and then to the specified time zone
    // let utc_time = response.page.updated_at.with_timezone(&Utc);
    // let time_zone: Tz = response.page.time_zone.parse().unwrap_or(chrono_tz::UTC);
    // let local_time = utc_time.with_timezone(&time_zone);

    if response.status.indicator != HEALTH_OK {
        return Ok(StatusCode::SERVICE_UNAVAILABLE);
    }

    Ok(StatusCode::OK)
}
