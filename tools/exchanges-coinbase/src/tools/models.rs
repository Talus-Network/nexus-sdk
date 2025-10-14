//! Data models for Coinbase exchange endpoints

use {
    crate::error::CoinbaseApiError,
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

/// Product ticker data from Coinbase Exchange API
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProductTickerData {
    /// Best ask price
    pub ask: String,
    /// Best bid price
    pub bid: String,
    /// 24h volume
    pub volume: String,
    /// Trade ID of the last trade
    pub trade_id: u64,
    /// Last trade price
    pub price: String,
    /// Last trade size
    pub size: String,
    /// Time of the last trade
    pub time: String,
    /// RFQ volume (optional field)
    pub rfq_volume: Option<String>,
    /// Conversions volume (optional field)
    pub conversions_volume: Option<String>,
}

/// Product stats data from Coinbase Exchange API
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProductStatsData {
    /// Opening price (in quote currency)
    pub open: String,
    /// Highest price (in quote currency)
    pub high: String,
    /// Lowest price (in quote currency)
    pub low: String,
    /// 24h volume (in base currency)
    pub volume: String,
    /// Last price (in quote currency)
    pub last: String,
    /// 30-day volume (in base currency) (optional field)
    pub volume_30day: Option<String>,
    /// 24h RFQ volume (optional field)
    pub rfq_volume_24hour: Option<String>,
    /// 24h conversions volume (optional field)
    pub conversions_volume_24hour: Option<String>,
    /// 30-day RFQ volume (optional field)
    pub rfq_volume_30day: Option<String>,
    /// 30-day conversions volume (optional field)
    pub conversions_volume_30day: Option<String>,
}

/// Coinbase API response with potential errors
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoinbaseApiResponse<T> {
    /// The response data
    pub data: Option<T>,
    /// List of errors if any
    pub errors: Option<Vec<CoinbaseApiError>>,
}
