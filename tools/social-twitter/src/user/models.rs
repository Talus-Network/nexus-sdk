use {
    crate::{
        error::{TwitterApiError, TwitterError, TwitterErrorKind, TwitterErrorResponse},
        impl_twitter_response_parser,
        list::models::{Includes, Meta},
        tweet::models::ApiError,
        twitter_client::TwitterApiParsedResponse,
    },
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<UserData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub includes: Option<Includes>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UsersResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<UserData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ApiError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserData {
    pub id: String,
    pub name: String,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affiliation: Option<Affiliation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_status: Option<Vec<ConnectionStatus>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<Entities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub most_recent_tweet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_tweet_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_banner_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_metrics: Option<PublicMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receives_your_dm: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<SubscriptionType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_type: Option<VerifiedType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld: Option<Withheld>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Affiliation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PublicMetrics {
    pub followers_count: i32,
    pub following_count: i32,
    pub listed_count: i32,
    pub tweet_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub like_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Withheld {
    pub country_codes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<ScopeIndicates>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScopeIndicates {
    User,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    FollowRequestReceived,
    FollowRequestSent,
    Blocking,
    FollowedBy,
    Following,
    Muting,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionType {
    Basic,
    Premium,
    PremiumPlus,
    None,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerifiedType {
    Blue,
    Government,
    Business,
    None,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Entities {
    pub description: Option<DescriptionEntities>,
    pub url: Option<UrlEntities>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DescriptionEntities {
    pub annotations: Option<Vec<DescriptionAnnotation>>,
    pub cashtags: Option<Vec<Cashtag>>,
    pub hashtags: Option<Vec<Hashtag>>,
    pub mentions: Option<Vec<Mention>>,
    pub urls: Option<Vec<UrlEntity>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UrlEntities {
    pub urls: Option<Vec<UrlEntity>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DescriptionAnnotation {
    pub end: i32,
    pub start: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
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
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnfollowResponse {
    /// Data returned when the request is successful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<UnfollowData>,
    /// Errors returned when the request fails
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<TwitterApiError>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnfollowData {
    /// Whether the user was unfollowed
    pub following: bool,
}

impl_twitter_response_parser!(UnfollowResponse, UnfollowData);
