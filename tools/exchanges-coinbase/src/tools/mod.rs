//! Exchange data endpoints for Coinbase API

use {
    serde::{Deserialize, Deserializer},
    serde_json::Value,
};

pub(crate) const COINBASE_API_BASE: &str = "https://api.coinbase.com";
pub(crate) const COINBASE_EXCHANGE_API_BASE: &str = "https://api.exchange.coinbase.com";

pub(crate) mod get_product_ticker;
pub(crate) mod get_spot_price;
pub(crate) mod models;

/// Custom deserializer for trading pair (currency pair/product ID) that accepts both string and tuple formats
pub(crate) fn deserialize_trading_pair<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;

    match value {
        Value::String(s) => {
            // Direct string format like "BTC-USD" or just base currency like "BTC"
            Ok(s)
        }
        Value::Array(arr) => {
            // Tuple format like ["BTC", "USD"]
            if arr.len() == 2 {
                let base = arr[0].as_str().ok_or_else(|| {
                    serde::de::Error::custom("First element of currency pair array must be a string")
                })?;
                let quote = arr[1].as_str().ok_or_else(|| {
                    serde::de::Error::custom("Second element of currency pair array must be a string")
                })?;
                Ok(format!("{}-{}", base, quote))
            } else {
                Err(serde::de::Error::custom("Currency pair array must contain exactly 2 elements"))
            }
        }
        _ => Err(serde::de::Error::custom(
            "Currency pair must be either a string (e.g., 'BTC-USD') or an array of two strings (e.g., ['BTC', 'USD'])"
        )),
    }
}
