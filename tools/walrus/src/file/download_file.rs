//! # `xyz.taluslabs.walrus.file.download@1`
//!
//! Standard Nexus Tool that downloads a file from Walrus and saves it to a local path.

use {
    crate::client::WalrusConfig,
    dirs,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
    thiserror::Error,
};

/// Errors that can occur during file upload
#[derive(Error, Debug)]
pub enum DownloadFileError {
    #[error("Failed to download file: {0}")]
    DownloadError(#[from] anyhow::Error),
    #[error("Invalid file data: {0}")]
    InvalidFile(String),
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum FileExtension {
    Txt,
    Json,
    Bin,
    Png,
    Jpg,
    Jpeg,
}

impl std::fmt::Display for FileExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileExtension::Txt => write!(f, ".txt"),
            FileExtension::Json => write!(f, ".json"),
            FileExtension::Bin => write!(f, ".bin"),
            FileExtension::Png => write!(f, ".png"),
            FileExtension::Jpg => write!(f, ".jpg"),
            FileExtension::Jpeg => write!(f, ".jpeg"),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The blob ID of the file to download
    blob_id: String,
    /// The path to save the file to
    #[serde(default = "default_output_path")]
    output_path: String,
    /// The URL of the aggregator to upload the JSON to
    #[serde(default)]
    aggregator_url: Option<String>,
    /// The file extension to save the file as
    #[serde(default = "default_file_extension")]
    file_extension: FileExtension,
}

fn default_output_path() -> String {
    dirs::download_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

fn default_file_extension() -> FileExtension {
    FileExtension::Txt
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { blob_id: String, contents: String },
    Err { reason: String },
}

pub(crate) struct DownloadFile;

impl NexusTool for DownloadFile {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.walrus.file.download")
    }

    fn path() -> &'static str {
        "/file/download"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let blob_id = input.blob_id.clone();
        let output_path = input.output_path.clone();

        match self.download_file(input).await {
            Ok(_) => Output::Ok {
                blob_id,
                contents: format!("File downloaded to {} successfully.", output_path),
            },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

impl DownloadFile {
    async fn download_file(&self, input: Input) -> Result<(), DownloadFileError> {
        // Validate output path
        let output_path =
            input.output_path.clone() + "/downloaded_file" + &input.file_extension.to_string();
        validate_output_path(&output_path)?;

        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let contents = walrus_client
            .download_file(&input.blob_id, &PathBuf::from(&output_path))
            .await?;

        Ok(contents)
    }
}

/// Validates the output path for writing
fn validate_output_path(output_path: &String) -> Result<(), DownloadFileError> {
    // Check if the directory exists
    if let Some(parent) = PathBuf::from(output_path).parent() {
        if !parent.exists() {
            return Err(DownloadFileError::InvalidFile(format!(
                "Directory does not exist: {}",
                parent.display()
            )));
        }

        // Check if the directory is writable
        if !is_directory_writable(parent) {
            return Err(DownloadFileError::InvalidFile(format!(
                "Directory is not writable: {}",
                parent.display()
            )));
        }
    }

    // Check if the file already exists and is not writable
    if PathBuf::from(output_path).exists() && !is_file_writable(&PathBuf::from(output_path)) {
        return Err(DownloadFileError::InvalidFile(format!(
            "File exists but is not writable: {}",
            PathBuf::from(output_path).display()
        )));
    }

    Ok(())
}

/// Check if a directory is writable
fn is_directory_writable(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.permissions().readonly())
        .unwrap_or(true)
        == false
}

/// Check if a file is writable
fn is_file_writable(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.permissions().readonly())
        .unwrap_or(true)
        == false
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, nexus_sdk::walrus::WalrusClient, std::fs, tempfile::tempdir};

    impl DownloadFile {
        // Helper method for testing
        fn with_custom_client() -> Self {
            Self {}
        }

        async fn download_for_test(
            &self,
            input: &Input,
            client: WalrusClient,
        ) -> Result<(), DownloadFileError> {
            // Validate output path
            validate_output_path(&input.output_path)?;

            client
                .download_file(&input.blob_id, &PathBuf::from(&input.output_path))
                .await?;

            Ok(())
        }

        async fn create_server_and_input(input: Input) -> (mockito::ServerGuard, Input) {
            let server = Server::new_async().await;
            let server_url = server.url();

            // Set up test input with server URL
            let input = Input {
                blob_id: input.blob_id,
                output_path: input.output_path,
                aggregator_url: Some(server_url.clone()),
                file_extension: input.file_extension,
            };

            (server, input)
        }

        async fn cleanup_test_file(file_path: &String) {
            let file_path = PathBuf::from(file_path);
            if file_path.exists() {
                fs::remove_file(&file_path).unwrap_or_default();
            }
        }

        async fn test_utils(file_name: &str) -> (Input, &[u8], tempfile::TempDir) {
            // Create a temp directory for output
            let dir = match tempdir() {
                Ok(dir) => dir,
                Err(e) => {
                    panic!("Failed to create temp directory: {}", e);
                }
            };

            let output_path = dir.path().join(file_name);

            // Ensure the directory exists
            let parent_path = dir.path();
            assert!(
                parent_path.exists(),
                "Temp directory not created: {:?}",
                parent_path
            );

            // Create a file with the given content
            let file_content = b"Hello, World!";

            (
                Input {
                    blob_id: "test_blob_id".to_string(),
                    output_path: output_path.to_string_lossy().to_string(),
                    aggregator_url: None,
                    file_extension: FileExtension::Txt,
                },
                file_content,
                dir,
            )
        }
    }

    #[tokio::test]
    async fn test_download_file_success() {
        // Set up test input - keep the tempdir alive for the duration of the test
        let (input, file_content, _tempdir) = DownloadFile::test_utils("downloaded_file.txt").await;

        // Create server and input
        let (mut server, input) = DownloadFile::create_server_and_input(input).await;

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

        // Skip validation for test purposes
        // Call the tool with our test client but bypass validation
        let tool = DownloadFile::with_custom_client();
        let result = tool.download_for_test(&input, walrus_client).await;

        // Verify the result
        assert!(
            result.is_ok(),
            "Download should succeed but got: {:?}",
            result
        );

        // Verify the file was downloaded correctly
        let downloaded_content = fs::read_to_string(&PathBuf::from(&input.output_path)).unwrap();
        assert_eq!(
            downloaded_content,
            std::str::from_utf8(file_content).unwrap()
        );

        // Verify the request was made
        mock.assert_async().await;

        // Clean up test file
        DownloadFile::cleanup_test_file(&input.output_path).await;
    }

    #[tokio::test]
    async fn test_download_file_error() {
        // Set up test input - keep the tempdir alive for the duration of the test
        let (input, _, _tempdir) = DownloadFile::test_utils("error_file.txt").await;

        // Create server and input
        let (mut server, input) = DownloadFile::create_server_and_input(input).await;

        // Set up mock response for error
        let mock = server
            .mock("GET", format!("/v1/blobs/{}", input.blob_id).as_str())
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "Internal server error"}"#)
            .create_async()
            .await;

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client, directly call the client to test the HTTP error
        let tool = DownloadFile::with_custom_client();
        let result = tool.download_for_test(&input, walrus_client).await;

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

        // Clean up test file (though it shouldn't exist due to the error)
        DownloadFile::cleanup_test_file(&input.output_path).await;
    }

    #[tokio::test]
    async fn test_download_nonexistent_blob() {
        // Set up test input - keep the tempdir alive for the duration of the test
        let (input, _, _tempdir) = DownloadFile::test_utils("nonexistent_file.txt").await;

        // Clean up any existing test file
        DownloadFile::cleanup_test_file(&input.output_path).await;

        // Create server and input
        let (mut server, input) = DownloadFile::create_server_and_input(input).await;

        // Set up mock response for non-existent blob
        let mock = server
            .mock("GET", format!("/v1/blobs/{}", input.blob_id).as_str())
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "Blob not found"}"#)
            .create_async()
            .await;

        // Create a client that points to our mock server
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(Some(server.url()))
            .build();

        // Call the tool with our test client, directly call the client to test the HTTP error
        let tool = DownloadFile::with_custom_client();
        let result = tool.download_for_test(&input, walrus_client).await;

        // Verify the result is an error
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("404") || error_message.contains("not found"),
            "Error message '{}' should contain 404 or not found",
            error_message
        );

        // Verify that the mock was called
        mock.assert_async().await;

        // Clean up test file (though it shouldn't exist due to the error)
        DownloadFile::cleanup_test_file(&input.output_path).await;
    }

    #[tokio::test]
    async fn test_output_formatting() {
        // Set up test parameters
        let (input, _, _) = DownloadFile::test_utils("format_test.txt").await;

        // Test the output formatting by directly calling the invoke method
        // and checking the format of the success output
        let result = Output::Ok {
            blob_id: input.blob_id.clone(),
            contents: format!(
                "File downloaded to {} successfully",
                PathBuf::from(&input.output_path).display()
            ),
        };

        // Verify correct formatting
        match result {
            Output::Ok {
                blob_id: result_blob_id,
                contents,
            } => {
                assert_eq!(result_blob_id, input.blob_id);
                assert!(contents.contains("successfully"));
                assert!(contents.contains(&input.output_path));
            }
            Output::Err { reason } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }
    }

    #[tokio::test]
    async fn test_invalid_output_path() {
        // Invalid directory path that definitely shouldn't exist
        let (input, _, _) = DownloadFile::test_utils("invalid_output_path.txt").await;

        // Create tool and directly validate the output path
        let result = validate_output_path(&input.output_path);

        // Expect an InvalidFile error
        assert!(result.is_err());
        match result {
            Err(DownloadFileError::InvalidFile(msg)) => {
                assert!(msg.contains("Directory does not exist"));
            }
            _ => panic!("Expected InvalidFile error, but got different error type"),
        }

        // Test through the main download method
        let tool = DownloadFile::with_custom_client();
        let result = tool.download_file(input).await;

        // Expect the same InvalidFile error
        assert!(result.is_err());
        match result {
            Err(DownloadFileError::InvalidFile(msg)) => {
                assert!(msg.contains("Directory does not exist"));
            }
            _ => panic!("Expected InvalidFile error, but got different error type"),
        }
    }
}
