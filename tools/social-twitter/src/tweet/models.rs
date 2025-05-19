use {
    crate::{
        error::{TwitterApiError, TwitterError, TwitterErrorKind, TwitterErrorResponse},
        impl_twitter_response_parser,
        twitter_client::TwitterApiParsedResponse,
    },
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TweetsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Tweet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<TwitterApiError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub includes: Option<Includes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetTweetResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Tweet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub includes: Option<Includes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Tweet {
    pub id: String,   // mandatory
    pub text: String, // mandatory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Attachments>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_annotations: Option<Vec<ContextAnnotation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_controls: Option<EditControls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_history_tweet_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Entities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo: Option<Geo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_public_metrics: Option<NonPublicMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_tweet: Option<NoteTweet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organic_metrics: Option<OrganicMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possibly_sensitive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promoted_metrics: Option<PromotedMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_metrics: Option<PublicMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced_tweets: Option<Vec<ReferencedTweet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_settings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Scopes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld: Option<Withheld>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Attachments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_source_tweet_id: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ContextAnnotation {
    pub domain: ContextAnnotationDomain,
    pub entity: ContextAnnotationEntity,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ContextAnnotationDomain {
    pub id: String,
    pub description: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ContextAnnotationEntity {
    pub id: String,
    pub description: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditControls {
    pub editable_until: String,
    pub edits_remaining: i32,
    pub is_edit_eligible: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Entities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<Annotation>>,
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
pub struct Annotation {
    pub end: i32,
    pub start: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
    #[serde(rename = "type")]
    pub annotation_type: String,
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
    pub id: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unwound_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UrlImage {
    pub height: i32,
    pub url: String,
    pub width: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Geo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Coordinates {
    pub coordinates: [f64; 2], // [longitude, latitude]
    #[serde(rename = "type")]
    pub coord_type: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NonPublicMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impression_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NoteTweet {
    pub text: String,
    pub entities: Option<Entities>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OrganicMetrics {
    pub impression_count: i32,
    pub like_count: i32,
    pub reply_count: i32,
    pub retweet_count: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PromotedMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impression_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub like_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retweet_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PublicMetrics {
    pub bookmark_count: i32,
    pub impression_count: i32,
    pub like_count: i32,
    pub reply_count: i32,
    pub retweet_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum ReferencedTweetType {
    #[serde(rename = "retweeted")]
    Retweeted,
    #[serde(rename = "quoted")]
    Quoted,
    #[serde(rename = "replied_to")]
    RepliedTo,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReferencedTweet {
    pub id: String,
    #[serde(rename = "type")]
    pub ref_type: ReferencedTweetType,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Scopes {
    pub followers: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Withheld {
    pub copyright: bool,
    pub country_codes: Vec<String>,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ApiError {
    pub title: String,
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    pub media_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Place {
    pub full_name: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contained_within: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo: Option<GeoPlace>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GeoPlace {
    pub bbox: Vec<f64>,
    pub properties: Value,
    #[serde(rename = "type")]
    pub geo_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Geometry>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Geometry {
    pub coordinates: Vec<f64>,
    #[serde(rename = "type")]
    pub geo_type: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Poll {
    pub duration_minutes: i32,
    pub end_datetime: String,
    pub id: String,
    pub options: Vec<PollOption>,
    pub voting_status: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PollOption {
    pub label: String,
    pub position: i32,
    pub votes: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Topic {
    pub description: String,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub id: String,
    pub name: String,
    pub protected: bool,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_metrics: Option<PublicUserMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PublicUserMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followers_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub following_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tweet_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listed_count: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<i32>,
}

/// Available Tweet fields that can be requested
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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

/// Available expansion fields
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionField {
    #[serde(rename = "article.cover_media")]
    ArticleCoverMedia,
    #[serde(rename = "article.media_entities")]
    ArticleMediaEntities,
    #[serde(rename = "attachments.media_keys")]
    AttachmentsMediaKeys,
    #[serde(rename = "attachments.media_source_tweet")]
    AttachmentsMediaSourceTweet,
    #[serde(rename = "attachments.poll_ids")]
    AttachmentsPollIds,
    AuthorId,
    EditHistoryTweetIds,
    #[serde(rename = "entities.mentions.username")]
    EntitiesMentionsUsername,
    #[serde(rename = "geo.place_id")]
    GeoPlaceId,
    InReplyToUserId,
    #[serde(rename = "entities.note.mentions.username")]
    EntitiesNoteMentionsUsername,
    #[serde(rename = "referenced_tweets.id")]
    ReferencedTweetsId,
    #[serde(rename = "referenced_tweets.id.attachments.media_keys")]
    ReferencedTweetsIdAttachmentsMediaKeys,
    #[serde(rename = "referenced_tweets.id.author_id")]
    ReferencedTweetsIdAuthorId,
}

/// Available Media fields
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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

/// Available Poll fields
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PollField {
    DurationMinutes,
    EndDatetime,
    Id,
    Options,
    VotingStatus,
}

/// Available User fields
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserField {
    Affiliation,
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

/// Available Place fields
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaceField {
    ContainedWithin,
    Country,
    CountryCode,
    FullName,
    Geo,
    Id,
    Name,
    PlaceType,
}

/// Available Exclude fields
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExcludeField {
    Replies,
    Retweets,
}

// This models are only for the post_tweet tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TweetResponse {
    /// Tweet's unique identifier
    pub id: String,
    /// List of tweet IDs in the edit history
    pub edit_history_tweet_ids: Vec<String>,
    /// The actual content of the tweet
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GeoInfo {
    /// Place ID for the location
    pub place_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MediaInfo {
    /// List of media IDs to attach
    pub media_ids: Vec<String>,
    /// List of user IDs to tag in the media
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagged_user_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PollInfo {
    /// Duration of the poll in minutes (5-10080)
    pub duration_minutes: i32,
    /// List of poll options (2-4 options)
    pub options: Vec<String>,
    /// Reply settings for the poll
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_settings: Option<ReplySettings>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReplyInfo {
    /// ID of the tweet to reply to
    pub in_reply_to_tweet_id: String,
    /// List of user IDs to exclude from replies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_reply_user_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum ReplySettings {
    #[serde(rename = "following")]
    Following,
    #[serde(rename = "mentionedUsers")]
    MentionedUsers,
    #[serde(rename = "subscribers")]
    Subscribers,
}

/// Twitter API response for a retweet request
#[derive(Debug, Deserialize)]
pub struct RetweetResponse {
    /// Data returned when the request is successful
    #[serde(default)]
    pub data: Option<RetweetData>,
    /// Errors returned when the request fails
    #[serde(default)]
    pub errors: Option<Vec<TwitterApiError>>,
}

/// Data structure for a successful retweet response
#[derive(Debug, Deserialize)]
pub struct RetweetData {
    /// ID of the retweeted tweet
    pub rest_id: String,
    /// Whether the tweet was successfully retweeted
    pub retweeted: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeleteResponse {
    /// Data returned when the request is successful
    #[serde(default)]
    pub data: Option<DeleteData>,
    /// Errors returned when the request fails
    #[serde(default)]
    pub errors: Option<Vec<TwitterApiError>>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteData {
    pub deleted: bool,
}

/// Twitter API response for an undo retweet request
#[derive(Debug, Deserialize)]
pub struct UndoRetweetResponse {
    /// Data returned when the request is successful
    #[serde(default)]
    pub data: Option<UndoRetweetData>,
    /// Errors returned when the request fails
    #[serde(default)]
    pub errors: Option<Vec<TwitterApiError>>,
}

/// Data structure for a successful undo retweet response
#[derive(Debug, Deserialize)]
pub struct UndoRetweetData {
    /// Whether the tweet was successfully retweeted
    pub retweeted: bool,
}

impl_twitter_response_parser!(RetweetResponse, RetweetData);
impl_twitter_response_parser!(DeleteResponse, DeleteData);
impl_twitter_response_parser!(TweetsResponse, Vec<Tweet>, includes = Includes, meta = Meta);
impl_twitter_response_parser!(UndoRetweetResponse, UndoRetweetData);
