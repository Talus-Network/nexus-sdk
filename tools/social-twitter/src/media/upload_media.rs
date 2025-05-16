//! # `xyz.taluslabs.social.twitter.upload-media@1`
//!
//! Standard Nexus Tool that uploads media to Twitter.

use {
    super::MEDIA_UPLOAD_ENDPOINT,
    crate::{
        auth::TwitterAuth,
        error::{TwitterError, TwitterErrorKind, TwitterResult},
        media::models::{
            EmptyResponse,
            MediaCategory,
            MediaType,
            MediaUploadData,
            MediaUploadResponse,
        },
        twitter_client::{TwitterClient, TWITTER_X_API_BASE},
    },
    base64,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::multipart::{Form, Part},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

/// Input for media upload
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Twitter API credentials
    #[serde(flatten)]
    auth: TwitterAuth,

    /// The Base64 encoded media content
    media_data: String,

    /// The MIME type of the media being uploaded. For example, video/mp4.
    media_type: MediaType,

    /// A string enum value which identifies a media use-case.
    media_category: MediaCategory,

    /// A comma-separated list of user IDs to set as additional owners allowed to use the returned media_id.
    #[serde(default)]
    additional_owners: Vec<String>,

    /// Chunk size in bytes for uploading media (default: calculated based on media size and type)
    /// Set to 0 to use automatic calculation
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,

    /// If false, waits for media processing to complete before returning
    /// If true, returns immediately after upload (default: true)
    #[serde(default = "default_optimistic_upload")]
    optimistic_upload: bool,
}

fn default_chunk_size() -> usize {
    0 // 0 means auto-calculate based on media size and type
}

fn default_optimistic_upload() -> bool {
    true // By default, return immediately after upload
}

/// Output for media upload
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    /// Successful upload
    Ok {
        /// Media ID for use in tweets
        media_id: String,
        /// Media key
        media_key: String,
    },
    /// Upload error
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (network, server, auth, etc.)
        kind: TwitterErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

impl From<TwitterError> for Output {
    fn from(e: TwitterError) -> Self {
        let error_response = e.to_error_response();
        Output::Err {
            reason: error_response.reason,
            kind: error_response.kind,
            status_code: error_response.status_code,
        }
    }
}

impl From<base64::DecodeError> for Output {
    fn from(e: base64::DecodeError) -> Self {
        Output::Err {
            reason: format!("Failed to decode media data: {}", e),
            kind: TwitterErrorKind::Unknown,
            status_code: None,
        }
    }
}

impl From<crate::twitter_client::TwitterClientError> for Output {
    fn from(e: crate::twitter_client::TwitterClientError) -> Self {
        Output::Err {
            reason: e.to_string(),
            kind: TwitterErrorKind::Network,
            status_code: None,
        }
    }
}

pub(crate) struct UploadMedia {
    api_base: String,
}

impl NexusTool for UploadMedia {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: TWITTER_X_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.social.twitter.upload-media@1")
    }

    fn path() -> &'static str {
        "/upload-media"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Create a Twitter client with the mock server URL
        let client = match TwitterClient::new(Some(MEDIA_UPLOAD_ENDPOINT), Some(&self.api_base)) {
            Ok(client) => client,
            Err(e) => return e.into(),
        };

        // Decode base64 media data
        let media_data = match base64::decode(&request.media_data) {
            Ok(data) => data,
            Err(e) => return e.into(),
        };

        // Upload media using chunking process
        match upload_media(
            &client,
            &request.auth,
            &media_data,
            &request.media_type,
            &request.media_category,
            request.chunk_size,
            if request.additional_owners.is_empty() {
                None
            } else {
                Some(&request.additional_owners)
            },
            request.optimistic_upload,
        )
        .await
        {
            Ok(response) => Output::Ok {
                media_id: response.id,
                media_key: response.media_key,
            },
            Err(e) => e.into(),
        }
    }
}

/// Upload media to Twitter in chunks
async fn upload_media(
    client: &TwitterClient,
    auth: &TwitterAuth,
    media_data: &[u8],
    media_type: &MediaType,
    media_category: &MediaCategory,
    chunk_size: usize,
    additional_owners: Option<&Vec<String>>,
    optimistic_upload: bool,
) -> TwitterResult<MediaUploadData> {
    // Calculate optimal chunk size if not specified
    let optimal_chunk_size = if chunk_size > 0 {
        chunk_size
    } else {
        calculate_optimal_chunk_size(media_data.len(), media_type, media_category)
    };

    // Validate number of chunks doesn't exceed Twitter's API limit of 999
    let total_chunks = (media_data.len() + optimal_chunk_size - 1) / optimal_chunk_size;
    if total_chunks > 999 {
        return Err(TwitterError::Other(format!(
            "Media would require {} chunks, which exceeds Twitter's limit of 999.",
            total_chunks
        )));
    }

    // 1. INIT phase - Initialize upload
    let init_response = init_upload(
        &client,
        auth,
        media_data.len() as u32,
        media_type,
        media_category,
        additional_owners,
    )
    .await?;

    let media_id = init_response.id.clone();

    // 2. APPEND phase - Upload chunks
    let chunks = media_data.chunks(optimal_chunk_size).enumerate();

    for (i, chunk) in chunks {
        append_chunk(&client, auth, &media_id, chunk, i as i32).await?;
    }

    // 3. FINALIZE phase - Complete the upload
    let mut finalize_result = finalize_upload(&client, auth, &media_id).await?;

    // 4. STATUS phase - Wait for processing to complete if not optimistic
    if !optimistic_upload {
        // Check if media requires processing and is not already completed
        if let Some(processing_info) = &finalize_result.processing_info {
            if processing_info.state != super::models::ProcessingState::Succeeded {
                // Wait for processing to complete
                finalize_result = wait_for_processing_completion(client, auth, &media_id).await?;
            }
        }
    }

    Ok(finalize_result)
}

/// Calculate optimal chunk size based on media size and type
fn calculate_optimal_chunk_size(
    media_size: usize,
    media_type: &MediaType,
    media_category: &MediaCategory,
) -> usize {
    // Size units
    const KB: usize = 1024;
    const MB: usize = 1024 * 1024;

    // Twitter API's maximum chunk size is 5MB as per documentation
    // https://developer.x.com/en/docs/x-api/v1/media/upload-media/api-reference/post-media-upload-append
    const MAX_CHUNK_SIZE: usize = 5 * MB; // 5MB

    // Minimum chunk size to avoid too many requests
    const MIN_CHUNK_SIZE: usize = 128 * KB; // 128KB

    // Size thresholds
    const LARGE_VIDEO_THRESHOLD: usize = 20 * MB; // 20MB
    const SMALL_FILE_THRESHOLD: usize = 10 * MB; // 10MB
    const MEDIUM_FILE_THRESHOLD: usize = 50 * MB; // 50MB

    // Chunk sizes
    const SMALLER_VIDEO_CHUNK_SIZE: usize = 4 * MB; // 4MB
    const GIF_CHUNK_SIZE: usize = 3 * MB; // 3MB
    const MAX_IMAGE_CHUNK_SIZE: usize = 2 * MB; // 2MB

    // Ideal chunk counts
    const SMALL_FILE_CHUNK_COUNT: usize = 8;
    const MEDIUM_FILE_CHUNK_COUNT: usize = 15;
    const LARGE_FILE_CHUNK_COUNT: usize = 25;

    // Rounding unit
    const CHUNK_SIZE_ALIGNMENT: usize = 128 * KB; // 128KB

    // For very small files, use a single chunk if possible
    if media_size <= MAX_CHUNK_SIZE {
        // Use the full file size if it's under the maximum allowed chunk size
        // This reduces overhead with multiple requests
        return media_size;
    }

    // Twitter doc mentions optimizing for cellular clients, so we set reasonable limits
    // For images (typically smaller and faster to upload)
    if *media_type == MediaType::ImageJpeg
        || *media_type == MediaType::ImageGif
        || *media_type == MediaType::ImagePng
        || *media_type == MediaType::ImageWebp
    {
        // For larger images, still keep chunks reasonably sized
        return std::cmp::min(media_size / 4, MAX_IMAGE_CHUNK_SIZE); // Max 2MB chunks for images
    }

    // For GIFs, which can be larger but still image-based
    if *media_type == MediaType::ImageGif || matches!(media_category, MediaCategory::TweetGif) {
        // Balance between speed and reliability
        return GIF_CHUNK_SIZE; // 3MB for GIFs
    }

    // For videos, which are usually much larger files
    if *media_type == MediaType::VideoMp4
        || *media_type == MediaType::VideoWebm
        || *media_type == MediaType::VideoMp2t
        || matches!(
            media_category,
            MediaCategory::TweetVideo | MediaCategory::DmVideo | MediaCategory::AmplifyVideo
        )
    {
        // For large videos, use max chunk size for better upload efficiency
        // The docs mention that larger chunks are better for stable connections
        if media_size > LARGE_VIDEO_THRESHOLD {
            return MAX_CHUNK_SIZE; // 5MB for large videos
        } else {
            return SMALLER_VIDEO_CHUNK_SIZE; // 4MB for smaller videos
        }
    }

    // Calculate optimal number of chunks based on file size
    // Aim for a reasonable number of chunks to balance reliability and performance
    let ideal_chunk_count = if media_size < SMALL_FILE_THRESHOLD {
        // For files < 10MB, aim for ~8 chunks
        SMALL_FILE_CHUNK_COUNT
    } else if media_size < MEDIUM_FILE_THRESHOLD {
        // For files between 10MB and 50MB, aim for ~15 chunks
        MEDIUM_FILE_CHUNK_COUNT
    } else {
        // For larger files, aim for ~25 chunks
        LARGE_FILE_CHUNK_COUNT
    };

    // Calculate chunk size based on ideal chunk count
    let calculated_size = media_size / ideal_chunk_count;

    // Ensure the calculated size is within bounds and round to nearest 128KB
    let chunk_size = std::cmp::min(
        MAX_CHUNK_SIZE,
        std::cmp::max(MIN_CHUNK_SIZE, calculated_size),
    );

    // Round to nearest 128KB for efficiency
    (chunk_size / CHUNK_SIZE_ALIGNMENT) * CHUNK_SIZE_ALIGNMENT
}

/// Initialize a media upload (INIT command)
async fn init_upload(
    client: &TwitterClient,
    auth: &TwitterAuth,
    total_bytes: u32,
    media_type: &MediaType,
    media_category: &MediaCategory,
    additional_owners: Option<&Vec<String>>,
) -> TwitterResult<MediaUploadData> {
    // Required parameters
    let form = Form::new()
        .text("command", "INIT")
        .text("total_bytes", total_bytes.to_string())
        .text("media_type", media_type.to_string())
        .text("media_category", media_category.to_string())
        .text(
            "additional_owners",
            additional_owners
                .map(|owners| owners.join(","))
                .unwrap_or_default(),
        );

    client
        .post::<MediaUploadResponse, ()>(auth, None, Some(form))
        .await
        .map_err(|e| {
            TwitterError::ApiError(
                e.reason,
                format!("{:?}", e.kind),
                e.status_code
                    .map(|code| code.to_string())
                    .unwrap_or_default(),
            )
        })
}

/// Append a chunk to the upload (APPEND command)
async fn append_chunk(
    client: &TwitterClient,
    auth: &TwitterAuth,
    media_id: &str,
    chunk: &[u8],
    segment_index: i32,
) -> TwitterResult<()> {
    // Validate segment_index is within allowed range (0-999)
    if segment_index < 0 || segment_index > 999 {
        return Err(TwitterError::Other(format!(
            "Invalid segment_index: {}. Must be between 0 and 999",
            segment_index
        )));
    }

    // Create part for the media chunk
    let part = Part::bytes(chunk.to_vec()).file_name("media.bin"); // Generic filename, doesn't matter

    // Create form with APPEND command as per Twitter API docs
    let form = Form::new()
        .text("command", "APPEND")
        .text("media_id", media_id.to_string())
        .text("segment_index", segment_index.to_string())
        .part("media", part);

    // Send request and handle empty response (HTTP 2XX)
    client
        .post::<EmptyResponse, ()>(auth, None, Some(form))
        .await
        .map_err(|e| {
            TwitterError::ApiError(
                e.reason,
                format!("{:?}", e.kind),
                e.status_code
                    .map(|code| code.to_string())
                    .unwrap_or_default(),
            )
        })
}

/// Finalize the media upload (FINALIZE command)
async fn finalize_upload(
    client: &TwitterClient,
    auth: &TwitterAuth,
    media_id: &str,
) -> TwitterResult<MediaUploadData> {
    // Create form with FINALIZE command
    let form = Form::new()
        .text("command", "FINALIZE")
        .text("media_id", media_id.to_string());

    client
        .post::<MediaUploadResponse, ()>(auth, None, Some(form))
        .await
        .map_err(|e| {
            TwitterError::ApiError(
                e.reason,
                format!("{:?}", e.kind),
                e.status_code
                    .map(|code| code.to_string())
                    .unwrap_or_default(),
            )
        })
}

/// Wait for media processing to complete
async fn wait_for_processing_completion(
    client: &TwitterClient,
    auth: &TwitterAuth,
    media_id: &str,
) -> TwitterResult<MediaUploadData> {
    // Maximum number of attempts
    const MAX_ATTEMPTS: u32 = 20;
    let mut attempts = 0;

    loop {
        // Check status
        let status = check_media_status(client, auth, media_id).await?;

        if let Some(processing_info) = &status.processing_info {
            match processing_info.state {
                super::models::ProcessingState::Succeeded => {
                    // Processing completed successfully
                    return Ok(status);
                }
                super::models::ProcessingState::Failed => {
                    // Processing failed
                    return Err(TwitterError::Other("Media processing failed".to_string()));
                }
                _ => {
                    // Still processing
                    attempts += 1;
                    if attempts >= MAX_ATTEMPTS {
                        return Err(TwitterError::Other(
                            "Media processing timed out".to_string(),
                        ));
                    }

                    // Wait for the recommended time or default to 2 seconds
                    let wait_secs = processing_info.check_after_secs.unwrap_or(2);
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs as u64)).await;
                }
            }
        } else {
            // No processing info means it's ready
            return Ok(status);
        }
    }
}

/// Check media upload status
async fn check_media_status(
    client: &TwitterClient,
    auth: &TwitterAuth,
    media_id: &str,
) -> TwitterResult<MediaUploadData> {
    // Create form for STATUS command
    let form = Form::new()
        .text("command", "STATUS")
        .text("media_id", media_id.to_string());

    client
        .post::<MediaUploadResponse, ()>(auth, None, Some(form))
        .await
        .map_err(|e| {
            TwitterError::ApiError(
                e.reason,
                format!("{:?}", e.kind),
                e.status_code
                    .map(|code| code.to_string())
                    .unwrap_or_default(),
            )
        })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::media::models::ProcessingState, mockito::Server, serde_json::json};

    impl UploadMedia {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, UploadMedia, TwitterClient) {
        let server = Server::new_async().await;
        let tool = UploadMedia::with_api_base(&server.url());
        let client = TwitterClient::new(Some("media/upload"), Some(&server.url())).unwrap();

        (server, tool, client)
    }

    fn create_test_input() -> Input {
        Input {
            auth: TwitterAuth::new(
                "test_consumer_key",
                "test_consumer_secret",
                "test_access_token",
                "test_access_token_secret",
            ),
            media_data: "SGVsbG8gV29ybGQ=".to_string(), // "Hello World" as base64
            media_type: MediaType::ImageJpeg,
            media_category: MediaCategory::TweetImage,
            additional_owners: vec![],
            chunk_size: 1024,
            optimistic_upload: true,
        }
    }

    #[tokio::test]
    async fn test_invalid_base64() {
        // Create server and tool
        let (_, tool, _) = create_server_and_tool().await;

        // Create input with invalid base64
        let mut input = create_test_input();
        input.media_data = "Invalid Base64 Data!!!".to_string();

        // Test the media upload
        let result = tool.invoke(input).await;

        // Verify the response is an error
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(
                    reason.contains("Failed to decode media data"),
                    "Error message should indicate base64 decode failure, got: {}",
                    reason
                );
                assert_eq!(kind, TwitterErrorKind::Unknown);
                assert!(status_code.is_none());
            }
        }
    }

    #[tokio::test]
    async fn test_init_failure() {
        // Create server and tool
        let (mut server, _, client) = create_server_and_tool().await;
        let input = create_test_input();

        // Set up mock for INIT failure
        let mock = server
            .mock("POST", "/media/upload")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "title": "Invalid Request",
                        "type": "invalid_request",
                        "detail": "Media category is required",
                        "status": 400
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Call init_upload directly
        let result = init_upload(
            &client,
            &input.auth,
            1024,
            &input.media_type,
            &input.media_category,
            None,
        )
        .await;

        // Verify the response is an error
        assert!(result.is_err(), "Expected error, got success: {:?}", result);
        if let Err(e) = result {
            assert!(
                e.to_string().contains("Twitter API error"),
                "Error message should indicate init failure, got: {}",
                e
            );
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    // Using a simpler testing approach - test each function individually rather than the full flow

    #[tokio::test]
    async fn test_init_upload() {
        // Create a client and server
        let (mut server, _, client) = create_server_and_tool().await;
        let input = create_test_input();

        // Set up mock for INIT
        let mock = server
            .mock("POST", "/media/upload")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "id": "12345678901234567890",
                        "media_key": "12_12345678901234567890",
                        "expires_after_secs": 3600
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Call init_upload directly
        let result = init_upload(
            &client,
            &input.auth,
            1024,
            &input.media_type,
            &input.media_category,
            None,
        )
        .await;

        // Verify success
        assert!(
            result.is_ok(),
            "Expected init_upload to succeed: {:?}",
            result
        );
        if let Ok(response) = result {
            assert_eq!(response.id, "12345678901234567890");
            assert_eq!(response.media_key, "12_12345678901234567890");
        }

        // Verify the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_append_chunk() {
        // Create a client and server
        let (mut server, _, client) = create_server_and_tool().await;
        let input = create_test_input();

        // Set up mock for APPEND
        let mock = server
            .mock("POST", "/media/upload")
            .with_status(204) // APPEND returns 204 No Content
            .with_header("content-type", "application/json")
            .with_body("") // Empty body for 204 response
            .create_async()
            .await;

        // Call append_chunk directly
        let result = append_chunk(
            &client,
            &input.auth,
            "12345678901234567890",
            "Hello World".as_bytes(),
            0,
        )
        .await;

        // Verify success
        assert!(
            result.is_ok(),
            "Expected append_chunk to succeed: {:?}",
            result
        );

        // Verify the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_finalize_upload() {
        // Create a client and server
        let (mut server, _, client) = create_server_and_tool().await;
        let input = create_test_input();

        // Set up mock for FINALIZE
        let mock = server
            .mock("POST", "/media/upload")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "id": "12345678901234567890",
                        "media_key": "12_12345678901234567890",
                        "processing_info": {
                            "state": "succeeded",
                            "progress_percent": 100
                        }
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Call finalize_upload directly
        let result = finalize_upload(&client, &input.auth, "12345678901234567890").await;

        // Verify success
        assert!(
            result.is_ok(),
            "Expected finalize_upload to succeed: {:?}",
            result
        );
        if let Ok(response) = result {
            assert_eq!(response.id, "12345678901234567890");
            assert_eq!(response.media_key, "12_12345678901234567890");
            assert!(response.processing_info.is_some());
            assert_eq!(
                response.processing_info.unwrap().state,
                ProcessingState::Succeeded
            );
        }

        // Verify the mock was called
        mock.assert_async().await;
    }
}
