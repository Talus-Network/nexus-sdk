use {
    crate::{
        error::{TwitterApiError, TwitterError, TwitterErrorKind, TwitterErrorResponse},
        impl_twitter_response_parser,
        tweet::models::{ApiError, Meta, ReferencedTweet},
        twitter_client::TwitterApiParsedResponse,
    },
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DmConversationResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<DmConversationData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<TwitterApiError>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DmConversationData {
    /// Unique identifier of a DM conversation.
    pub dm_conversation_id: String,
    /// Unique identifier of a DM Event.
    pub dm_event_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DmEventsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<DmEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub includes: Option<Includes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DmEvent {
    pub id: String,
    pub event_type: EventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Attachments>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dm_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<DmEntities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced_tweets: Option<Vec<ReferencedTweet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    MessageCreate,
    ParticipantsJoin,
    ParticipantsLeave,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Attachments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_keys: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DmEntities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cashtags: Option<Vec<Cashtag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashtags: Option<Vec<Hashtag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions: Option<Vec<Mention>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<Vec<UrlEntity>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Cashtag {
    pub end: i32,
    pub start: i32,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Hashtag {
    pub end: i32,
    pub start: i32,
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Mention {
    pub end: i32,
    pub start: i32,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UrlEntity {
    pub end: i32,
    pub start: i32,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<UrlImage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unwound_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UrlImage {
    pub url: String,
    pub height: i32,
    pub width: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Includes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<Vec<Media>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub places: Option<Vec<Place>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polls: Option<Vec<Poll>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Topic>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tweets: Option<Vec<Tweet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<User>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Media {
    #[serde(rename = "type")]
    pub media_type: String,
    pub media_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Place {
    pub id: String,
    pub full_name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Poll {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Topic {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Tweet {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DmEventField {
    Attachments,
    CreatedAt,
    DmConversationId,
    Entities,
    EventType,
    Id,
    ParticipantIds,
    ReferencedTweets,
    SenderId,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionField {
    #[serde(rename = "attachments.media_keys")]
    AttachmentsMediaKeys,
    ParticipantIds,
    #[serde(rename = "referenced_tweets.id")]
    ReferencedTweetsId,
    SenderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaField {
    AltText,
    DurationMs,
    Height,
    MediaKey,
    NonPublicMetrics,
    OrganicMetrics,
    PreviewImageUrl,
    PromotedMetrics,
    PublicMetrics,
    #[serde(rename = "type")]
    Type,
    Url,
    Variants,
    Width,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserField {
    Affiliation,
    ConfirmedEmail,
    ConnectionStatus,
    CreatedAt,
    Description,
    Entities,
    Id,
    IsIdentityVerified,
    Location,
    MostRecentTweetId,
    Name,
    Parody,
    PinnedTweetId,
    ProfileBannerUrl,
    ProfileImageUrl,
    Protected,
    PublicMetrics,
    ReceivesYourDm,
    Subscription,
    SubscriptionType,
    Url,
    Username,
    Verified,
    VerifiedFollowersCount,
    VerifiedType,
    Withheld,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TweetField {
    Article,
    Attachments,
    AuthorId,
    CardUri,
    CommunityId,
    ContextAnnotations,
    ConversationId,
    CreatedAt,
    DisplayTextRange,
    EditControls,
    EditHistoryTweetIds,
    Entities,
    Geo,
    Id,
    InReplyToUserId,
    Lang,
    MediaMetadata,
    NonPublicMetrics,
    NoteTweet,
    OrganicMetrics,
    PossiblySensitive,
    PromotedMetrics,
    PublicMetrics,
    ReferencedTweets,
    ReplySettings,
    Scopes,
    Source,
    Text,
    Withheld,
}

#[derive(Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationType {
    Group,
}

#[derive(Deserialize, JsonSchema, Serialize)]
pub struct Attachment {
    /// The media id of the attachment.
    pub media_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Message {
    /// The text of the message.
    /// Required if attachments is not provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The attachments for the message.
    /// Required if text is not provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

impl Message {
    /// Validates that either text or attachments is provided
    pub fn validate(&self) -> Result<(), String> {
        if self.text.is_none() && self.attachments.is_none() {
            return Err("Either text or attachments must be provided".to_string());
        }
        if let Some(text) = &self.text {
            if text.is_empty() {
                return Err("Text must not be empty".to_string());
            }
        }
        if let Some(attachments) = &self.attachments {
            if attachments.is_empty() {
                return Err("Attachments must not be empty".to_string());
            }
        }
        Ok(())
    }
}

impl_twitter_response_parser!(DmConversationResponse, DmConversationData);
