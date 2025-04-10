use {
    oauth1_request::{delete, get, post, put, signature_method::HmacSha1, Token},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

/// Twitter API authentication credentials and methods used across all Twitter API tools
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TwitterAuth {
    /// Consumer API key for Twitter API application
    pub consumer_key: String,
    /// Consumer Secret key for Twitter API application
    pub consumer_secret_key: String,
    /// Access Token for user's Twitter account
    pub access_token: String,
    /// Access Token Secret for user's Twitter account
    pub access_token_secret: String,
}

impl TwitterAuth {
    /// Create a new TwitterAuth instance
    #[cfg(test)]
    pub fn new(
        consumer_key: impl Into<String>,
        consumer_secret_key: impl Into<String>,
        access_token: impl Into<String>,
        access_token_secret: impl Into<String>,
    ) -> Self {
        Self {
            consumer_key: consumer_key.into(),
            consumer_secret_key: consumer_secret_key.into(),
            access_token: access_token.into(),
            access_token_secret: access_token_secret.into(),
        }
    }

    /// Create an OAuth token from the credentials
    pub fn to_token(&self) -> Token {
        Token::from_parts(
            self.consumer_key.clone(),
            self.consumer_secret_key.clone(),
            self.access_token.clone(),
            self.access_token_secret.clone(),
        )
    }

    /// Generate an OAuth authorization header for a POST request
    pub fn generate_auth_header(&self, url: &str) -> String {
        let token = self.to_token();
        post(url, &(), &token, HmacSha1::new())
    }

    /// Generate an OAuth authorization header for a GET request
    pub fn generate_auth_header_for_get(&self, url: &str) -> String {
        let token = self.to_token();
        get(url, &(), &token, HmacSha1::new())
    }

    /// Generate an OAuth authorization header for a PUT request
    pub fn generate_auth_header_for_put(&self, url: &str) -> String {
        let token = self.to_token();
        put(url, &(), &token, HmacSha1::new())
    }

    /// Generate an OAuth authorization header for a DELETE request
    pub fn generate_auth_header_for_delete(&self, url: &str) -> String {
        let token = self.to_token();
        delete(url, &(), &token, HmacSha1::new())
    }
}
