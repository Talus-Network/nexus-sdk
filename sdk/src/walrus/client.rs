use {
    crate::walrus::models::*,
    futures_util::StreamExt,
    reqwest::Client,
    serde::{de::DeserializeOwned, Serialize},
    std::{io, path::PathBuf},
    thiserror::Error,
    tokio::{fs::File, io::AsyncWriteExt},
};

// Publisher and Aggregator URLs are from <https://github.com/MystenLabs/walrus/blob/232d27ff7b3c2ba08aa4e10729b095f300b46384/docs/book/assets/operators.json>
// Walrus Default API Endpoints
pub const WALRUS_PUBLISHER_URL: &str = "https://publisher.walrus-testnet.walrus.space";
pub const WALRUS_AGGREGATOR_URL: &str = "https://aggregator.walrus-testnet.walrus.space";

/// Errors that can occur when interacting with the Walrus API
#[derive(Error, Debug)]
pub enum WalrusError {
    /// Error reading file from disk
    #[error("Failed to read file: {path:?}, error: {source}")]
    FileReadError {
        /// Path to the file that failed to be read
        path: PathBuf,
        /// The underlying IO error
        #[source]
        source: io::Error,
    },

    /// Error creating or writing to a file
    #[error("Failed to write to file: {path:?}, error: {source}")]
    FileWriteError {
        /// Path to the file that failed to be written
        path: PathBuf,
        /// The underlying IO error
        #[source]
        source: io::Error,
    },

    /// Error serializing data to JSON
    #[error("Failed to serialize data to JSON: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Error during HTTP request
    #[error("HTTP request failed: {message}")]
    RequestError {
        /// Error message
        message: String,
        /// The underlying reqwest error
        #[source]
        source: reqwest::Error,
    },

    /// Error from API response
    #[error("API error: {status_code} - {message}")]
    ApiError {
        /// HTTP status code
        status_code: u16,
        /// Error message from API
        message: String,
    },

    /// Error processing stream data
    #[error("Failed to process data stream: {0}")]
    StreamError(#[from] reqwest::Error),
}

/// Result type used throughout the Walrus client
pub type Result<T> = std::result::Result<T, WalrusError>;

/// Builder for WalrusClient configuration
pub struct WalrusClientBuilder {
    client: Client,
    publisher_url: String,
    aggregator_url: String,
}

impl Default for WalrusClientBuilder {
    /// Creates a default WalrusClientBuilder with standard configuration
    fn default() -> Self {
        Self {
            client: Client::new(),
            publisher_url: WALRUS_PUBLISHER_URL.to_string(),
            aggregator_url: WALRUS_AGGREGATOR_URL.to_string(),
        }
    }
}

impl WalrusClientBuilder {
    /// Create a new WalrusClientBuilder with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom HTTP client
    pub fn with_client(mut self, client: Client) -> Self {
        self.client = client;
        self
    }

    /// Set a custom publisher URL
    pub fn with_publisher_url(mut self, url: &str) -> Self {
        self.publisher_url = url.to_string();
        self
    }

    /// Set a custom aggregator URL
    pub fn with_aggregator_url(mut self, url: &str) -> Self {
        self.aggregator_url = url.to_string();
        self
    }

    /// Build the WalrusClient with the configured settings
    pub fn build(self) -> WalrusClient {
        WalrusClient {
            client: self.client,
            publisher_url: self.publisher_url,
            aggregator_url: self.aggregator_url,
        }
    }
}

/// Client for interacting with the Walrus decentralized blob storage system
pub struct WalrusClient {
    client: Client,
    publisher_url: String,
    aggregator_url: String,
}

impl Default for WalrusClient {
    /// Creates a default WalrusClient with standard configuration
    fn default() -> Self {
        WalrusClientBuilder::default().build()
    }
}

impl WalrusClient {
    /// Create a new WalrusClient with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a builder to create a customized WalrusClient
    pub fn builder() -> WalrusClientBuilder {
        WalrusClientBuilder::default()
    }

    /// Upload a file to Walrus
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to upload
    /// * `epochs` - Number of epochs to store the file
    /// * `send_to` - Optional address to which the created Blob object should be sent
    ///
    /// # Returns
    /// * `Result<StorageInfo>` - Information about the uploaded file
    pub async fn upload_file(
        &self,
        file_path: &PathBuf,
        epochs: u64,
        send_to: Option<String>,
    ) -> Result<StorageInfo> {
        // Read file content
        let file_content =
            tokio::fs::read(file_path)
                .await
                .map_err(|e| WalrusError::FileReadError {
                    path: file_path.clone(),
                    source: e,
                })?;

        // Construct API URL with query parameters
        let mut url = format!("{}/v1/blobs?epochs={}", self.publisher_url, epochs);
        if let Some(address) = send_to {
            url.push_str(&format!("&send_object_to={}", address));
        }

        // Send PUT request
        let response = self
            .client
            .put(&url)
            .body(file_content)
            .send()
            .await
            .map_err(|e| WalrusError::RequestError {
                message: "Failed to upload file".to_string(),
                source: e,
            })?;

        if !response.status().is_success() {
            let status_code = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(WalrusError::ApiError {
                status_code,
                message: error_text,
            });
        }

        let storage_info =
            response
                .json::<StorageInfo>()
                .await
                .map_err(|e| WalrusError::RequestError {
                    message: "Failed to parse response".to_string(),
                    source: e,
                })?;

        Ok(storage_info)
    }

    /// Upload JSON data to Walrus
    ///
    /// # Arguments
    /// * `data` - Data to serialize as JSON and upload
    /// * `epochs` - Number of epochs to store the data
    /// * `send_to` - Optional address to which the created Blob object should be sent
    ///
    /// # Returns
    /// * `Result<StorageInfo>` - Information about the uploaded data
    pub async fn upload_json<T: Serialize>(
        &self,
        data: &T,
        epochs: u64,
        send_to: Option<String>,
    ) -> Result<StorageInfo> {
        // Serialize data to JSON
        let json_content = serde_json::to_vec(data).map_err(WalrusError::SerializationError)?;

        // Construct API URL with query parameters
        let mut url = format!("{}/v1/blobs?epochs={}", self.publisher_url, epochs);
        if let Some(address) = send_to {
            url.push_str(&format!("&send_object_to={}", address));
        }

        // Send PUT request with JSON content
        let response = self
            .client
            .put(&url)
            .header("Content-Type", "application/json")
            .body(json_content)
            .send()
            .await
            .map_err(|e| WalrusError::RequestError {
                message: "Failed to upload JSON data".to_string(),
                source: e,
            })?;

        if !response.status().is_success() {
            let status_code = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(WalrusError::ApiError {
                status_code,
                message: error_text,
            });
        }

        let storage_info =
            response
                .json::<StorageInfo>()
                .await
                .map_err(|e| WalrusError::RequestError {
                    message: "Failed to parse response".to_string(),
                    source: e,
                })?;

        Ok(storage_info)
    }

    /// Download a file from Walrus
    ///
    /// # Arguments
    /// * `blob_id` - The blob ID of the file to download
    /// * `output` - Path where the downloaded file should be saved
    pub async fn download_file(&self, blob_id: &str, output: &PathBuf) -> Result<()> {
        // Construct download URL
        let url = format!("{}/v1/blobs/{}", self.aggregator_url, blob_id);

        // Send GET request
        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| WalrusError::RequestError {
                    message: "Failed to download blob".to_string(),
                    source: e,
                })?;

        if !response.status().is_success() {
            let status_code = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(WalrusError::ApiError {
                status_code,
                message: error_text,
            });
        }

        // Stream the response body to file
        let mut file = File::create(output)
            .await
            .map_err(|e| WalrusError::FileWriteError {
                path: output.clone(),
                source: e,
            })?;

        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(WalrusError::StreamError)?;
            file.write_all(&chunk)
                .await
                .map_err(|e| WalrusError::FileWriteError {
                    path: output.clone(),
                    source: e,
                })?;
        }

        Ok(())
    }

    /// Download a file from Walrus and return its contents as bytes
    ///
    /// # Arguments
    /// * `blob_id` - The blob ID of the file to download
    ///
    /// # Returns
    /// * `Result<Vec<u8>>` - The file content as bytes
    pub async fn read_file(&self, blob_id: &str) -> Result<Vec<u8>> {
        // Construct download URL
        let url = format!("{}/v1/blobs/{}", self.aggregator_url, blob_id);

        // Send GET request
        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| WalrusError::RequestError {
                    message: "Failed to download blob".to_string(),
                    source: e,
                })?;

        if !response.status().is_success() {
            let status_code = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(WalrusError::ApiError {
                status_code,
                message: error_text,
            });
        }

        // Get the bytes directly from the response
        let bytes = response
            .bytes()
            .await
            .map_err(|e| WalrusError::RequestError {
                message: "Failed to read response bytes".to_string(),
                source: e,
            })?;

        Ok(bytes.to_vec())
    }

    /// Download and parse JSON data from Walrus
    ///
    /// # Arguments
    /// * `blob_id` - The blob ID of the JSON data to download
    ///
    /// # Returns
    /// * `Result<T>` - The parsed JSON data
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize the JSON into, must implement DeserializeOwned
    pub async fn read_json<T: DeserializeOwned>(&self, blob_id: &str) -> Result<T> {
        // Construct download URL
        let url = format!("{}/v1/blobs/{}", self.aggregator_url, blob_id);

        // Send GET request
        let response =
            self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| WalrusError::RequestError {
                    message: "Failed to download JSON blob".to_string(),
                    source: e,
                })?;

        if !response.status().is_success() {
            let status_code = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(WalrusError::ApiError {
                status_code,
                message: error_text,
            });
        }

        // Parse the JSON response
        let json_data = response
            .json::<T>()
            .await
            .map_err(|e| WalrusError::RequestError {
                message: "Failed to parse JSON data".to_string(),
                source: e,
            })?;

        Ok(json_data)
    }

    /// Verify if a blob exists in the Walrus network
    ///
    /// # Arguments
    /// * `blob_id` - The blob ID to verify
    ///
    /// # Returns
    /// * `Result<bool>` - True if the blob exists, false otherwise
    pub async fn verify_blob(&self, blob_id: &str) -> Result<bool> {
        // Construct URL to check blob existence
        let url = format!("{}/v1/blobs/{}", self.aggregator_url, blob_id);

        // Send HEAD request to check if blob exists
        let response =
            self.client
                .head(&url)
                .send()
                .await
                .map_err(|e| WalrusError::RequestError {
                    message: "Failed to verify blob existence".to_string(),
                    source: e,
                })?;

        Ok(response.status().is_success())
    }
}
