//! # `xyz.taluslabs.storage.walrus.verify-blob@1`
//!
//! Standard Nexus Tool that verifies a blob.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    thiserror::Error,
};

#[derive(Error, Debug)]
pub enum VerifyBlobError {
    #[error("Failed to verify blob: {0}")]
    VerificationError(#[from] anyhow::Error),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    blob_id: String,
    #[serde(default)]
    aggregator_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { verified: bool },
    Err { reason: String },
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
        match self.verify_blob(input).await {
            Ok(verified) => Output::Ok { verified },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

impl VerifyBlob {
    async fn verify_blob(&self, input: Input) -> Result<bool, VerifyBlobError> {
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let is_verified = walrus_client.verify_blob(&input.blob_id).await?;

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
            let is_verified = client.verify_blob(&input.blob_id).await?;
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
            Ok(verified) => Output::Ok { verified },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        };

        // Verify the result
        match result {
            Output::Ok { verified } => {
                assert!(verified, "Expected verification to be true");
            }
            Output::Err { reason } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_verify_blob_false() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;

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
            Ok(verified) => Output::Ok { verified },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        };

        // Verify the result
        match result {
            Output::Ok { verified } => {
                assert!(!verified, "Expected verification to be false");
            }
            Output::Err { reason } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_verify_blob_error() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;

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
            Ok(verified) => Output::Ok { verified },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        };

        // Verify the result
        match result {
            Output::Ok { verified } => {
                assert!(!verified, "Expected verification to be false for 500 error");
            }
            Output::Err { reason } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }
}
