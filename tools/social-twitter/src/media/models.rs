use {
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

/// Available options for media category
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub enum MediaCategory {
    #[serde(rename = "amplify_video")]
    AmplifyVideo,
    #[serde(rename = "tweet_gif")]
    TweetGif,
    #[serde(rename = "tweet_image")]
    TweetImage,
    #[serde(rename = "tweet_video")]
    TweetVideo,
    #[serde(rename = "dm_video")]
    DmVideo,
    #[serde(rename = "subtitles")]
    Subtitles,
}

/// Available options for media upload command
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub enum MediaCommand {
    INIT,
    APPEND,
    FINALIZE,
}

/// Media upload response
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MediaUploadResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<MediaUploadData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<crate::tweet::models::ApiError>>,
}

/// Media upload data
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MediaUploadData {
    pub id: String,
    pub media_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after_secs: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_info: Option<ProcessingInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i32>,
}

/// Processing information for media uploads
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ProcessingInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_after_secs: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<i32>,
    pub state: String, // succeeded, in_progress, pending, failed
}
