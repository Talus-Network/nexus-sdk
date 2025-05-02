//! # `xyz.taluslabs.storage.walrus.upload-json@1`
//!
//! Standard Nexus Tool that uploads a JSON file to Walrus and returns the blob ID.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, walrus::StorageInfo, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    thiserror::Error,
};

/// Errors that can occur during JSON upload
#[derive(Error, Debug)]
pub enum UploadJsonError {
    #[error("Failed to upload JSON: {0}")]
    UploadError(#[from] anyhow::Error),
    #[error("Invalid JSON data: {0}")]
    InvalidJson(String),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The JSON data to upload
    json: String,
    /// The walrus publisher URL
    #[serde(default)]
    publisher_url: Option<String>,
    /// The URL of the aggregator to upload the JSON to
    #[serde(default)]
    aggregator_url: Option<String>,
    /// Number of epochs to store the data
    #[serde(default = "default_epochs")]
    epochs: u64,
    /// Optional address to which the created Blob object should be sent
    #[serde(default)]
    send_to_address: Option<String>,
}

fn default_epochs() -> u64 {
    1
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    AlreadyCertified {
        blob_id: String,
        end_epoch: u64,
        tx_digest: String,
    },
    NewlyCreated {
        blob_id: String,
        end_epoch: u64,
        sui_object_id: String,
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

/// Types of errors that can occur during JSON upload
#[derive(Serialize, JsonSchema, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum UploadErrorKind {
    /// Error during network request
    Network,
    /// Error validating JSON
    Validation,
}

pub(crate) struct UploadJson;

impl NexusTool for UploadJson {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.storage.walrus.upload-json@1")
    }

    fn path() -> &'static str {
        "/json/upload"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        match self.upload(input).await {
            Ok(storage_info) => {
                if let Some(ac) = &storage_info.already_certified {
                    Output::AlreadyCertified {
                        blob_id: ac.blob_id.clone(),
                        end_epoch: ac.end_epoch,
                        tx_digest: ac.event.tx_digest.clone(),
                    }
                } else {
                    let created_blob = storage_info.newly_created.unwrap();

                    Output::NewlyCreated {
                        blob_id: created_blob.blob_object.blob_id,
                        end_epoch: created_blob.blob_object.storage.end_epoch,
                        sui_object_id: created_blob.blob_object.id,
                    }
                }
            }
            Err(e) => {
                let (kind, status_code) = match &e {
                    UploadJsonError::InvalidJson(_) => (UploadErrorKind::Validation, None),
                    UploadJsonError::UploadError(err) => {
                        let status_code = err
                            .to_string()
                            .split("status ")
                            .nth(1)
                            .and_then(|s| s.split(':').next())
                            .and_then(|s| s.trim().parse::<u16>().ok());

                        (UploadErrorKind::Network, status_code)
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

impl UploadJson {
    async fn upload(&self, input: Input) -> Result<StorageInfo, UploadJsonError> {
        // Validate JSON before proceeding
        serde_json::from_str::<serde_json::Value>(&input.json)
            .map_err(|e| UploadJsonError::InvalidJson(e.to_string()))?;

        let walrus_client = WalrusConfig::new()
            .with_publisher_url(input.publisher_url)
            .with_aggregator_url(input.aggregator_url)
            .build();

        let storage_info = walrus_client
            .upload_json(&input.json, input.epochs, input.send_to_address)
            .await?;

        Ok(storage_info)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, nexus_sdk::walrus::WalrusClient, serde_json::json};

    // Override upload method for testing
    impl UploadJson {
        // Helper method for testing
        fn with_custom_client() -> Self {
            Self {}
        }

        async fn upload_for_test(
            &self,
            input: Input,
            client: WalrusClient,
        ) -> Result<StorageInfo, UploadJsonError> {
            // Validate JSON before proceeding
            serde_json::from_str::<serde_json::Value>(&input.json)
                .map_err(|e| UploadJsonError::InvalidJson(e.to_string()))?;

            let storage_info = client
                .upload_json(&input.json, input.epochs, input.send_to_address)
                .await?;

            Ok(storage_info)
        }
    }

    async fn create_server_and_input() -> (mockito::ServerGuard, Input) {
        let server = Server::new_async().await;
        let server_url = server.url();

        // Create test JSON data
        let json_data = json!({
            "name": "test",
            "value": 123
        })
        .to_string();

        // Set up test input with server URL
        let input = Input {
            json: json_data,
            publisher_url: Some(server_url.clone()),
            aggregator_url: Some(server_url),
            epochs: 1,
            send_to_address: None,
        };

        (server, input)
    }

    #[tokio::test]
    async fn test_upload_json_newly_created() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;

        // Set up mock response for newly created blob
        let mock = server
            .mock("PUT", "/v1/blobs")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "newlyCreated": {
                        "blobObject": {
                            "blobId": "test_blob_id",
                            "id": "test_object_id",
                            "storage": {
                                "endEpoch": 100
                            }
                        }
                    },
                    "alreadyCertified": null
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_publisher_url(Some(server.url()))
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = UploadJson::with_custom_client();
        let result = match tool.upload_for_test(input, walrus_client).await {
            Ok(storage_info) => {
                println!("storage_info: {:?}", storage_info);
                if let Some(nc) = &storage_info.newly_created {
                    Output::NewlyCreated {
                        blob_id: nc.blob_object.blob_id.clone(),
                        end_epoch: nc.blob_object.storage.end_epoch,
                        sui_object_id: nc.blob_object.id.clone(),
                    }
                } else if let Some(ac) = &storage_info.already_certified {
                    Output::AlreadyCertified {
                        blob_id: ac.blob_id.clone(),
                        end_epoch: ac.end_epoch,
                        tx_digest: ac.event.tx_digest.clone(),
                    }
                } else {
                    panic!("Neither newly_created nor already_certified is Some");
                }
            }
            Err(e) => Output::Err {
                reason: e.to_string(),
                kind: UploadErrorKind::Network,
                status_code: None,
            },
        };

        // Verify the result
        match result {
            Output::NewlyCreated {
                blob_id,
                end_epoch,
                sui_object_id,
            } => {
                assert_eq!(blob_id, "test_blob_id");
                assert_eq!(end_epoch, 100);
                assert_eq!(sui_object_id, "test_object_id");
            }
            Output::AlreadyCertified { .. } => {
                panic!("Expected NewlyCreated result, got AlreadyCertified");
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                panic!(
                    "Expected NewlyCreated result, got error: {} (kind: {:?}, status: {:?})",
                    reason, kind, status_code
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_upload_json_already_certified() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;

        // Set up mock response for already certified blob
        let mock = server
            .mock("PUT", "/v1/blobs")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "newlyCreated": null,
                    "alreadyCertified": {
                        "blobId": "certified_blob_id",
                        "endEpoch": 200,
                        "event": {
                            "txDigest": "certified_tx_digest",
                            "timestampMs": 12345678,
                            "suiAddress": "sui_address"
                        }
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_publisher_url(Some(server.url()))
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = UploadJson::with_custom_client();
        let result = match tool.upload_for_test(input, walrus_client).await {
            Ok(storage_info) => {
                println!("storage_info: {:?}", storage_info);
                if let Some(nc) = &storage_info.newly_created {
                    Output::NewlyCreated {
                        blob_id: nc.blob_object.blob_id.clone(),
                        end_epoch: nc.blob_object.storage.end_epoch,
                        sui_object_id: nc.blob_object.id.clone(),
                    }
                } else if let Some(ac) = &storage_info.already_certified {
                    Output::AlreadyCertified {
                        blob_id: ac.blob_id.clone(),
                        end_epoch: ac.end_epoch,
                        tx_digest: ac.event.tx_digest.clone(),
                    }
                } else {
                    panic!("Neither newly_created nor already_certified is Some");
                }
            }
            Err(e) => Output::Err {
                reason: e.to_string(),
                kind: UploadErrorKind::Network,
                status_code: None,
            },
        };

        // Verify the result
        match result {
            Output::NewlyCreated { .. } => {
                panic!("Expected AlreadyCertified result, got NewlyCreated");
            }
            Output::AlreadyCertified {
                blob_id,
                end_epoch,
                tx_digest,
            } => {
                assert_eq!(blob_id, "certified_blob_id");
                assert_eq!(end_epoch, 200);
                assert_eq!(tx_digest, "certified_tx_digest");
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                panic!(
                    "Expected AlreadyCertified result, got error: {} (kind: {:?}, status: {:?})",
                    reason, kind, status_code
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_upload_json_error() {
        // Create server and input
        let (mut server, input) = create_server_and_input().await;

        // Set up mock response for error
        let mock = server
            .mock("PUT", "/v1/blobs")
            .match_query(mockito::Matcher::Any)
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
            .with_publisher_url(Some(server.url()))
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client
        let tool = UploadJson::with_custom_client();
        let result = tool.upload_for_test(input, walrus_client).await;

        // Verify the result is an error
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("status 500") || error_message.contains("server error"),
            "Error message '{}' should contain 'status 500' or 'server error'",
            error_message
        );

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_upload_invalid_json() {
        // Create test input with invalid JSON
        let input = Input {
            json: "this is not valid json".to_string(),
            publisher_url: None,
            aggregator_url: None,
            epochs: 1,
            send_to_address: None,
        };

        // Call the tool
        let tool = UploadJson::with_custom_client();
        let result = tool.invoke(input).await;

        // Verify the result
        match result {
            Output::NewlyCreated { .. } | Output::AlreadyCertified { .. } => {
                panic!("Expected error result, got success");
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(reason.contains("Invalid JSON"));
                assert_eq!(kind, UploadErrorKind::Validation);
                assert_eq!(status_code, None);
            }
        }
    }
}
