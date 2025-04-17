//! # `xyz.taluslabs.social.twitter.upload-media@1`
//!
//! Standard Nexus Tool that uploads media to Twitter.

use {
    crate::{
        auth::TwitterAuth,
        error::{parse_twitter_response, TwitterError, TwitterResult},
        media::{
            models::{MediaCategory, MediaUploadResponse, ProcessingInfo},
            MEDIA_UPLOAD_ENDPOINT,
        },
        tweet::TWITTER_API_BASE,
    },
    base64,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::{
        multipart::{Form, Part},
        Client,
    },
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
    media_type: String,

    /// A string enum value which identifies a media use-case.
    media_category: MediaCategory,

    /// A comma-separated list of user IDs to set as additional owners allowed to use the returned media_id.
    #[serde(default)]
    additional_owners: Vec<String>,

    /// Chunk size in bytes for uploading media (default: calculated based on media size and type)
    /// Set to 0 to use automatic calculation
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
}

fn default_chunk_size() -> usize {
    0 // 0 means auto-calculate based on media size and type
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
        /// Processing information if available
        #[serde(skip_serializing_if = "Option::is_none")]
        processing_info: Option<ProcessingInfo>,
    },
    /// Upload error
    Err {
        /// Error message if the upload failed
        reason: String,
    },
}

pub(crate) struct UploadMedia {
    api_base: String,
}

impl NexusTool for UploadMedia {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {
            api_base: format!("{}{}", TWITTER_API_BASE, MEDIA_UPLOAD_ENDPOINT),
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
        // Decode base64 media data
        let media_data = match base64::decode(&request.media_data) {
            Ok(data) => data,
            Err(e) => {
                return Output::Err {
                    reason: format!("Failed to decode media data: {}", e),
                }
            }
        };

        // Upload media using chunking process
        match upload_media(
            &self.api_base,
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
        )
        .await
        {
            Ok(response) => match response.data {
                Some(data) => Output::Ok {
                    media_id: data.id,
                    media_key: data.media_key,
                    processing_info: data.processing_info,
                },
                None => Output::Err {
                    reason: "No data in response".to_string(),
                },
            },
            Err(e) => Output::Err {
                reason: format!("Failed to upload media: {}", e),
            },
        }
    }
}

/// Upload media to Twitter in chunks
async fn upload_media(
    api_url: &str,
    auth: &TwitterAuth,
    media_data: &[u8],
    media_type: &str,
    media_category: &MediaCategory,
    chunk_size: usize,
    additional_owners: Option<&Vec<String>>,
) -> TwitterResult<MediaUploadResponse> {
    let client = Client::new();

    // Calculate optimal chunk size if not specified
    let optimal_chunk_size = if chunk_size > 0 {
        chunk_size
    } else {
        calculate_optimal_chunk_size(media_data.len(), media_type, media_category)
    };

    // 1. INIT phase - Initialize upload
    let init_response = init_upload(
        &client,
        api_url,
        auth,
        media_data.len() as u32,
        media_type,
        media_category,
        additional_owners,
    )
    .await?;

    let media_id = init_response
        .data
        .as_ref()
        .ok_or_else(|| TwitterError::Other("Media upload initialization failed".to_string()))?
        .id
        .clone();

    // 2. APPEND phase - Upload chunks
    let chunks = media_data.chunks(optimal_chunk_size).enumerate();

    for (i, chunk) in chunks {
        append_chunk(&client, api_url, auth, &media_id, chunk, i as i32).await?;
    }

    // 3. FINALIZE phase - Complete the upload
    finalize_upload(&client, api_url, auth, &media_id).await
}

/// Calculate optimal chunk size based on media size and type
fn calculate_optimal_chunk_size(
    media_size: usize,
    media_type: &str,
    media_category: &MediaCategory,
) -> usize {
    // Twitter API's maximum chunk size is 5MB as per documentation
    // https://developer.x.com/en/docs/x-api/v1/media/upload-media/api-reference/post-media-upload-append
    const MAX_CHUNK_SIZE: usize = 5 * 1024 * 1024; // 5MB

    // Minimum chunk size to avoid too many requests
    const MIN_CHUNK_SIZE: usize = 128 * 1024; // 128KB

    // For very small files, use a single chunk if possible
    if media_size <= MAX_CHUNK_SIZE {
        // Use the full file size if it's under the maximum allowed chunk size
        // This reduces overhead with multiple requests
        return media_size;
    }

    // Twitter doc mentions optimizing for cellular clients, so we set reasonable limits
    // For images (typically smaller and faster to upload)
    if media_type.starts_with("image/") && !media_type.contains("gif") {
        // For larger images, still keep chunks reasonably sized
        return std::cmp::min(media_size / 4, 2 * 1024 * 1024); // Max 2MB chunks for images
    }

    // For GIFs, which can be larger but still image-based
    if media_type == "image/gif" || matches!(media_category, MediaCategory::TweetGif) {
        // Balance between speed and reliability
        return 3 * 1024 * 1024; // 3MB for GIFs
    }

    // For videos, which are usually much larger files
    if media_type.starts_with("video/")
        || matches!(
            media_category,
            MediaCategory::TweetVideo | MediaCategory::DmVideo | MediaCategory::AmplifyVideo
        )
    {
        // For large videos, use max chunk size for better upload efficiency
        // The docs mention that larger chunks are better for stable connections
        if media_size > 20 * 1024 * 1024 {
            return MAX_CHUNK_SIZE; // 5MB for large videos
        } else {
            return 4 * 1024 * 1024; // 4MB for smaller videos
        }
    }

    // Calculate optimal number of chunks based on file size
    // Aim for a reasonable number of chunks to balance reliability and performance
    let ideal_chunk_count = if media_size < 10 * 1024 * 1024 {
        // For files < 10MB, aim for ~8 chunks
        8
    } else if media_size < 50 * 1024 * 1024 {
        // For files between 10MB and 50MB, aim for ~15 chunks
        15
    } else {
        // For larger files, aim for ~25 chunks
        25
    };

    // Calculate chunk size based on ideal chunk count
    let calculated_size = media_size / ideal_chunk_count;

    // Ensure the calculated size is within bounds and round to nearest 128KB
    let chunk_size = std::cmp::min(
        MAX_CHUNK_SIZE,
        std::cmp::max(MIN_CHUNK_SIZE, calculated_size),
    );

    // Round to nearest 128KB for efficiency
    (chunk_size / (128 * 1024)) * (128 * 1024)
}

/// Initialize a media upload (INIT command)
async fn init_upload(
    client: &Client,
    api_url: &str,
    auth: &TwitterAuth,
    total_bytes: u32,
    media_type: &str,
    media_category: &MediaCategory,
    additional_owners: Option<&Vec<String>>,
) -> TwitterResult<MediaUploadResponse> {
    let mut form = Form::new();

    // Required parameters
    form = form
        .text("command", "INIT")
        .text("total_bytes", total_bytes.to_string())
        .text("media_type", media_type.to_string());

    // Add media category
    let media_category_str = match media_category {
        MediaCategory::AmplifyVideo => "amplify_video",
        MediaCategory::TweetGif => "tweet_gif",
        MediaCategory::TweetImage => "tweet_image",
        MediaCategory::TweetVideo => "tweet_video",
        MediaCategory::DmVideo => "dm_video",
        MediaCategory::Subtitles => "subtitles",
    };
    form = form.text("media_category", media_category_str);

    // Add optional additional owners if present
    if let Some(owners) = additional_owners {
        if !owners.is_empty() {
            form = form.text("additional_owners", owners.join(","));
        }
    }

    // Send the request
    let auth_header = auth.generate_auth_header(api_url);

    let response = client
        .post(api_url)
        .header("Authorization", auth_header)
        .multipart(form)
        .send()
        .await?;

    parse_twitter_response::<MediaUploadResponse>(response).await
}

/// Append a chunk to the upload (APPEND command)
async fn append_chunk(
    client: &Client,
    api_url: &str,
    auth: &TwitterAuth,
    media_id: &str,
    chunk: &[u8],
    segment_index: i32,
) -> TwitterResult<()> {
    // Create part for the media chunk
    let part = Part::bytes(chunk.to_vec()).file_name("media.bin"); // Generic filename, doesn't matter

    // Create form with APPEND command
    let form = Form::new()
        .text("command", "APPEND")
        .text("media_id", media_id.to_string())
        .text("segment_index", segment_index.to_string())
        .part("media", part);

    // Send the request
    let auth_header = auth.generate_auth_header(api_url);

    let response = client
        .post(api_url)
        .header("Authorization", auth_header)
        .multipart(form)
        .send()
        .await?;

    // APPEND should return 204 No Content
    if response.status() != reqwest::StatusCode::NO_CONTENT {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TwitterError::Other(format!(
            "Failed to append media chunk: {}",
            error_text
        )));
    }

    Ok(())
}

/// Finalize the media upload (FINALIZE command)
async fn finalize_upload(
    client: &Client,
    api_url: &str,
    auth: &TwitterAuth,
    media_id: &str,
) -> TwitterResult<MediaUploadResponse> {
    // Create form with FINALIZE command
    let form = Form::new()
        .text("command", "FINALIZE")
        .text("media_id", media_id.to_string());

    // Send the request
    let auth_header = auth.generate_auth_header(api_url);

    let response = client
        .post(api_url)
        .header("Authorization", auth_header)
        .multipart(form)
        .send()
        .await?;

    parse_twitter_response::<MediaUploadResponse>(response).await
}

/// Check media upload status
async fn _check_media_status(
    client: &Client,
    auth: &TwitterAuth,
    media_id: &str,
) -> TwitterResult<MediaUploadResponse> {
    let url = format!("{}/media/upload/status", TWITTER_API_BASE);

    let auth_header = auth.generate_auth_header(&url);

    let response = client
        .get(&url)
        .header("Authorization", auth_header)
        .query(&[("media_id", media_id)])
        .send()
        .await?;

    parse_twitter_response::<MediaUploadResponse>(response).await
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, serde_json::json};

    impl UploadMedia {
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, UploadMedia) {
        let server = Server::new_async().await;
        let tool = UploadMedia::with_api_base(&server.url());
        (server, tool)
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
            media_type: "image/jpeg".to_string(),
            media_category: MediaCategory::TweetImage,
            additional_owners: vec![],
            chunk_size: 1024,
        }
    }

    #[tokio::test]
    async fn test_invalid_base64() {
        // Create server and tool
        let (_, tool) = create_server_and_tool().await;

        // Create input with invalid base64
        let mut input = create_test_input();
        input.media_data = "Invalid Base64 Data!!!".to_string();

        // Test the media upload
        let result = tool.invoke(input).await;

        // Verify the response is an error
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Failed to decode media data"),
                    "Error message should indicate base64 decode failure, got: {}",
                    reason
                );
            }
        }
    }

    #[tokio::test]
    async fn test_init_failure() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for INIT failure
        let mock = server
            .mock("POST", "/")
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

        // Test the media upload
        let result = tool.invoke(create_test_input()).await;

        // Verify the response is an error
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Failed to upload media"),
                    "Error message should indicate upload failure, got: {}",
                    reason
                );
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    // Using a simpler testing approach - test each function individually rather than the full flow

    #[tokio::test]
    async fn test_init_upload() {
        // Create a client and server
        let (mut server, _) = create_server_and_tool().await;
        let client = Client::new();
        let auth = TwitterAuth::new(
            "test_consumer_key",
            "test_consumer_secret",
            "test_access_token",
            "test_access_token_secret",
        );

        // Set up mock for INIT
        let mock = server
            .mock("POST", "/")
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
            &server.url(),
            &auth,
            1024,
            "image/jpeg",
            &MediaCategory::TweetImage,
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
            assert!(response.data.is_some(), "Expected data in response");
            let data = response.data.unwrap();
            assert_eq!(data.id, "12345678901234567890");
            assert_eq!(data.media_key, "12_12345678901234567890");
        }

        // Verify the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_append_chunk() {
        // Create a client and server
        let (mut server, _) = create_server_and_tool().await;
        let client = Client::new();
        let auth = TwitterAuth::new(
            "test_consumer_key",
            "test_consumer_secret",
            "test_access_token",
            "test_access_token_secret",
        );

        // Set up mock for APPEND
        let mock = server
            .mock("POST", "/")
            .with_status(204) // APPEND returns 204 No Content
            .create_async()
            .await;

        // Call append_chunk directly
        let result = append_chunk(
            &client,
            &server.url(),
            &auth,
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
        let (mut server, _) = create_server_and_tool().await;
        let client = Client::new();
        let auth = TwitterAuth::new(
            "test_consumer_key",
            "test_consumer_secret",
            "test_access_token",
            "test_access_token_secret",
        );

        // Set up mock for FINALIZE
        let mock = server
            .mock("POST", "/")
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
        let result = finalize_upload(&client, &server.url(), &auth, "12345678901234567890").await;

        // Verify success
        assert!(
            result.is_ok(),
            "Expected finalize_upload to succeed: {:?}",
            result
        );
        if let Ok(response) = result {
            assert!(response.data.is_some(), "Expected data in response");
            let data = response.data.unwrap();
            assert_eq!(data.id, "12345678901234567890");
            assert_eq!(data.media_key, "12_12345678901234567890");
            assert!(data.processing_info.is_some());
            let processing_info = data.processing_info.unwrap();
            assert_eq!(processing_info.state, "succeeded");
        }

        // Verify the mock was called
        mock.assert_async().await;
    }
}
