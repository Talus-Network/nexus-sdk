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

    /// Chunk size in bytes for uploading media (default: 1MB)
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
}

fn default_chunk_size() -> usize {
    1024 * 1024 // 1MB default chunk size
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
    let chunks = media_data.chunks(chunk_size).enumerate();

    for (i, chunk) in chunks {
        append_chunk(&client, api_url, auth, &media_id, chunk, i as i32).await?;
    }

    // 3. FINALIZE phase - Complete the upload
    finalize_upload(&client, api_url, auth, &media_id).await
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
