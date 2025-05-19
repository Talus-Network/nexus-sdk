//! # `xyz.taluslabs.storage.walrus.upload-file@1`
//!
//! Standard Nexus Tool that uploads a file to Walrus and returns the blob ID.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{
        fqn,
        walrus::{StorageInfo, WalrusError},
        ToolFqn,
    },
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
    UploadError(#[from] WalrusError),
    #[error("Invalid file data: {0}")]
    InvalidFile(String),
}

/// Types of errors that can occur during file upload
#[derive(Serialize, JsonSchema, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum UploadErrorKind {
    /// Error during network request
    Network,
    /// Error validating file
    Validation,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The path to the file to upload
    file_path: String,
    /// The walrus publisher URL
    #[serde(
        default,
        deserialize_with = "crate::utils::validation::deserialize_url_opt"
    )]
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

pub(crate) struct UploadFile;

impl NexusTool for UploadFile {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.storage.walrus.upload-file@1")
    }

    fn path() -> &'static str {
        "/upload-file"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        match self.upload(input).await {
            Ok(storage_info) => handle_successful_upload(storage_info),
            Err(e) => {
                let (kind, status_code) = match &e {
                    UploadFileError::InvalidFile(_) => (UploadErrorKind::Validation, None),
                    UploadFileError::UploadError(err) => {
                        let status_code = match err {
                            WalrusError::ApiError { status_code, .. } => Some(*status_code),
                            _ => None,
                        };

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

/// Handles the successful upload case by extracting the blob ID from the storage info
fn handle_successful_upload(storage_info: StorageInfo) -> Output {
    if let Some(newly_created) = storage_info.newly_created {
        Output::NewlyCreated {
            blob_id: newly_created.blob_object.blob_id,
            end_epoch: newly_created.blob_object.storage.end_epoch,
            sui_object_id: newly_created.blob_object.id,
        }
    } else if let Some(already_certified) = storage_info.already_certified {
        Output::AlreadyCertified {
            blob_id: already_certified.blob_id,
            end_epoch: already_certified.end_epoch,
            tx_digest: already_certified.event.tx_digest,
        }
    } else {
        Output::Err {
            reason: "Neither newly created nor already certified".to_string(),
            kind: UploadErrorKind::Validation,
            status_code: None,
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
            .await
            .map_err(UploadFileError::UploadError)?;

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
                .await
                .map_err(UploadFileError::UploadError)?;

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
                assert_eq!(reason, "Neither newly created nor already certified");
                assert_eq!(kind, UploadErrorKind::Validation);
                assert_eq!(status_code, None);
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
                assert_eq!(reason, "Neither newly created nor already certified");
                assert_eq!(kind, UploadErrorKind::Validation);
                assert_eq!(status_code, None);
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
        let output = match tool.upload_for_test(input, walrus_client).await {
            Ok(storage_info) => handle_successful_upload(storage_info),
            Err(e) => {
                let (kind, status_code) = match &e {
                    UploadFileError::InvalidFile(_) => (UploadErrorKind::Validation, None),
                    UploadFileError::UploadError(err) => {
                        let status_code = match err {
                            WalrusError::ApiError { status_code, .. } => Some(*status_code),
                            _ => None,
                        };

                        (UploadErrorKind::Network, status_code)
                    }
                };

                Output::Err {
                    reason: e.to_string(),
                    kind,
                    status_code,
                }
            }
        };

        // Verify the result
        match output {
            Output::NewlyCreated { .. } | Output::AlreadyCertified { .. } => {
                panic!("Expected error, but got success");
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(reason.contains("500") || reason.contains("server error"));
                assert_eq!(kind, UploadErrorKind::Network);
                assert_eq!(status_code, Some(500));
            }
        }

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
            Output::NewlyCreated { .. } | Output::AlreadyCertified { .. } => {
                panic!("Expected error result, got success");
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(reason.contains("File does not exist"));
                assert_eq!(kind, UploadErrorKind::Validation);
                assert_eq!(status_code, None);
            }
        }
    }
}
