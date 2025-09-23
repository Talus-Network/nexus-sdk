//! Data models for Coinbase market endpoints

use {
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

/// Spot price data from Coinbase API
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpotPriceData {
    /// The price amount as a string
    pub amount: String,
    /// The base currency (e.g., "BTC", "ETH")
    pub base: String,
    /// The quote currency (e.g., "USD", "USDT")
    pub currency: String,
}

/// Coinbase API response with potential errors
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoinbaseApiResponse<T> {
    /// The response data
    pub data: Option<T>,
    /// List of errors if any
    pub errors: Option<Vec<crate::error::CoinbaseApiError>>,
}
