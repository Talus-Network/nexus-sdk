//! # `xyz.taluslabs.market.coinbase.get-spot-price@1`
//!
//! Standard Nexus Tool that retrieves the current spot price for a currency pair from Coinbase.

use {
    crate::{
        coinbase_client::CoinbaseClient,
        market::{
            models::{CoinbaseApiResponse, SpotPriceData},
            COINBASE_API_BASE,
        },
    },
    chrono::{NaiveDate, Utc},
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Deserializer, Serialize},
    serde_json::Value,
};

/// Custom deserializer for currency_pair that accepts both string and tuple formats
fn deserialize_currency_pair<'de, D>(deserializer: D) -> Result<String, D::Error>
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

/// Validates that the date string is in YYYY-MM-DD format and is a valid date
fn validate_date_format(date: &str) -> Result<(), String> {
    // Parse the date using chrono
    let parsed_date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Date must be in YYYY-MM-DD format".to_string())?;

    // Check that the date is not in the future
    let today = Utc::now().date_naive();
    if parsed_date > today {
        return Err("Date cannot be in the future".to_string());
    }

    Ok(())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Currency pair to get spot price for (e.g., "BTC-USD", "ETH-EUR" or ["BTC", "USD"])
    /// Can also be just the base currency (e.g., "BTC") when quote_currency is provided
    #[serde(deserialize_with = "deserialize_currency_pair")]
    currency_pair: String,
    /// Optional quote currency (e.g., "USD", "EUR"). When provided, currency_pair should be just the base currency
    quote_currency: Option<String>,
    /// Optional date for historical spot price (format: YYYY-MM-DD). If not provided, returns current spot price
    date: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// The price amount as a string
        amount: String,
        /// The base currency (e.g., "BTC", "ETH")
        base: String,
        /// The quote currency (e.g., "USD", "USDT")
        currency: String,
    },
    Err {
        /// Error message if the request failed
        reason: String,
    },
}

pub(crate) struct GetSpotPrice {
    client: CoinbaseClient,
}

impl NexusTool for GetSpotPrice {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        let client = CoinbaseClient::new(Some(COINBASE_API_BASE));
        Self { client }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.market.coinbase.get-spot-price@1")
    }

    fn path() -> &'static str {
        "/get-spot-price"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Validate and construct the final currency pair
        let final_currency_pair = match (&request.currency_pair, &request.quote_currency) {
            (base, Some(quote)) => {
                // If quote_currency is provided, currency_pair should be just the base currency
                if base.contains('-') {
                    return Output::Err {
                        reason: "When quote_currency is provided, currency_pair should be just the base currency (e.g., 'BTC'), not a full pair (e.g., 'BTC-USD')".to_string(),
                    };
                }
                if base.is_empty() || quote.is_empty() {
                    return Output::Err {
                        reason: "Both base currency and quote currency must be non-empty"
                            .to_string(),
                    };
                }
                format!("{}-{}", base, quote)
            }
            (pair, None) => {
                // If no quote_currency provided, currency_pair should be a complete pair
                if pair.is_empty() {
                    return Output::Err {
                        reason: "Currency pair cannot be empty".to_string(),
                    };
                }
                pair.clone()
            }
        };

        // Validate date format if provided
        if let Some(ref date) = request.date {
            if let Err(validation_error) = validate_date_format(date) {
                return Output::Err {
                    reason: format!("Invalid date format: {}", validation_error),
                };
            }
        }

        // Create the endpoint path
        let endpoint = if let Some(ref date) = request.date {
            format!("v2/prices/{}/spot?date={}", final_currency_pair, date)
        } else {
            format!("v2/prices/{}/spot", final_currency_pair)
        };

        // Make the API request using the client
        match self
            .client
            .get::<CoinbaseApiResponse<SpotPriceData>>(&endpoint)
            .await
        {
            Ok(api_response) => {
                // Check for errors in the response
                if let Some(errors) = api_response.errors {
                    if let Some(first_error) = errors.first() {
                        return Output::Err {
                            reason: first_error
                                .error_message
                                .clone()
                                .unwrap_or_else(|| "API error".to_string()),
                        };
                    }
                }

                // Extract the data
                if let Some(data) = api_response.data {
                    Output::Ok {
                        amount: data.amount,
                        base: data.base,
                        currency: data.currency,
                    }
                } else {
                    Output::Err {
                        reason: "No data in API response".to_string(),
                    }
                }
            }
            Err(error_response) => Output::Err {
                reason: error_response.reason,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ::{mockito::Server, serde_json::json},
    };

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetSpotPrice) {
        let server = Server::new_async().await;
        let client = CoinbaseClient::new(Some(&server.url()));
        let tool = GetSpotPrice { client };
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            currency_pair: "BTC-USD".to_string(),
            quote_currency: None,
            date: None,
        }
    }

    fn create_test_input_from_tuple() -> Input {
        // This simulates deserializing from JSON: ["BTC", "USD"]
        let json = serde_json::json!({
            "currency_pair": ["BTC", "USD"]
        });
        serde_json::from_value(json).expect("Failed to deserialize test input")
    }

    fn create_test_input_with_quote_currency() -> Input {
        Input {
            currency_pair: "BTC".to_string(),
            quote_currency: Some("USD".to_string()),
            date: None,
        }
    }

    fn create_test_input_with_date() -> Input {
        Input {
            currency_pair: "BTC-USD".to_string(),
            quote_currency: None,
            date: Some("2023-12-01".to_string()),
        }
    }

    #[tokio::test]
    async fn test_successful_spot_price() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/v2/prices/BTC-USD/spot")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "amount": "45000.00",
                        "base": "BTC",
                        "currency": "USD"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the spot price request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok {
                amount,
                base,
                currency,
            } => {
                assert_eq!(amount, "45000.00");
                assert_eq!(base, "BTC");
                assert_eq!(currency, "USD");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_successful_spot_price_with_tuple() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/v2/prices/BTC-USD/spot")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "amount": "45000.00",
                        "base": "BTC",
                        "currency": "USD"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the spot price request with tuple format
        let result = tool.invoke(create_test_input_from_tuple()).await;

        // Verify the response
        match result {
            Output::Ok {
                amount,
                base,
                currency,
            } => {
                assert_eq!(amount, "45000.00");
                assert_eq!(base, "BTC");
                assert_eq!(currency, "USD");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_currency_pair() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            currency_pair: "".to_string(),
            quote_currency: None,
            date: None,
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert_eq!(reason, "Currency pair cannot be empty");
            }
        }
    }

    #[tokio::test]
    async fn test_api_error() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for API error response
        let mock = server
            .mock("GET", "/v2/prices/INVALID-PAIR/spot")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "errors": [{
                        "message": "Invalid currency pair",
                        "type": "invalid_request"
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;

        let input = Input {
            currency_pair: "INVALID-PAIR".to_string(),
            quote_currency: None,
            date: None,
        };

        // Test the spot price request
        let result = tool.invoke(input).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(reason.contains("API error"));
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[test]
    fn test_deserialize_currency_pair_string() {
        let json = serde_json::json!({
            "currency_pair": "ETH-EUR"
        });
        let input: Input = serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(input.currency_pair, "ETH-EUR");
        assert_eq!(input.quote_currency, None);
        assert_eq!(input.date, None);
    }

    #[test]
    fn test_deserialize_currency_pair_tuple() {
        let json = serde_json::json!({
            "currency_pair": ["ETH", "EUR"]
        });
        let input: Input = serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(input.currency_pair, "ETH-EUR");
        assert_eq!(input.quote_currency, None);
        assert_eq!(input.date, None);
    }

    #[test]
    fn test_deserialize_with_quote_currency() {
        let json = serde_json::json!({
            "currency_pair": "ETH",
            "quote_currency": "EUR"
        });
        let input: Input = serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(input.currency_pair, "ETH");
        assert_eq!(input.quote_currency, Some("EUR".to_string()));
        assert_eq!(input.date, None);
    }

    #[test]
    fn test_deserialize_currency_pair_invalid_tuple_length() {
        let json = serde_json::json!({
            "currency_pair": ["ETH"]
        });
        let result: Result<Input, _> = serde_json::from_value(json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exactly 2 elements"));
    }

    #[test]
    fn test_deserialize_currency_pair_invalid_tuple_type() {
        let json = serde_json::json!({
            "currency_pair": ["ETH", 123]
        });
        let result: Result<Input, _> = serde_json::from_value(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be a string"));
    }

    #[test]
    fn test_deserialize_currency_pair_invalid_type() {
        let json = serde_json::json!({
            "currency_pair": 123
        });
        let result: Result<Input, _> = serde_json::from_value(json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be either a string"));
    }

    #[tokio::test]
    async fn test_successful_spot_price_with_quote_currency() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/v2/prices/BTC-USD/spot")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "amount": "45000.00",
                        "base": "BTC",
                        "currency": "USD"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the spot price request with separate base and quote currencies
        let result = tool.invoke(create_test_input_with_quote_currency()).await;

        // Verify the response
        match result {
            Output::Ok {
                amount,
                base,
                currency,
            } => {
                assert_eq!(amount, "45000.00");
                assert_eq!(base, "BTC");
                assert_eq!(currency, "USD");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_combination_with_quote_currency() {
        let (_, tool) = create_server_and_tool().await;

        // Test with full currency pair and quote_currency (should fail)
        let input = Input {
            currency_pair: "BTC-USD".to_string(),
            quote_currency: Some("EUR".to_string()),
            date: None,
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(reason.contains("currency_pair should be just the base currency"));
            }
        }
    }

    #[tokio::test]
    async fn test_empty_base_currency_with_quote() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            currency_pair: "".to_string(),
            quote_currency: Some("USD".to_string()),
            date: None,
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert_eq!(
                    reason,
                    "Both base currency and quote currency must be non-empty"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_empty_quote_currency_with_base() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            currency_pair: "BTC".to_string(),
            quote_currency: Some("".to_string()),
            date: None,
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert_eq!(
                    reason,
                    "Both base currency and quote currency must be non-empty"
                );
            }
        }
    }

    #[test]
    fn test_deserialize_with_date() {
        let json = serde_json::json!({
            "currency_pair": "BTC-USD",
            "date": "2023-12-01"
        });
        let input: Input = serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(input.currency_pair, "BTC-USD");
        assert_eq!(input.quote_currency, None);
        assert_eq!(input.date, Some("2023-12-01".to_string()));
    }

    #[test]
    fn test_validate_date_format_valid() {
        assert!(validate_date_format("2023-12-01").is_ok());
        assert!(validate_date_format("2000-01-01").is_ok());
        assert!(validate_date_format("2020-12-31").is_ok());
    }

    #[test]
    fn test_validate_date_format_invalid_format_clearly_wrong() {
        let result = validate_date_format("not-a-date");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("YYYY-MM-DD format"));
    }

    #[test]
    fn test_validate_date_format_invalid_format() {
        let result = validate_date_format("2023/12/01");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("YYYY-MM-DD format"));
    }

    #[test]
    fn test_validate_date_format_future_date() {
        // Use a date that's definitely in the future (tomorrow)
        let tomorrow = (Utc::now().date_naive() + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let result = validate_date_format(&tomorrow);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be in the future"));
    }

    #[tokio::test]
    async fn test_successful_spot_price_with_date() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response for historical price
        let mock = server
            .mock("GET", "/v2/prices/BTC-USD/spot?date=2023-12-01")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "amount": "42000.00",
                        "base": "BTC",
                        "currency": "USD"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the spot price request with historical date
        let result = tool.invoke(create_test_input_with_date()).await;

        // Verify the response
        match result {
            Output::Ok {
                amount,
                base,
                currency,
            } => {
                assert_eq!(amount, "42000.00");
                assert_eq!(base, "BTC");
                assert_eq!(currency, "USD");
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_date_format() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            currency_pair: "BTC-USD".to_string(),
            quote_currency: None,
            date: Some("2023/12/01".to_string()),
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert!(reason.contains("Invalid date format"));
            }
        }
    }

    #[tokio::test]
    async fn test_both_currency_fields_empty() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            currency_pair: "".to_string(),
            quote_currency: Some("".to_string()),
            date: None,
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                assert_eq!(
                    reason,
                    "Both base currency and quote currency must be non-empty"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_currency_pair_with_special_characters() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            currency_pair: "BTC@USD".to_string(),
            quote_currency: None,
            date: None,
        };

        let result = tool.invoke(input).await;

        // Should fail due to invalid currency pair format
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err { reason } => {
                // API should reject this as invalid
                assert!(reason.contains("API error") || reason.contains("Invalid"));
            }
        }
    }

    #[test]
    fn test_validate_date_format_edge_cases() {
        // Test leap year
        assert!(validate_date_format("2024-02-29").is_ok());

        // Test non-leap year February 29th (should fail)
        let result = validate_date_format("2023-02-29");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("YYYY-MM-DD format"));

        // Test December 31st
        assert!(validate_date_format("2023-12-31").is_ok());

        // Test January 1st
        assert!(validate_date_format("2023-01-01").is_ok());
    }

    #[tokio::test]
    async fn test_currency_pair_case_sensitivity() {
        let (mut server, tool) = create_server_and_tool().await;

        // Test with lowercase currency pair
        let mock = server
            .mock("GET", "/v2/prices/btc-usd/spot")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "data": {
                        "amount": "45000.00",
                        "base": "BTC",
                        "currency": "USD"
                    }
                })
                .to_string(),
            )
            .create_async()
            .await;

        let input = Input {
            currency_pair: "btc-usd".to_string(),
            quote_currency: None,
            date: None,
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => {
                // Should succeed if API accepts lowercase
            }
            Output::Err { reason } => {
                // Should fail if API is case-sensitive
                assert!(reason.contains("API error") || reason.contains("Invalid"));
            }
        }

        mock.assert_async().await;
    }
}
