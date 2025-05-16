//! # `xyz.taluslabs.storage.walrus.verify-blob@1`
//!
//! Standard Nexus Tool that verifies a blob.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, walrus::WalrusError, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    thiserror::Error,
};

#[derive(Error, Debug)]
pub enum VerifyBlobError {
    #[error("Failed to verify blob: {0}")]
    VerificationError(#[from] WalrusError),
}

#[derive(Serialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum UploadErrorKind {
    Server,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The blob ID to verify
    blob_id: String,
    /// The URL of the Walrus aggregator to verify the blob on
    #[serde(
        default,
        deserialize_with = "crate::utils::validation::deserialize_url_opt"
    )]
    aggregator_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Verified {
        blob_id: String,
    },
    Unverified {
        blob_id: String,
    },
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (upload, validation, etc.)
        kind: UploadErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct VerifyBlob;

impl NexusTool for VerifyBlob {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.storage.walrus.verify-blob@1")
    }

    fn path() -> &'static str {
        "/verify-blob"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let blob_id = input.blob_id.clone();

        match self.verify_blob(input).await {
            Ok(verified) => {
                if verified {
                    Output::Verified { blob_id }
                } else {
                    Output::Unverified { blob_id }
                }
            }
            Err(e) => {
                let status_code = match &e {
                    VerifyBlobError::VerificationError(WalrusError::ApiError {
                        status_code,
                        ..
                    }) => Some(*status_code),
                    _ => None,
                };

                Output::Err {
                    reason: e.to_string(),
                    kind: UploadErrorKind::Server,
                    status_code,
                }
            }
        }
    }
}

impl VerifyBlob {
    async fn verify_blob(&self, input: Input) -> Result<bool, VerifyBlobError> {
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let is_verified = walrus_client
            .verify_blob(&input.blob_id)
            .await
            .map_err(VerifyBlobError::VerificationError)?;

        Ok(is_verified)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, nexus_sdk::walrus::WalrusClient, serde_json::json};

    // Override verify_blob method for testing
    impl VerifyBlob {
        // Helper method for testing
        fn with_custom_client() -> Self {
            Self
        }

        async fn verify_blob_for_test(
            &self,
            input: Input,
            client: WalrusClient,
        ) -> Result<bool, VerifyBlobError> {
            let is_verified = client
                .verify_blob(&input.blob_id)
                .await
                .map_err(VerifyBlobError::VerificationError)?;
            Ok(is_verified)
        }
    }

    async fn create_server_and_input() -> (mockito::ServerGuard, Input) {
        let server = Server::new_async().await;
        let server_url = server.url();

        // Set up test input with server URL
        let input = Input {
            blob_id: "test_blob_id".to_string(),
            aggregator_url: Some(server_url),
        };

        (server, input)
    }

    #[tokio::test]
    async fn test_verify_blob_true() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;
        let blob_id = input.blob_id.clone();

        // Set up mock response for successful verification
        let mock = server
            .mock("HEAD", "/v1/blobs/test_blob_id")
            .with_status(200)
            .create_async()
            .await;

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = VerifyBlob::with_custom_client();
        let result = match tool.verify_blob_for_test(input, walrus_client).await {
            Ok(verified) => {
                if verified {
                    Output::Verified { blob_id }
                } else {
                    Output::Unverified { blob_id }
                }
            }
            Err(e) => Output::Err {
                reason: e.to_string(),
                kind: UploadErrorKind::Server,
                status_code: None,
            },
        };

        // Verify the result
        match result {
            Output::Verified { blob_id: _ } => {
                // Test passed
            }
            Output::Unverified { blob_id: _ } => {
                panic!("Expected verification to be true");
            }
            Output::Err {
                reason,
                kind: _,
                status_code: _,
            } => {
                panic!("Expected Verified result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_verify_blob_false() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;
        let blob_id = input.blob_id.clone();

        // Set up mock response for failed verification
        let mock = server
            .mock("HEAD", "/v1/blobs/test_blob_id")
            .with_status(404)
            .create_async()
            .await;

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = VerifyBlob::with_custom_client();
        let result = match tool.verify_blob_for_test(input, walrus_client).await {
            Ok(verified) => {
                if verified {
                    Output::Verified { blob_id }
                } else {
                    Output::Unverified { blob_id }
                }
            }
            Err(e) => Output::Err {
                reason: e.to_string(),
                kind: UploadErrorKind::Server,
                status_code: None,
            },
        };

        // Verify the result
        match result {
            Output::Verified { blob_id: _ } => {
                panic!("Expected verification to be false");
            }
            Output::Unverified { blob_id: _ } => {
                // Test passed
            }
            Output::Err {
                reason,
                kind: _,
                status_code: _,
            } => {
                panic!("Expected UnVerified result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_verify_blob_error() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;
        let blob_id = input.blob_id.clone();

        // Set up mock response for error
        let mock = server
            .mock("HEAD", "/v1/blobs/test_blob_id")
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

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = VerifyBlob::with_custom_client();
        let result = match tool.verify_blob_for_test(input, walrus_client).await {
            Ok(verified) => {
                if verified {
                    Output::Verified { blob_id }
                } else {
                    Output::Unverified { blob_id }
                }
            }
            Err(e) => {
                let status_code = match &e {
                    VerifyBlobError::VerificationError(WalrusError::ApiError {
                        status_code,
                        ..
                    }) => Some(*status_code),
                    _ => None,
                };

                Output::Err {
                    reason: e.to_string(),
                    kind: UploadErrorKind::Server,
                    status_code,
                }
            }
        };

        // Verify the result
        match result {
            Output::Verified { blob_id: _ } => {
                panic!("Expected verification to be false for 500 error");
            }
            Output::Unverified { blob_id: _ } => {
                // Test passed
            }
            Output::Err {
                reason,
                kind: _,
                status_code: _,
            } => {
                panic!("Expected UnVerified result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
