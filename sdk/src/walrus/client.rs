use {
    crate::walrus::models::*,
    futures_util::StreamExt,
    reqwest::Client,
    serde::{de::DeserializeOwned, Serialize},
    std::{io, path::PathBuf, time::Duration},
    thiserror::Error,
    tokio::{fs::File, io::AsyncWriteExt, time},
};

/// Publisher and Aggregator URLs are from <https://github.com/MystenLabs/walrus/blob/232d27ff7b3c2ba08aa4e10729b095f300b46384/docs/book/assets/operators.json>
/// Walrus Default API Endpoints
pub const WALRUS_PUBLISHER_URL: &str = "https://publisher.walrus-testnet.walrus.space";
pub const WALRUS_AGGREGATOR_URL: &str = "https://aggregator.walrus-testnet.walrus.space";

/// Maximum number of epochs Walrus allows for storing data
pub const WALRUS_MAX_EPOCHS: u8 = 53;

const WALRUS_RESPONSE_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
const WALRUS_RESPONSE_READ_TIMEOUT: Duration = Duration::from_secs(10);
const WALRUS_ERROR_RESPONSE_MAX_BYTES: usize = 4_096;

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

    /// Error serializing or parsing JSON data
    #[error("Failed to process JSON data: {0}")]
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

    /// An untrusted response exceeded the caller's byte budget.
    #[error("Walrus response exceeds the {max_bytes}-byte limit")]
    ResponseTooLarge {
        /// Maximum response bytes accepted by the caller.
        max_bytes: usize,
    },

    /// An untrusted response stopped making read progress.
    #[error("Walrus response made no read progress for {timeout:?}")]
    ResponseReadTimeout {
        /// Maximum interval between response chunks.
        timeout: Duration,
    },

    /// An untrusted request exceeded its end-to-end deadline.
    #[error("Walrus response exceeded the total deadline of {timeout:?}")]
    ResponseTotalTimeout {
        /// Maximum duration for the request and response body.
        timeout: Duration,
    },
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
        epochs: u8,
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

        self.upload_bytes(file_content, epochs, send_to).await
    }

    /// Upload JSON data to Walrus.
    ///
    /// This preserves the public API used by downstream callers while storing
    /// exactly the JSON byte representation produced by [`serde_json`].
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
        epochs: u8,
        send_to: Option<String>,
    ) -> Result<StorageInfo> {
        let json_content = serde_json::to_vec(data).map_err(WalrusError::SerializationError)?;

        let mut url = format!("{}/v1/blobs?epochs={}", self.publisher_url, epochs);
        if let Some(address) = send_to {
            url.push_str(&format!("&send_object_to={address}"));
        }

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

    /// Upload bytes to Walrus.
    ///
    /// # Arguments
    /// * `data` - Bytes to upload
    /// * `epochs` - Number of epochs to store the data
    /// * `send_to` - Optional address to which the created Blob object should be sent
    ///
    /// # Returns
    /// * `Result<StorageInfo>` - Information about the uploaded data
    pub async fn upload_bytes(
        &self,
        data: impl Into<Vec<u8>>,
        epochs: u8,
        send_to: Option<String>,
    ) -> Result<StorageInfo> {
        // Construct API URL with query parameters
        let mut url = format!("{}/v1/blobs?epochs={}", self.publisher_url, epochs);
        if let Some(address) = send_to {
            url.push_str(&format!("&send_object_to={address}"));
        }

        // Send PUT request with raw blob content
        let response = self
            .client
            .put(&url)
            .body(data.into())
            .send()
            .await
            .map_err(|e| WalrusError::RequestError {
                message: "Failed to upload bytes".to_string(),
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
        file.flush()
            .await
            .map_err(|e| WalrusError::FileWriteError {
                path: output.clone(),
                source: e,
            })?;

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

    /// Reads one blob under a caller-provided byte ceiling and repository deadlines.
    pub(crate) async fn read_file_bounded(
        &self,
        blob_id: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        let url = format!("{}/v1/blobs/{}", self.aggregator_url, blob_id);
        self.read_url_bounded(
            &url,
            max_bytes,
            WALRUS_RESPONSE_TOTAL_TIMEOUT,
            WALRUS_RESPONSE_READ_TIMEOUT,
        )
        .await
    }

    async fn read_url_bounded(
        &self,
        url: &str,
        max_bytes: usize,
        total_timeout: Duration,
        read_timeout: Duration,
    ) -> Result<Vec<u8>> {
        let read =
            async {
                let response = self.client.get(url).send().await.map_err(|source| {
                    WalrusError::RequestError {
                        message: "Failed to download blob".to_owned(),
                        source,
                    }
                })?;
                let status = response.status();
                let response_limit = if status.is_success() {
                    max_bytes
                } else {
                    WALRUS_ERROR_RESPONSE_MAX_BYTES
                };
                if response.content_length().is_some_and(|length| {
                    length > u64::try_from(response_limit).unwrap_or(u64::MAX)
                }) {
                    return Err(WalrusError::ResponseTooLarge {
                        max_bytes: response_limit,
                    });
                }

                let mut bytes = Vec::new();
                let mut stream = response.bytes_stream();
                loop {
                    let next = time::timeout(read_timeout, stream.next())
                        .await
                        .map_err(|_| WalrusError::ResponseReadTimeout {
                            timeout: read_timeout,
                        })?;
                    let Some(chunk) = next else {
                        break;
                    };
                    let chunk = chunk.map_err(WalrusError::StreamError)?;
                    if chunk.len() > response_limit.saturating_sub(bytes.len()) {
                        return Err(WalrusError::ResponseTooLarge {
                            max_bytes: response_limit,
                        });
                    }
                    bytes.extend_from_slice(&chunk);
                }

                if !status.is_success() {
                    return Err(WalrusError::ApiError {
                        status_code: status.as_u16(),
                        message: String::from_utf8_lossy(&bytes).into_owned(),
                    });
                }
                Ok(bytes)
            };

        time::timeout(total_timeout, read)
            .await
            .map_err(|_| WalrusError::ResponseTotalTimeout {
                timeout: total_timeout,
            })?
    }

    /// Download and parse JSON data from Walrus.
    ///
    /// # Arguments
    /// * `blob_id` - The blob ID of the JSON data to download
    ///
    /// # Returns
    /// * `Result<T>` - The parsed JSON data
    pub async fn read_json<T: DeserializeOwned>(&self, blob_id: &str) -> Result<T> {
        let bytes = self.read_file(blob_id).await?;
        serde_json::from_slice(&bytes).map_err(WalrusError::SerializationError)
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

#[cfg(test)]
mod bounded_read_tests {
    use {
        super::*,
        std::future::pending,
        tokio::{io::AsyncReadExt as _, net::TcpListener, task::JoinHandle, time::sleep},
    };

    struct ChunkedServer {
        url: String,
        task: JoinHandle<()>,
    }

    impl Drop for ChunkedServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn chunked_server(
        status: &'static str,
        chunks: Vec<(Duration, Vec<u8>)>,
        finish: bool,
    ) -> ChunkedServer {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server binds");
        let address = listener.local_addr().expect("test server has an address");
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("test client connects");
            let mut request = [0_u8; 1_024];
            let _ = socket.read(&mut request).await;
            let header = format!(
                "HTTP/1.1 {status}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            );
            if socket.write_all(header.as_bytes()).await.is_err() {
                return;
            }
            for (delay, chunk) in chunks {
                sleep(delay).await;
                let prefix = format!("{:x}\r\n", chunk.len());
                if socket.write_all(prefix.as_bytes()).await.is_err()
                    || socket.write_all(&chunk).await.is_err()
                    || socket.write_all(b"\r\n").await.is_err()
                {
                    return;
                }
            }
            if finish {
                let _ = socket.write_all(b"0\r\n\r\n").await;
            } else {
                pending::<()>().await;
            }
        });
        ChunkedServer {
            url: format!("http://{address}"),
            task,
        }
    }

    fn client(server: &ChunkedServer) -> WalrusClient {
        WalrusClient::builder()
            .with_aggregator_url(&server.url)
            .build()
    }

    #[tokio::test]
    async fn bounded_read_rejects_a_chunked_body_before_response_completion() {
        let server = chunked_server(
            "200 OK",
            vec![
                (Duration::ZERO, b"1234".to_vec()),
                (Duration::ZERO, b"5678".to_vec()),
            ],
            false,
        )
        .await;

        let error = client(&server)
            .read_file_bounded("oversized", 5)
            .await
            .expect_err("the response exceeds its byte budget");

        assert!(matches!(
            error,
            WalrusError::ResponseTooLarge { max_bytes: 5 }
        ));
    }

    #[tokio::test]
    async fn bounded_read_caps_error_response_bodies() {
        let oversized = vec![b'x'; WALRUS_ERROR_RESPONSE_MAX_BYTES + 1];
        let server = chunked_server(
            "500 Internal Server Error",
            vec![(Duration::ZERO, oversized)],
            false,
        )
        .await;

        let error = client(&server)
            .read_file_bounded("error", 1)
            .await
            .expect_err("the error response exceeds its byte budget");

        assert!(matches!(
            error,
            WalrusError::ResponseTooLarge {
                max_bytes: WALRUS_ERROR_RESPONSE_MAX_BYTES,
            }
        ));
    }

    #[tokio::test]
    async fn bounded_read_fails_when_the_body_stops_making_progress() {
        let server = chunked_server("200 OK", vec![], false).await;

        let error = client(&server)
            .read_url_bounded(
                &format!("{}/v1/blobs/stalled", server.url),
                10,
                Duration::from_millis(500),
                Duration::from_millis(30),
            )
            .await
            .expect_err("a stalled body must time out");

        assert!(matches!(error, WalrusError::ResponseReadTimeout { .. }));
    }

    #[tokio::test]
    async fn bounded_read_enforces_the_total_deadline_while_progress_continues() {
        let chunks = (0..10)
            .map(|_| (Duration::from_millis(20), b"x".to_vec()))
            .collect();
        let server = chunked_server("200 OK", chunks, true).await;

        let error = client(&server)
            .read_url_bounded(
                &format!("{}/v1/blobs/slow", server.url),
                100,
                Duration::from_millis(70),
                Duration::from_millis(50),
            )
            .await
            .expect_err("the progressing body must still meet the total deadline");

        assert!(matches!(error, WalrusError::ResponseTotalTimeout { .. }));
    }

    #[tokio::test]
    async fn bounded_read_accepts_content_within_the_budget() {
        let server = chunked_server(
            "200 OK",
            vec![
                (Duration::ZERO, b"bounded".to_vec()),
                (Duration::ZERO, b" body".to_vec()),
            ],
            true,
        )
        .await;

        let bytes = client(&server)
            .read_file_bounded("valid", 12)
            .await
            .expect("bounded content is accepted");

        assert_eq!(bytes, b"bounded body");
    }
}
