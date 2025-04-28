//! # `xyz.taluslabs.walrus.file.upload@1`
//!
//! Standard Nexus Tool that uploads a file to Walrus and returns the blob ID.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, walrus::StorageInfo, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
    thiserror::Error,
};

/// Errors that can occur during file upload
#[derive(Error, Debug)]
pub enum UploadFileError {
    #[error("Failed to upload file: {0}")]
    UploadError(#[from] anyhow::Error),
    #[error("Invalid file data: {0}")]
    InvalidFile(String),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The path to the file to upload
    file_path: String,
    /// The walrus publisher URL
    #[serde(default)]
    publisher_url: Option<String>,
    /// The number of epochs to store the file
    #[serde(default = "default_epochs")]
    epochs: u64,
    /// Optional address to which the created Blob object should be sent
    #[serde(default)]
    send_to: Option<String>,
}

fn default_epochs() -> u64 {
    1
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        blob_id: String,
        end_epoch: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        newly_created: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        already_certified: Option<bool>,
        // if the blob is already certified, this will be the tx digest of the blob object
        #[serde(skip_serializing_if = "Option::is_none")]
        tx_digest: Option<String>,
        // if the blob is newly created, this will be the sui object ID of the blob object
        #[serde(skip_serializing_if = "Option::is_none")]
        sui_object_id: Option<String>,
    },
    Err {
        reason: String,
    },
}

pub(crate) struct UploadFile;

impl NexusTool for UploadFile {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.walrus.file.upload")
    }

    fn path() -> &'static str {
        "/file/upload"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        match self.upload(input).await {
            Ok(storage_info) => handle_successful_upload(storage_info),
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

/// Handles the successful upload case by extracting the blob ID from the storage info
fn handle_successful_upload(storage_info: StorageInfo) -> Output {
    if let Some(newly_created) = storage_info.newly_created {
        Output::Ok {
            already_certified: None,
            blob_id: newly_created.blob_object.blob_id,
            end_epoch: newly_created.blob_object.storage.end_epoch,
            newly_created: Some(true),
            sui_object_id: Some(newly_created.blob_object.id),
            tx_digest: None,
        }
    } else if let Some(already_certified) = storage_info.already_certified {
        Output::Ok {
            already_certified: Some(true),
            blob_id: already_certified.blob_id,
            end_epoch: already_certified.end_epoch,
            newly_created: None,
            sui_object_id: None,
            tx_digest: Some(already_certified.event.tx_digest),
        }
    } else {
        Output::Err {
            reason: "Neither newly created nor already certified".to_string(),
        }
    }
}

fn validate_file_path(file_path: &str) -> Result<(), UploadFileError> {
    let file_path = PathBuf::from(file_path);
    if !file_path.exists() {
        return Err(UploadFileError::InvalidFile(format!(
            "File does not exist: {}",
            file_path.display()
        )));
    }
    Ok(())
}

impl UploadFile {
    async fn upload(&self, input: Input) -> Result<StorageInfo, UploadFileError> {
        // Validate file path
        validate_file_path(&input.file_path)?;

        let walrus_client = WalrusConfig::new()
            .with_publisher_url(input.publisher_url)
            .build();

        let storage_info = walrus_client
            .upload_file(
                &PathBuf::from(&input.file_path),
                input.epochs,
                input.send_to,
            )
            .await?;

        Ok(storage_info)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, nexus_sdk::walrus::WalrusClient, serde_json::json};

    // Override upload method for testing
    impl UploadFile {
        // Helper method for testing
        fn with_custom_client() -> Self {
            Self {}
        }

        async fn upload_for_test(
            &self,
            input: Input,
            client: WalrusClient,
        ) -> Result<StorageInfo, UploadFileError> {
            // Validate file path
            validate_file_path(&input.file_path)?;

            let storage_info = client
                .upload_file(
                    &PathBuf::from(&input.file_path),
                    input.epochs,
                    input.send_to,
                )
                .await?;

            Ok(storage_info)
        }

        async fn create_server_and_input(file_path: &str) -> (mockito::ServerGuard, Input) {
            let server = Server::new_async().await;
            let server_url = server.url();

            // Set up test input with server URL
            let input = Input {
                file_path: file_path.to_string(),
                publisher_url: Some(server_url.clone()),
                epochs: 1,
                send_to: None,
            };

            (server, input)
        }

        async fn create_test_file(file_path: &str, file_content: &str) {
            let file_path = PathBuf::from(file_path);
            std::fs::write(&file_path, file_content).unwrap();
        }

        async fn remove_test_file(file_path: &str) {
            let file_path = PathBuf::from(file_path);
            std::fs::remove_file(&file_path).unwrap();
        }
    }

    #[tokio::test]
    async fn test_upload_file_newly_created() {
        // Create test file
        let file_path = "test.txt";
        UploadFile::create_test_file(file_path, "test").await;

        // Create server and input
        let (mut server, input) = UploadFile::create_server_and_input(file_path).await;

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
            .build();

        // Call the tool with our test client
        let tool = UploadFile::with_custom_client();
        let result = match tool.upload_for_test(input, walrus_client).await {
            Ok(storage_info) => handle_successful_upload(storage_info),
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        };

        // Verify the result
        match result {
            Output::Ok {
                blob_id,
                newly_created,
                already_certified,
                end_epoch,
                sui_object_id,
                tx_digest,
            } => {
                assert_eq!(blob_id, "test_blob_id");
                assert_eq!(newly_created, Some(true));
                assert_eq!(already_certified, None);
                assert_eq!(end_epoch, 100);
                assert_eq!(sui_object_id, Some("test_object_id".to_string()));
                assert_eq!(tx_digest, None);
            }
            Output::Err { reason } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;

        // Clean up test file
        UploadFile::remove_test_file("test.txt").await;
    }

    #[tokio::test]
    async fn test_upload_file_already_certified() {
        // Create test file
        let file_path = "test_already_certified.txt";
        UploadFile::create_test_file(file_path, "test").await;

        // Create server and input
        let (mut server, input) = UploadFile::create_server_and_input(file_path).await;

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
            .build();

        // Call the tool with our test client
        let tool = UploadFile::with_custom_client();
        let result = match tool.upload_for_test(input, walrus_client).await {
            Ok(storage_info) => handle_successful_upload(storage_info),
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        };

        // Verify the result
        match result {
            Output::Ok {
                blob_id,
                newly_created,
                already_certified,
                end_epoch,
                sui_object_id,
                tx_digest,
            } => {
                assert_eq!(blob_id, "certified_blob_id");
                assert_eq!(newly_created, None);
                assert_eq!(already_certified, Some(true));
                assert_eq!(end_epoch, 200);
                assert_eq!(sui_object_id, None);
                assert_eq!(tx_digest, Some("certified_tx_digest".to_string()));
            }
            Output::Err { reason } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;

        // Clean up test file
        UploadFile::remove_test_file("test_already_certified.txt").await;
    }

    #[tokio::test]
    async fn test_upload_file_error() {
        // Create test file
        let file_path = "test_error.txt";
        UploadFile::create_test_file(file_path, "test").await;

        // Create server and input
        let (mut server, input) = UploadFile::create_server_and_input(file_path).await;

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
            .build();

        // Call the tool with our test client
        let tool = UploadFile::with_custom_client();
        let result = tool.upload_for_test(input, walrus_client).await;

        // Verify the result is an error
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("500") || error_message.contains("server error"),
            "Error message '{}' should contain 500 or server error",
            error_message
        );

        // Verify that the mock was called
        mock.assert_async().await;

        // Clean up test file
        UploadFile::remove_test_file("test_error.txt").await;
    }

    #[tokio::test]
    async fn test_upload_invalid_file() {
        // Create test input with non-existent file path
        let file_path = "non_existent_file.txt";
        let input = Input {
            file_path: file_path.to_string(),
            publisher_url: None,
            epochs: 1,
            send_to: None,
        };

        // Call the tool
        let tool = UploadFile::with_custom_client();
        let result = tool.invoke(input).await;

        // Verify the result
        match result {
            Output::Ok { .. } => {
                panic!("Expected error result, got OK");
            }
            Output::Err { reason } => {
                assert!(reason.contains("File does not exist"));
            }
        }
    }
}
