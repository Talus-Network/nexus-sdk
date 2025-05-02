//! # `xyz.taluslabs.storage.walrus.read-json@1`
//!
//! Standard Nexus Tool that reads a JSON file from Walrus and returns the JSON data.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    thiserror::Error,
};

/// Errors that can occur during JSON upload
#[derive(Error, Debug)]
pub enum ReadJsonError {
    #[error("Failed to read JSON: {0}")]
    ReadError(#[from] anyhow::Error),
    #[error("Invalid JSON data: {0}")]
    InvalidJson(String),
}

/// Types of errors that can occur during JSON read
#[derive(Serialize, JsonSchema, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReadErrorKind {
    /// Error during network request
    Network,
    /// Error validating JSON
    Validation,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The blob ID of the JSON file to read
    blob_id: String,

    /// The URL of the Walrus aggregator to read the JSON from
    #[serde(default)]
    aggregator_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        json: Value,
        text: String,
    },
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (upload, validation, etc.)
        kind: ReadErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct ReadJson;

impl NexusTool for ReadJson {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.storage.walrus.read-json@1")
    }

    fn path() -> &'static str {
        "/json/read"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        match self.read(input).await {
            Ok(string_result) => {
                let json_data = serde_json::from_str(&string_result);

                match json_data {
                    Ok(json) => Output::Ok {
                        json,
                        text: string_result,
                    },
                    Err(e) => Output::Err {
                        reason: ReadJsonError::InvalidJson(e.to_string()).to_string(),
                        kind: ReadErrorKind::Validation,
                        status_code: None,
                    },
                }
            }
            Err(e) => {
                let status_code = e
                    .to_string()
                    .split("status ")
                    .nth(1)
                    .and_then(|s| s.split(':').next())
                    .and_then(|s| s.trim().parse::<u16>().ok());

                Output::Err {
                    reason: e.to_string(),
                    kind: ReadErrorKind::Network,
                    status_code,
                }
            }
        }
    }
}

impl ReadJson {
    async fn read(&self, input: Input) -> Result<String, ReadJsonError> {
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let storage_info = walrus_client.read_json(&input.blob_id).await;

        match storage_info {
            Ok(string_result) => Ok(string_result),
            Err(e) => Err(ReadJsonError::ReadError(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, nexus_sdk::walrus::WalrusClient, serde_json::json};

    // Helper function to create test input
    fn create_test_input() -> Input {
        Input {
            blob_id: "test_blob_id".to_string(),
            aggregator_url: None,
        }
    }

    // Helper function to create mock server and client
    async fn create_mock_server_and_client() -> (mockito::ServerGuard, WalrusClient) {
        let server = Server::new_async().await;
        let client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        (server, client)
    }

    #[tokio::test]
    async fn test_read_json_success() {
        let (mut server, client) = create_mock_server_and_client().await;
        let input = create_test_input();

        // Mock successful response
        let mock = server
            .mock("GET", "/v1/blobs/test_blob_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "name": "test",
                    "value": 123
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result: Result<serde_json::Value, anyhow::Error> =
            client.read_json::<serde_json::Value>(&input.blob_id).await;

        match result {
            Ok(json_data) => {
                assert_eq!(json_data["name"], "test");
                assert_eq!(json_data["value"], 123);
            }
            Err(e) => panic!("Expected successful JSON read, but got error: {}", e),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_read_json_not_found() {
        let (mut server, client) = create_mock_server_and_client().await;
        let input = create_test_input();

        // Mock not found response
        let mock = server
            .mock("GET", "/v1/blobs/test_blob_id")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "error": "Blob not found"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result: Result<serde_json::Value, anyhow::Error> =
            client.read_json::<serde_json::Value>(&input.blob_id).await;

        match result {
            Ok(_) => panic!("Expected error, but got successful JSON read"),
            Err(e) => assert!(e.to_string().contains("Blob not found")),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_read_json_server_error() {
        let (mut server, client) = create_mock_server_and_client().await;
        let input = create_test_input();

        // Mock server error
        let mock = server
            .mock("GET", "/v1/blobs/test_blob_id")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "error": "Internal server error"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result: Result<serde_json::Value, anyhow::Error> =
            client.read_json::<serde_json::Value>(&input.blob_id).await;
        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_read_json_invalid_json() {
        let (mut server, client) = create_mock_server_and_client().await;
        let input = create_test_input();

        // Mock response with invalid JSON
        let mock = server
            .mock("GET", "/v1/blobs/test_blob_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("invalid json")
            .create_async()
            .await;

        let result: Result<serde_json::Value, anyhow::Error> =
            client.read_json::<serde_json::Value>(&input.blob_id).await;
        assert!(result.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_read_json_with_custom_aggregator() {
        let (mut server, client) = create_mock_server_and_client().await;
        let mut input = create_test_input();
        input.aggregator_url = Some(server.url());

        // Mock successful response
        let mock = server
            .mock("GET", "/v1/blobs/test_blob_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "name": "test",
                    "value": 123
                })
                .to_string(),
            )
            .create_async()
            .await;

        let result: Result<serde_json::Value, anyhow::Error> =
            client.read_json::<serde_json::Value>(&input.blob_id).await;
        assert!(result.is_ok());
        let json_data = result.unwrap();
        assert_eq!(json_data["name"], "test");
        assert_eq!(json_data["value"], 123);

        mock.assert_async().await;
    }
}
