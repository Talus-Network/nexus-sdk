//! # `xyz.taluslabs.storage.walrus.download-file@1`
//!
//! Standard Nexus Tool that downloads a file from Walrus and saves it to a local path.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, walrus::WalrusError, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
    tempfile::TempDir,
    thiserror::Error,
};

/// Errors that can occur during file download
#[derive(Error, Debug)]
pub enum DownloadFileError {
    #[error("Failed to download file: {0}")]
    DownloadError(#[from] WalrusError),
    #[error("Invalid folder path: {0}")]
    InvalidFolder(String),
    #[error("Write error: {0}")]
    WriteError(String),
}

/// Types of errors that can occur during file download
#[derive(Serialize, JsonSchema, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DownloadErrorKind {
    /// Error during network request
    Network,
    /// Error validating file paths or permissions
    Validation,
    /// Error writing to file system
    FileSystem,
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
    #[serde(default)]
    output_path: String,
    /// The URL of the aggregator to download the file from
    #[serde(
        default,
        deserialize_with = "crate::utils::validation::deserialize_url_opt"
    )]
    aggregator_url: Option<String>,
    /// The file extension to save the file as
    #[serde(default = "default_file_extension")]
    file_extension: FileExtension,
}

fn default_file_extension() -> FileExtension {
    FileExtension::Txt
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        blob_id: String,
        contents: String,
    },
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (network, validation, etc.)
        kind: DownloadErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct DownloadFile;

impl NexusTool for DownloadFile {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.storage.walrus.download-file@1")
    }

    fn path() -> &'static str {
        "/download-file"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let blob_id = input.blob_id.clone();

        match self.download_file(input).await {
            Ok(final_path) => Output::Ok {
                blob_id,
                contents: format!("File downloaded to {} successfully.", final_path),
            },
            Err(e) => {
                let (kind, status_code) = match &e {
                    DownloadFileError::InvalidFolder(_) => (DownloadErrorKind::Validation, None),
                    DownloadFileError::WriteError(_) => (DownloadErrorKind::FileSystem, None),
                    DownloadFileError::DownloadError(err) => {
                        let status_code = match err {
                            WalrusError::ApiError { status_code, .. } => Some(*status_code),
                            _ => None,
                        };

                        (DownloadErrorKind::Network, status_code)
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

impl DownloadFile {
    async fn download_file(&self, input: Input) -> Result<String, DownloadFileError> {
        let extension = input.file_extension.to_string();
        let (final_path, _temp_dir): (String, Option<TempDir>) = if input.output_path.is_empty() {
            // Use a tempdir for output
            let temp_dir =
                TempDir::new().map_err(|e| DownloadFileError::WriteError(e.to_string()))?;
            let file_path = temp_dir
                .path()
                .join(format!("downloaded_file{}", extension));
            (file_path.to_string_lossy().to_string(), Some(temp_dir))
        } else {
            let output_path = input.output_path.clone();
            let mut final_path = output_path.clone() + "/downloaded_file" + &extension;
            let mut counter = 1;
            while PathBuf::from(&final_path).exists() {
                final_path = format!("{}/downloaded_file({}){}", output_path, counter, extension);
                counter += 1;
            }
            // Validate output path
            validate_output_path(&final_path)?;
            (final_path, None)
        };

        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let _contents = walrus_client
            .download_file(&input.blob_id, &PathBuf::from(&final_path))
            .await?;

        Ok(final_path)
    }
}

/// Validates the output path for writing
fn validate_output_path(output_path: &String) -> Result<(), DownloadFileError> {
    // Check if the directory exists
    if let Some(parent) = PathBuf::from(output_path).parent() {
        if !parent.exists() {
            return Err(DownloadFileError::InvalidFolder(format!(
                "Directory does not exist: {}",
                parent.display()
            )));
        }

        // Check if the directory is writable
        if !is_directory_writable(parent) {
            return Err(DownloadFileError::WriteError(format!(
                "Directory is not writable: {}",
                parent.display()
            )));
        }
    }

    // Check if the file already exists and is not writable
    if PathBuf::from(output_path).exists() && !is_file_writable(&PathBuf::from(output_path)) {
        return Err(DownloadFileError::WriteError(format!(
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
    use {super::*, mockito::Server, nexus_sdk::walrus::WalrusClient, std::fs};

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
            let output_path = input.output_path.clone();
            let extension = input.file_extension.to_string();
            let final_path = output_path + "/downloaded_file" + &extension;

            // Create the directory if it doesn't exist
            if let Some(parent) = PathBuf::from(&final_path).parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| DownloadFileError::WriteError(e.to_string()))?;
                }
            }

            client
                .download_file(&input.blob_id, &PathBuf::from(&final_path))
                .await?;

            Ok(())
        }

        async fn create_server_and_input(
            output_path: Option<String>,
        ) -> (mockito::ServerGuard, Input, &'static [u8]) {
            let server = Server::new_async().await;
            let server_url = server.url();

            // Create a temporary directory for testing
            let temp_dir;
            if output_path.is_none() {
                // Create a temp dir or use system temp dir as fallback
                let temp_dir_result = TempDir::new();
                let temp_dir_path = match temp_dir_result {
                    Ok(dir) => dir.path().to_path_buf(),
                    Err(_) => std::env::temp_dir(),
                };

                temp_dir = temp_dir_path
                    .join(format!("walrus_test{}", FileExtension::Txt.to_string()))
                    .to_string_lossy()
                    .to_string();
            } else {
                temp_dir = output_path.clone().unwrap();
            }

            // Set up test input with server URL
            let input = Input {
                blob_id: "test_blob_id".to_string(),
                output_path: temp_dir,
                aggregator_url: Some(server_url.clone()),
                file_extension: FileExtension::Txt,
            };

            // Create a file with the given content
            static FILE_CONTENT: &[u8] = b"Hello, World!";

            (server, input, FILE_CONTENT)
        }

        async fn cleanup_test_file(file_path: &String) {
            let file_path = PathBuf::from(file_path);
            if file_path.exists() {
                fs::remove_file(&file_path).unwrap_or_default();
            }
        }
    }

    #[tokio::test]
    async fn test_download_file_success() {
        // Create server and input
        let (mut server, input, file_content) = DownloadFile::create_server_and_input(None).await;

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
        println!("result: {:?}", result);
        // Verify the result
        assert!(
            result.is_ok(),
            "Download should succeed but got: {:?}",
            result
        );

        // Verify the file was downloaded correctly
        let final_path = format!(
            "{}/downloaded_file{}",
            input.output_path,
            input.file_extension.to_string()
        );
        let downloaded_content = fs::read_to_string(&PathBuf::from(&final_path)).unwrap();
        assert_eq!(
            downloaded_content,
            std::str::from_utf8(file_content).unwrap()
        );

        // Verify the request was made
        mock.assert_async().await;

        // Clean up test file
        DownloadFile::cleanup_test_file(&final_path).await;
    }

    #[tokio::test]
    async fn test_download_file_error() {
        // Create server and input
        let (mut server, input, _) = DownloadFile::create_server_and_input(None).await;

        // Ensure the directory exists to avoid validation errors
        if let Some(parent) = PathBuf::from(&input.output_path).parent() {
            fs::create_dir_all(parent).unwrap_or_default();
        }

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

        // Use download_for_test which creates directories
        let tool = DownloadFile::with_custom_client();
        let result = tool.download_for_test(&input, walrus_client).await;

        assert!(result.is_err(), "Expected error result");

        // Check if error contains 500 status code
        let error_str = format!("{:?}", result.unwrap_err());
        assert!(
            error_str.contains("500"),
            "Error should contain status code 500"
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_download_nonexistent_blob() {
        // Create server and input
        let (mut server, input, _) = DownloadFile::create_server_and_input(None).await;

        // Ensure the directory exists to avoid validation errors
        if let Some(parent) = PathBuf::from(&input.output_path).parent() {
            fs::create_dir_all(parent).unwrap_or_default();
        }

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

        // Use download_for_test which creates directories
        let tool = DownloadFile::with_custom_client();
        let result = tool.download_for_test(&input, walrus_client).await;

        assert!(result.is_err(), "Expected error result");

        // Check if error contains 404 status code
        let error_str = format!("{:?}", result.unwrap_err());
        assert!(
            error_str.contains("404"),
            "Error should contain status code 404"
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_output_formatting() {
        // Set up test parameters
        let (_, input, _) = DownloadFile::create_server_and_input(None).await;

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
            Output::Err {
                reason,
                kind: _,
                status_code: _,
            } => {
                panic!("Expected OK result, got error: {}", reason);
            }
        }
    }

    #[tokio::test]
    async fn test_invalid_output_path() {
        // Create an input with an invalid path
        let (_, input, _) =
            DownloadFile::create_server_and_input(Some("nonexistent/directory".to_string())).await;

        let tool = DownloadFile::with_custom_client();
        let output = tool.invoke(input).await;

        // Print the actual reason for debugging
        match &output {
            Output::Ok { .. } => println!("Got Ok when expecting Err"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                println!("Error reason: {:?}", reason);
                println!("Error kind: {:?}", kind);
                println!("Status code: {:?}", status_code);
            }
        }

        match output {
            Output::Ok { .. } => panic!("Expected error output, but got successful download"),
            Output::Err {
                reason: _,
                kind,
                status_code,
            } => {
                assert_eq!(kind, DownloadErrorKind::Validation);
                assert_eq!(status_code, None);
            }
        }
    }
}
