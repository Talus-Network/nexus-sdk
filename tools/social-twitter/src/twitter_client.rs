//! Twitter API client implementation
//!
//! This module provides a client for interacting with the Twitter API v2.

use {
    crate::{
        auth::TwitterAuth,
        error::{parse_twitter_response, TwitterError, TwitterErrorResponse},
    },
    reqwest::Client,
    serde::{de::DeserializeOwned, Serialize},
    serde_json::Value,
    std::sync::Arc,
};

/// Twitter API client for making authenticated requests
pub struct TwitterClient {
    /// HTTP client for making requests
    client: Arc<Client>,
    /// URL for Twitter API
    api_base: String,
}

pub(crate) const TWITTER_API_BASE: &str = "https://api.twitter.com/2";

#[derive(Debug, thiserror::Error)]
pub enum TwitterClientError {
    #[error("Unsupported HTTP method: {0}")]
    UnsupportedMethod(String),
}

impl TwitterClient {
    /// Creates a new Twitter client instance
    ///
    /// Optionally takes an endpoint suffix to append to the API base
    pub fn new(
        endpoint_suffix: Option<&str>,
        base_url: Option<&str>,
    ) -> Result<Self, TwitterClientError> {
        let base = base_url.unwrap_or(TWITTER_API_BASE);
        let api_base = match endpoint_suffix {
            Some(suffix) => format!("{}/{}", base, suffix),
            None => base.to_string(),
        };

        Ok(Self {
            client: Arc::new(Client::new()),
            api_base,
        })
    }

    /// Returns the base API URL
    ///
    /// This is the base URL of the Twitter API, which is the URL of the API endpoint
    /// that is used to make requests to the Twitter API.
    #[allow(dead_code)]
    pub fn get_base_api_url(&self) -> &str {
        &self.api_base
    }

    /// Updates the base API URL
    ///
    /// This is used to update the base API URL of the Twitter client.
    #[allow(dead_code)]
    pub fn update_base_api_url(&mut self, new_base_api_url: &str) {
        self.api_base = new_base_api_url.to_string();
    }

    /// Makes a POST request to the Twitter API
    pub async fn post<T, U>(
        &self,
        auth: &TwitterAuth,
        body: U,
    ) -> Result<T::Output, TwitterErrorResponse>
    where
        T: TwitterApiParsedResponse + DeserializeOwned + std::fmt::Debug,
        U: Serialize,
    {
        let raw_response: T = self.make_request("POST", auth, Some(body)).await?;
        raw_response.parse_twitter_response()
    }

    /// Makes a GET request to the Twitter API with a bearer token
    pub async fn get<T>(&self, bearer_token: String) -> Result<T, TwitterErrorResponse>
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        self.make_request_with_bearer_token::<T>("GET", bearer_token)
            .await
    }

    /// Makes a GET request to the Twitter API with auth
    pub async fn get_with_auth<T>(&self, auth: &TwitterAuth) -> Result<T, TwitterErrorResponse>
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        self.make_request::<T, Value>("GET", auth, None).await
    }

    /// Makes a PUT request to the Twitter API
    pub async fn put<T, U>(&self, auth: &TwitterAuth, body: U) -> Result<T, TwitterErrorResponse>
    where
        T: DeserializeOwned + std::fmt::Debug,
        U: Serialize,
    {
        self.make_request("PUT", auth, Some(body)).await
    }

    /// Makes a DELETE request to the Twitter API
    pub async fn delete<T>(&self, auth: &TwitterAuth) -> Result<T::Output, TwitterErrorResponse>
    where
        T: TwitterApiParsedResponse + DeserializeOwned + std::fmt::Debug,
    {
        let raw_response: T = self
            .make_request::<T, serde_json::Value>("DELETE", auth, None)
            .await?;
        raw_response.parse_twitter_response()
    }

    /// Makes an authenticated request to the Twitter API with auth
    ///
    /// This is a helper function that makes a request to the Twitter API with auth
    async fn make_request<T, Value>(
        &self,
        method: &str,
        auth: &TwitterAuth,
        body: Option<Value>,
    ) -> Result<T, TwitterErrorResponse>
    where
        T: DeserializeOwned + std::fmt::Debug,
        Value: Serialize,
    {
        let auth_header = match method {
            "GET" => auth.generate_auth_header(&self.api_base),
            "POST" => auth.generate_auth_header(&self.api_base),
            "DELETE" => auth.generate_auth_header_for_delete(&self.api_base),
            "PUT" => auth.generate_auth_header_for_put(&self.api_base),
            _ => {
                return Err(TwitterError::Other(
                    TwitterClientError::UnsupportedMethod(method.to_string()).to_string(),
                )
                .to_error_response())
            }
        };

        let mut request = self.client.request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
            &self.api_base,
        );

        request = request
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(&body);
        }

        // Network/connection errors
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                return Err(TwitterError::Network(e).to_error_response());
            }
        };

        // API errors (status codes, parsing, etc.)
        match parse_twitter_response::<T>(response).await {
            Ok(data) => Ok(data),
            Err(e) => Err(e.to_error_response()),
        }
    }

    /// Makes an authenticated request to the Twitter API with a bearer token
    async fn make_request_with_bearer_token<T>(
        &self,
        method: &str,
        bearer_token: String,
    ) -> Result<T, TwitterErrorResponse>
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        let mut request = self.client.request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
            &self.api_base,
        );

        request = request
            .header("Authorization", format!("Bearer {}", bearer_token))
            .header("Content-Type", "application/json");

        // Network/connection errors
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                return Err(TwitterError::Network(e).to_error_response());
            }
        };

        // API errors (status codes, parsing, etc.)
        match parse_twitter_response::<T>(response).await {
            Ok(data) => Ok(data),
            Err(e) => Err(e.to_error_response()),
        }
    }
}

/// Trait to implement `TwitterApiParsedResponse` for a given response type and data type
///
/// This trait implements the `TwitterApiParsedResponse` trait for a given response type and data type.
pub trait TwitterApiParsedResponse {
    type Output;

    fn parse_twitter_response(self) -> Result<Self::Output, TwitterErrorResponse>;
}

/// Macro to implement `TwitterApiParsedResponse` for a given response type and data type
///
/// This macro implements the `TwitterApiParsedResponse` trait for a given response type and data type.
#[macro_export]
macro_rules! impl_twitter_response_parser {
    ($response_ty:ty, $data_ty:ty) => {
        impl TwitterApiParsedResponse for $response_ty {
            type Output = $data_ty;

            fn parse_twitter_response(self) -> Result<Self::Output, TwitterErrorResponse> {
                // If we have errors, return them immediately
                if let Some(errors) = self.errors {
                    if let Some(first_error) = errors.first() {
                        return Err(TwitterErrorResponse {
                            reason: first_error.detail.clone().unwrap_or_default(),
                            kind: TwitterErrorKind::Api,
                            status_code: None,
                        });
                    }
                }

                // If we have data, check if we need to include meta and includes
                if let Some(data) = self.data {
                    return Ok(data);
                }

                // If we have neither data nor errors, it's an unknown error
                Err(TwitterError::ParseError(
                    serde_json::from_str::<serde_json::Value>(
                        "Twitter API response validation failed - no data or errors found in response",
                    )
                    .unwrap_err(),
                )
                .to_error_response())
            }
        }
    };
}
