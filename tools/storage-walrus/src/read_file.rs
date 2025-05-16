//! # `xyz.taluslabs.storage.walrus.read-file@1`
//!
//! Standard Nexus Tool that reads a file from Walrus and returns the contents.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, walrus::WalrusError, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    thiserror::Error,
};

/// Errors that can occur during file read
#[derive(Error, Debug)]
pub enum ReadFileError {
    #[error("Failed to read file: {0}")]
    ReadError(#[from] WalrusError),
}

/// Types of errors that can occur during file read
#[derive(Serialize, JsonSchema, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReadErrorKind {
    /// Error during network request
    Network,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The blob ID of the file to read
    blob_id: String,
    /// The URL of the aggregator to read the file from
    #[serde(
        default,
        deserialize_with = "crate::utils::validation::deserialize_url_opt"
    )]
    aggregator_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        bytes: Vec<u8>,
    },
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (network, validation, etc.)
        kind: ReadErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct ReadFile;

impl NexusTool for ReadFile {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.storage.walrus.read-file@1")
    }

    fn path() -> &'static str {
        "/read-file"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        match self.read_file(input).await {
            Ok(bytes) => Output::Ok { bytes },
            Err(e) => {
                let (kind, status_code) = match &e {
                    ReadFileError::ReadError(err) => {
                        let status_code = match err {
                            WalrusError::ApiError { status_code, .. } => Some(*status_code),
                            _ => None,
                        };

                        (ReadErrorKind::Network, status_code)
                    }
                };

                Output::Err {
                    reason: e.to_string(),
                    kind,
                    status_code,
                }
            }
        }
    }
}

impl ReadFile {
    async fn read_file(&self, input: Input) -> Result<Vec<u8>, ReadFileError> {
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let _contents = walrus_client.read_file(&input.blob_id).await?;

        Ok(_contents)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, nexus_sdk::walrus::WalrusClient};

    impl ReadFile {
        // Helper method for testing
        fn with_custom_client() -> Self {
            Self {}
        }

        async fn read_for_test(
            &self,
            input: &Input,
            client: WalrusClient,
        ) -> Result<Vec<u8>, ReadFileError> {
            client
                .read_file(&input.blob_id)
                .await
                .map_err(ReadFileError::ReadError)
        }
    }

    async fn create_test_server_and_input() -> (mockito::ServerGuard, Input, &'static [u8]) {
        let server = Server::new_async().await;
        let server_url = server.url();

        // Set up test input with server URL
        let input = Input {
            blob_id: "test_blob_id".to_string(),
            aggregator_url: Some(server_url.clone()),
        };

        // Create file content for test
        static FILE_CONTENT: &[u8] = b"Hello, World!";

        (server, input, FILE_CONTENT)
    }

    #[tokio::test]
    async fn test_read_file_success() {
        // Create server and input
        let (mut server, input, file_content) = create_test_server_and_input().await;

        // Set up mock response
        let mock = server
            .mock("GET", format!("/v1/blobs/{}", input.blob_id).as_str())
            .with_status(200)
            .with_header("content-type", "application/octet-stream")
            .with_body(file_content)
            .create_async()
            .await;

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = ReadFile::with_custom_client();
        let result = tool.read_for_test(&input, walrus_client).await;

        // Verify the result
        assert!(result.is_ok(), "Read should succeed but got: {:?}", result);

        // Verify the content was read correctly
        let content = result.unwrap();
        assert_eq!(content, file_content);

        // Verify the request was made
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_read_file_error() {
        // Create server and input
        let (mut server, input, _) = create_test_server_and_input().await;

        // Mock server error
        let mock = server
            .mock("GET", format!("/v1/blobs/{}", input.blob_id).as_str())
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "Internal server error"}"#)
            .create_async()
            .await;

        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = ReadFile::with_custom_client();
        let result = tool.read_for_test(&input, walrus_client).await;

        assert!(result.is_err(), "Expected error result");

        // Check if error contains 500 status code
        let error = result.unwrap_err();
        match error {
            ReadFileError::ReadError(WalrusError::ApiError { status_code, .. }) => {
                assert_eq!(status_code, 500);
            }
            _ => panic!("Unexpected error type: {:?}", error),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_read_nonexistent_blob() {
        // Create server and input
        let (mut server, input, _) = create_test_server_and_input().await;

        // Mock not found response
        let mock = server
            .mock("GET", format!("/v1/blobs/{}", input.blob_id).as_str())
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "Blob not found"}"#)
            .create_async()
            .await;

        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = ReadFile::with_custom_client();
        let result = tool.read_for_test(&input, walrus_client).await;

        assert!(result.is_err(), "Expected error result");

        // Check if error contains 404 status code
        let error = result.unwrap_err();
        match error {
            ReadFileError::ReadError(WalrusError::ApiError { status_code, .. }) => {
                assert_eq!(status_code, 404);
            }
            _ => panic!("Unexpected error type: {:?}", error),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invoke_success() {
        // Create server and input
        let (mut server, input, file_content) = create_test_server_and_input().await;

        // Set up mock response
        let mock = server
            .mock("GET", format!("/v1/blobs/{}", input.blob_id).as_str())
            .with_status(200)
            .with_header("content-type", "application/octet-stream")
            .with_body(file_content)
            .create_async()
            .await;

        // Create the tool and invoke it
        let tool = ReadFile::with_custom_client();
        let result = tool.invoke(input).await;

        // Verify correct output format
        match result {
            Output::Ok { bytes } => {
                assert_eq!(bytes, file_content);
            }
            Output::Err { reason, .. } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invoke_error() {
        // Create server and input
        let (mut server, input, _) = create_test_server_and_input().await;

        // Mock server error
        let mock = server
            .mock("GET", format!("/v1/blobs/{}", input.blob_id).as_str())
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "Internal server error"}"#)
            .create_async()
            .await;

        // Create the tool and invoke it
        let tool = ReadFile::with_custom_client();
        let result = tool.invoke(input).await;

        // Verify error output format
        match result {
            Output::Ok { .. } => {
                panic!("Expected error result, got success");
            }
            Output::Err {
                kind, status_code, ..
            } => {
                assert_eq!(kind, ReadErrorKind::Network);
                assert_eq!(status_code, Some(500));
            }
        }

        mock.assert_async().await;
    }
}
