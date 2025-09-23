//! Coinbase API client implementation
//!
//! This module provides a clean client for interacting with the Coinbase API.

use {
    crate::{error::CoinbaseErrorResponse, market::COINBASE_API_BASE},
    reqwest::Client,
    serde::de::DeserializeOwned,
    std::sync::Arc,
};

/// Coinbase API client for making requests
pub struct CoinbaseClient {
    /// HTTP client for making requests
    client: Arc<Client>,
    /// Base URL for Coinbase API
    base_url: String,
}

impl CoinbaseClient {
    /// Creates a new Coinbase client instance
    pub fn new(base_url: Option<&str>) -> Self {
        let base_url = base_url.unwrap_or(COINBASE_API_BASE).to_string();

        Self {
            client: Arc::new(Client::new()),
            base_url,
        }
    }

    /// Makes a GET request to the specified endpoint
    pub async fn get<T>(&self, endpoint: &str) -> Result<T, CoinbaseErrorResponse>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}/{}", self.base_url, endpoint);

        let response = match self.client.get(&url).send().await {
            Ok(response) => response,
            Err(e) => {
                let kind = crate::error::CoinbaseErrorKind::from_network_error(&e);
                return Err(CoinbaseErrorResponse {
                    reason: format!("Network error: {}", e),
                    kind,
                    status_code: Some(0), // Network errors have status code 0
                    correlation_id: None,
                });
            }
        };

        let status = response.status();
        let text = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return Err(CoinbaseErrorResponse {
                    reason: format!("Failed to read response: {}", e),
                    kind: crate::error::CoinbaseErrorKind::Parse,
                    status_code: None,
                    correlation_id: None,
                });
            }
        };

        if !status.is_success() {
            // Try to parse the error response from Coinbase API
            match serde_json::from_str::<crate::error::CoinbaseApiError>(&text) {
                Ok(api_error) => {
                    return Err(api_error.to_error_response(status.as_u16()));
                }
                Err(_) => {
                    // If we can't parse the error response, fallback to status code mapping
                    let kind = crate::error::CoinbaseErrorKind::from_status_code(status.as_u16());
                    return Err(CoinbaseErrorResponse {
                        reason: format!("API error ({}): {}", status, text),
                        kind,
                        status_code: Some(status.as_u16()),
                        correlation_id: None,
                    });
                }
            }
        }

        match serde_json::from_str::<T>(&text) {
            Ok(data) => Ok(data),
            Err(e) => Err(CoinbaseErrorResponse {
                reason: format!("Failed to parse JSON: {}", e),
                kind: crate::error::CoinbaseErrorKind::Parse,
                status_code: None,
                correlation_id: None,
            }),
        }
    }
}
