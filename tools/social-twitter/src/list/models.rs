use {
    crate::tweet::models::{
        ApiError,
        Attachments,
        ContextAnnotation,
        EditControls,
        Entities,
        Geo,
        NonPublicMetrics,
        NoteTweet,
        OrganicMetrics,
        PromotedMetrics,
        PublicMetrics,
        ReferencedTweet,
        Scopes,
        Withheld,
    },
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ListData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<ListData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub includes: Option<Includes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ListMemberResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ListMemberData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListMemberData {
    pub is_member: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListData {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follower_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListTweetsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<ListTweet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub includes: Option<Includes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListTweet {
    pub id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Attachments>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_annotations: Option<Vec<ContextAnnotation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
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
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld: Option<Withheld>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Includes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<User>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tweets: Option<Vec<ListTweet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<Vec<Media>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub places: Option<Vec<Place>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polls: Option<Vec<Poll>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: String,
    pub name: String,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_metrics: Option<UserPublicMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserPublicMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followers_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub following_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tweet_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listed_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Media {
    pub media_key: String,
    #[serde(rename = "type")]
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Place {
    pub id: String,
    pub full_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Poll {
    pub id: String,
    pub options: Vec<PollOption>,
    pub end_datetime: String,
    pub duration_minutes: i32,
    pub voting_status: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PollOption {
    pub position: i32,
    pub label: String,
    pub votes: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListField {
    CreatedAt,
    Description,
    FollowerCount,
    Id,
    MemberCount,
    Name,
    OwnerId,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Expansion {
    OwnerId,
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
