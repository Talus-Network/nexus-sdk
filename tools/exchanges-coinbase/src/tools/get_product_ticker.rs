//! # `xyz.taluslabs.exchanges.coinbase.get-product-ticker@1`
//!
//! Standard Nexus Tool that retrieves the current ticker information for a product from Coinbase Exchange API.

use {
    crate::{
        coinbase_client::CoinbaseClient,
        error::CoinbaseErrorKind,
        tools::{deserialize_trading_pair, models::ProductTickerData, COINBASE_EXCHANGE_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Product ID (currency pair) to get ticker for (e.g., "BTC-USD", "ETH-EUR" or ["BTC", "USD"])
    /// Can also be just the base currency (e.g., "BTC") when quote_currency is provided
    #[serde(deserialize_with = "deserialize_trading_pair")]
    product_id: String,
    /// Optional quote currency (e.g., "USD", "EUR"). When provided, product_id should be just the base currency
    quote_currency: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        /// Best ask price
        ask: String,
        /// Best bid price
        bid: String,
        /// 24h volume
        volume: String,
        /// Trade ID of the last trade
        trade_id: u64,
        /// Last trade price
        price: String,
        /// Last trade size
        size: String,
        /// Time of the last trade
        time: String,
        /// RFQ volume (only included if present)
        #[serde(skip_serializing_if = "Option::is_none")]
        rfq_volume: Option<String>,
        /// Conversions volume (only included if present)
        #[serde(skip_serializing_if = "Option::is_none")]
        conversions_volume: Option<String>,
    },
    Err {
        /// Detailed error message
        reason: String,
        /// Type of error (network, server, auth, etc.)
        kind: CoinbaseErrorKind,
        /// HTTP status code if available
        #[serde(skip_serializing_if = "Option::is_none")]
        status_code: Option<u16>,
    },
}

pub(crate) struct GetProductTicker {
    client: CoinbaseClient,
}

impl NexusTool for GetProductTicker {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        let client = CoinbaseClient::new(Some(COINBASE_EXCHANGE_API_BASE));
        Self { client }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.exchanges.coinbase.get-product-ticker@1")
    }

    fn path() -> &'static str {
        "/get-product-ticker"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, request: Self::Input) -> Self::Output {
        // Validate and construct the final product ID
        let final_product_id = match (&request.product_id, &request.quote_currency) {
            (base, Some(quote)) => {
                // If quote_currency is provided, product_id should be just the base currency
                if base.contains('-') {
                    return Output::Err {
                        reason: "When quote_currency is provided, product_id should be just the base currency (e.g., 'BTC'), not a full pair (e.g., 'BTC-USD')".to_string(),
                        kind: CoinbaseErrorKind::InvalidRequest,
                        status_code: None,
                    };
                }
                if base.is_empty() || quote.is_empty() {
                    return Output::Err {
                        reason: "Both base currency and quote currency must be non-empty"
                            .to_string(),
                        kind: CoinbaseErrorKind::InvalidRequest,
                        status_code: None,
                    };
                }
                format!("{}-{}", base, quote)
            }
            (pair, None) => {
                // If no quote_currency provided, product_id should be a complete pair
                if pair.is_empty() {
                    return Output::Err {
                        reason: "Product ID cannot be empty".to_string(),
                        kind: CoinbaseErrorKind::InvalidRequest,
                        status_code: None,
                    };
                }
                pair.clone()
            }
        };

        // Create the endpoint path
        let endpoint = format!("products/{}/ticker", final_product_id);

        // Make the API request using the client
        match self.client.get::<ProductTickerData>(&endpoint).await {
            Ok(ticker_data) => Output::Ok {
                ask: ticker_data.ask,
                bid: ticker_data.bid,
                volume: ticker_data.volume,
                trade_id: ticker_data.trade_id,
                price: ticker_data.price,
                size: ticker_data.size,
                time: ticker_data.time,
                rfq_volume: ticker_data.rfq_volume,
                conversions_volume: ticker_data.conversions_volume,
            },
            Err(error_response) => Output::Err {
                reason: error_response.reason,
                kind: error_response.kind,
                status_code: error_response.status_code,
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

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetProductTicker) {
        let server = Server::new_async().await;
        let client = CoinbaseClient::new(Some(&server.url()));
        let tool = GetProductTicker { client };
        (server, tool)
    }

    fn create_test_input() -> Input {
        Input {
            product_id: "BTC-USD".to_string(),
            quote_currency: None,
        }
    }

    fn create_test_input_from_tuple() -> Input {
        // This simulates deserializing from JSON: ["BTC", "USD"]
        let json = serde_json::json!({
            "product_id": ["BTC", "USD"]
        });
        serde_json::from_value(json).expect("Failed to deserialize test input")
    }

    fn create_test_input_with_quote_currency() -> Input {
        Input {
            product_id: "BTC".to_string(),
            quote_currency: Some("USD".to_string()),
        }
    }

    #[tokio::test]
    async fn test_successful_ticker_request() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/products/BTC-USD/ticker")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "ask": "6267.71",
                    "bid": "6265.15",
                    "volume": "53602.03940154",
                    "trade_id": 86326522,
                    "price": "6268.48",
                    "size": "0.00698254",
                    "time": "2020-03-20T00:22:57.833Z",
                    "rfq_volume": "123.122"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the ticker request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok {
                ask,
                bid,
                volume,
                trade_id,
                price,
                size,
                time,
                rfq_volume,
                conversions_volume,
            } => {
                assert_eq!(ask, "6267.71");
                assert_eq!(bid, "6265.15");
                assert_eq!(volume, "53602.03940154");
                assert_eq!(trade_id, 86326522);
                assert_eq!(price, "6268.48");
                assert_eq!(size, "0.00698254");
                assert_eq!(time, "2020-03-20T00:22:57.833Z");
                assert_eq!(rfq_volume, Some("123.122".to_string()));
                assert_eq!(conversions_volume, None);
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} (Kind: {:?}, Status Code: {:?})",
                reason, kind, status_code
            ),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_successful_ticker_request_with_tuple() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/products/BTC-USD/ticker")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "ask": "6267.71",
                    "bid": "6265.15",
                    "volume": "53602.03940154",
                    "trade_id": 86326522,
                    "price": "6268.48",
                    "size": "0.00698254",
                    "time": "2020-03-20T00:22:57.833Z",
                    "rfq_volume": "123.122"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the ticker request with tuple format
        let result = tool.invoke(create_test_input_from_tuple()).await;

        // Verify the response
        match result {
            Output::Ok {
                ask,
                bid,
                volume,
                trade_id,
                price,
                size,
                time,
                rfq_volume,
                conversions_volume,
            } => {
                assert_eq!(ask, "6267.71");
                assert_eq!(bid, "6265.15");
                assert_eq!(volume, "53602.03940154");
                assert_eq!(trade_id, 86326522);
                assert_eq!(price, "6268.48");
                assert_eq!(size, "0.00698254");
                assert_eq!(time, "2020-03-20T00:22:57.833Z");
                assert_eq!(rfq_volume, Some("123.122".to_string()));
                assert_eq!(conversions_volume, None);
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} (Kind: {:?}, Status Code: {:?})",
                reason, kind, status_code
            ),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_successful_ticker_request_with_quote_currency() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/products/BTC-USD/ticker")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "ask": "6267.71",
                    "bid": "6265.15",
                    "volume": "53602.03940154",
                    "trade_id": 86326522,
                    "price": "6268.48",
                    "size": "0.00698254",
                    "time": "2020-03-20T00:22:57.833Z",
                    "rfq_volume": "123.122"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the ticker request with separate base and quote currencies
        let result = tool.invoke(create_test_input_with_quote_currency()).await;

        // Verify the response
        match result {
            Output::Ok {
                ask,
                bid,
                volume,
                trade_id,
                price,
                size,
                time,
                rfq_volume,
                conversions_volume,
            } => {
                assert_eq!(ask, "6267.71");
                assert_eq!(bid, "6265.15");
                assert_eq!(volume, "53602.03940154");
                assert_eq!(trade_id, 86326522);
                assert_eq!(price, "6268.48");
                assert_eq!(size, "0.00698254");
                assert_eq!(time, "2020-03-20T00:22:57.833Z");
                assert_eq!(rfq_volume, Some("123.122".to_string()));
                assert_eq!(conversions_volume, None);
            }
            Output::Err {
                reason,
                kind,
                status_code,
            } => panic!(
                "Expected success, got error: {} (Kind: {:?}, Status Code: {:?})",
                reason, kind, status_code
            ),
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_empty_product_id() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            product_id: "".to_string(),
            quote_currency: None,
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(reason, "Product ID cannot be empty");
                assert_eq!(kind, CoinbaseErrorKind::InvalidRequest);
                assert_eq!(status_code, None);
            }
        }
    }

    #[tokio::test]
    async fn test_invalid_combination_with_quote_currency() {
        let (_, tool) = create_server_and_tool().await;

        // Test with full product ID and quote_currency (should fail)
        let input = Input {
            product_id: "BTC-USD".to_string(),
            quote_currency: Some("EUR".to_string()),
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(reason.contains("product_id should be just the base currency"));
                assert_eq!(kind, CoinbaseErrorKind::InvalidRequest);
                assert_eq!(status_code, None);
            }
        }
    }

    #[tokio::test]
    async fn test_empty_base_currency_with_quote() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            product_id: "".to_string(),
            quote_currency: Some("USD".to_string()),
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(
                    reason,
                    "Both base currency and quote currency must be non-empty"
                );
                assert_eq!(kind, CoinbaseErrorKind::InvalidRequest);
                assert_eq!(status_code, None);
            }
        }
    }

    #[tokio::test]
    async fn test_empty_quote_currency_with_base() {
        let (_, tool) = create_server_and_tool().await;

        let input = Input {
            product_id: "BTC".to_string(),
            quote_currency: Some("".to_string()),
        };

        let result = tool.invoke(input).await;

        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert_eq!(
                    reason,
                    "Both base currency and quote currency must be non-empty"
                );
                assert_eq!(kind, CoinbaseErrorKind::InvalidRequest);
                assert_eq!(status_code, None);
            }
        }
    }

    #[tokio::test]
    async fn test_api_error() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock for API error response
        let mock = server
            .mock("GET", "/products/INVALID-PAIR/ticker")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "message": "Invalid product ID"
                })
                .to_string(),
            )
            .create_async()
            .await;

        let input = Input {
            product_id: "INVALID-PAIR".to_string(),
            quote_currency: None,
        };

        // Test the ticker request
        let result = tool.invoke(input).await;

        // Verify the error response
        match result {
            Output::Ok { .. } => panic!("Expected error, got success"),
            Output::Err {
                reason,
                kind,
                status_code,
            } => {
                assert!(reason.contains("API error") || reason.contains("Invalid"));
                // API error should have proper kind and status_code
                assert!(matches!(
                    kind,
                    CoinbaseErrorKind::InvalidRequest | CoinbaseErrorKind::NotFound
                ));
                assert!(status_code.is_some());
            }
        }

        // Verify that the mock was called
        mock.assert_async().await;
    }

    #[test]
    fn test_deserialize_product_id_string() {
        let json = serde_json::json!({
            "product_id": "ETH-EUR"
        });
        let input: Input = serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(input.product_id, "ETH-EUR");
        assert_eq!(input.quote_currency, None);
    }

    #[test]
    fn test_deserialize_product_id_tuple() {
        let json = serde_json::json!({
            "product_id": ["ETH", "EUR"]
        });
        let input: Input = serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(input.product_id, "ETH-EUR");
        assert_eq!(input.quote_currency, None);
    }

    #[test]
    fn test_deserialize_with_quote_currency() {
        let json = serde_json::json!({
            "product_id": "ETH",
            "quote_currency": "EUR"
        });
        let input: Input = serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(input.product_id, "ETH");
        assert_eq!(input.quote_currency, Some("EUR".to_string()));
    }

    #[test]
    fn test_deserialize_product_id_invalid_tuple_length() {
        let json = serde_json::json!({
            "product_id": ["ETH"]
        });
        let result: Result<Input, _> = serde_json::from_value(json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exactly 2 elements"));
    }

    #[test]
    fn test_deserialize_product_id_invalid_tuple_type() {
        let json = serde_json::json!({
            "product_id": ["ETH", 123]
        });
        let result: Result<Input, _> = serde_json::from_value(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be a string"));
    }

    #[test]
    fn test_deserialize_product_id_invalid_type() {
        let json = serde_json::json!({
            "product_id": 123
        });
        let result: Result<Input, _> = serde_json::from_value(json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be either a string"));
    }
}
