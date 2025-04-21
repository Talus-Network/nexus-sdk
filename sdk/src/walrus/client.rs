use {
    crate::walrus::models::*,
    anyhow::{anyhow, Context, Result},
    futures_util::StreamExt,
    reqwest::Client,
    serde::{de::DeserializeOwned, Serialize},
    std::path::PathBuf,
    tokio::{fs::File, io::AsyncWriteExt},
};

// Publisher and Aggregator URLs are from https://github.com/MystenLabs/walrus/blob/232d27ff7b3c2ba08aa4e10729b095f300b46384/docs/book/assets/operators.json
// Walrus API Endpoints
pub const WALRUS_PUBLISHER_URL: &str = "https://publisher.walrus-testnet.walrus.space";
pub const WALRUS_AGGREGATOR_URL: &str = "https://aggregator.walrus-testnet.walrus.space";

/// Client for interacting with the Walrus decentralized blob storage system
pub struct WalrusClient {
    client: Client,
}

impl WalrusClient {
    /// Create a new WalrusClient with default configuration
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Create a new WalrusClient with a custom reqwest Client
    pub fn with_client(client: Client) -> Self {
        Self { client }
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
        let file_content = tokio::fs::read(file_path)
            .await
            .context(format!("Failed to read file: {:?}", file_path))?;

        // Construct API URL with query parameters
        let mut url = format!("{}/v1/blobs?epochs={}", WALRUS_PUBLISHER_URL, epochs);
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
            .context("Failed to upload file")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Upload failed: {}", error_text));
        }

        let storage_info: StorageInfo = response.json().await?;
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
        let json_content = serde_json::to_vec(data).context("Failed to serialize data to JSON")?;

        // Construct API URL with query parameters
        let mut url = format!("{}/v1/blobs?epochs={}", WALRUS_PUBLISHER_URL, epochs);
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
            .context("Failed to upload JSON data")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Upload failed: {}", error_text));
        }

        let storage_info: StorageInfo = response.json().await?;
        Ok(storage_info)
    }

    /// Download a file from Walrus
    ///
    /// # Arguments
    /// * `blob_id` - The blob ID of the file to download
    /// * `output` - Path where the downloaded file should be saved
    pub async fn download_file(&self, blob_id: &str, output: &PathBuf) -> Result<()> {
        // Construct download URL
        let url = format!("{}/v1/blobs/{}", WALRUS_AGGREGATOR_URL, blob_id);

        // Send GET request
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to download blob")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Download failed: {}", error_text));
        }

        // Stream the response body to file
        let mut file = File::create(output).await?;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;
        }

        Ok(())
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
        let url = format!("{}/v1/blobs/{}", WALRUS_AGGREGATOR_URL, blob_id);

        // Send GET request
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to download JSON blob")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Download failed: {}", error_text));
        }

        // Parse the JSON response
        let json_data = response
            .json::<T>()
            .await
            .context("Failed to parse JSON data")?;

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
        let url = format!("{}/v1/blobs/{}", WALRUS_AGGREGATOR_URL, blob_id);

        // Send HEAD request to check if blob exists
        let response = self
            .client
            .head(&url)
            .send()
            .await
            .context("Failed to verify blob existence")?;

        Ok(response.status().is_success())
    }
}
