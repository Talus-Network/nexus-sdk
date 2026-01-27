//! # `xyz.taluslabs.exchanges.coinbase.get-product-stats@1`
//!
//! Standard Nexus Tool that retrieves 24-hour and 30-day statistics for a product from Coinbase Exchange API.

use {
    crate::{
        coinbase_client::CoinbaseClient,
        error::CoinbaseErrorKind,
        tools::{deserialize_trading_pair, models::ProductStatsData, COINBASE_EXCHANGE_API_BASE},
    },
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// Product ID (currency pair) to get stats for (e.g., "BTC-USD", "ETH-EUR" or ["BTC", "USD"])
    /// Can also be just the base currency (e.g., "BTC") when quote_currency is provided
    #[serde(deserialize_with = "deserialize_trading_pair")]
    product_id: String,
    /// Optional quote currency (e.g., "USD", "EUR"). When provided, product_id should be just the base currency
    quote_currency: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Output {
    Ok {
        /// Opening price (in quote currency)
        open: String,
        /// Highest price (in quote currency)
        high: String,
        /// Lowest price (in quote currency)
        low: String,
        /// 24h volume (in base currency)
        volume: String,
        /// Last price (in quote currency)
        last: String,
        /// 30-day volume (in base currency) (only included if present)
        #[serde(skip_serializing_if = "Option::is_none")]
        volume_30day: Option<String>,
        /// 24h RFQ volume (only included if present)
        #[serde(skip_serializing_if = "Option::is_none")]
        rfq_volume_24hour: Option<String>,
        /// 24h conversions volume (only included if present)
        #[serde(skip_serializing_if = "Option::is_none")]
        conversions_volume_24hour: Option<String>,
        /// 30-day RFQ volume (only included if present)
        #[serde(skip_serializing_if = "Option::is_none")]
        rfq_volume_30day: Option<String>,
        /// 30-day conversions volume (only included if present)
        #[serde(skip_serializing_if = "Option::is_none")]
        conversions_volume_30day: Option<String>,
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

pub(crate) struct GetProductStats {
    client: CoinbaseClient,
}

impl NexusTool for GetProductStats {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        let client = CoinbaseClient::new(Some(COINBASE_EXCHANGE_API_BASE));
        Self { client }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.exchanges.coinbase.get-product-stats@1")
    }

    fn path() -> &'static str {
        "/get-product-stats"
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
        let endpoint = format!("products/{}/stats", final_product_id);

        // Make the API request using the client
        match self.client.get::<ProductStatsData>(&endpoint).await {
            Ok(stats_data) => Output::Ok {
                open: stats_data.open,
                high: stats_data.high,
                low: stats_data.low,
                volume: stats_data.volume,
                last: stats_data.last,
                volume_30day: stats_data.volume_30day,
                rfq_volume_24hour: stats_data.rfq_volume_24hour,
                conversions_volume_24hour: stats_data.conversions_volume_24hour,
                rfq_volume_30day: stats_data.rfq_volume_30day,
                conversions_volume_30day: stats_data.conversions_volume_30day,
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

    async fn create_server_and_tool() -> (mockito::ServerGuard, GetProductStats) {
        let server = Server::new_async().await;
        let client = CoinbaseClient::new(Some(&server.url()));
        let tool = GetProductStats { client };
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
    async fn test_successful_stats_request() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/products/BTC-USD/stats")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "open": "5414.18000000",
                    "high": "6441.37000000",
                    "low": "5261.69000000",
                    "volume": "53687.76764233",
                    "last": "6250.02000000",
                    "volume_30day": "786763.72930864",
                    "rfq_volume_24hour": "78.23",
                    "conversions_volume_24hour": "0.000000",
                    "rfq_volume_30day": "0.000000",
                    "conversions_volume_30day": "0.000000"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the stats request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok {
                open,
                high,
                low,
                volume,
                last,
                volume_30day,
                rfq_volume_24hour,
                conversions_volume_24hour,
                rfq_volume_30day,
                conversions_volume_30day,
            } => {
                assert_eq!(open, "5414.18000000");
                assert_eq!(high, "6441.37000000");
                assert_eq!(low, "5261.69000000");
                assert_eq!(volume, "53687.76764233");
                assert_eq!(last, "6250.02000000");
                assert_eq!(volume_30day, Some("786763.72930864".to_string()));
                assert_eq!(rfq_volume_24hour, Some("78.23".to_string()));
                assert_eq!(conversions_volume_24hour, Some("0.000000".to_string()));
                assert_eq!(rfq_volume_30day, Some("0.000000".to_string()));
                assert_eq!(conversions_volume_30day, Some("0.000000".to_string()));
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
    async fn test_successful_stats_request_with_tuple() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/products/BTC-USD/stats")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "open": "5414.18000000",
                    "high": "6441.37000000",
                    "low": "5261.69000000",
                    "volume": "53687.76764233",
                    "last": "6250.02000000",
                    "volume_30day": "786763.72930864",
                    "rfq_volume_24hour": "78.23",
                    "conversions_volume_24hour": "0.000000",
                    "rfq_volume_30day": "0.000000",
                    "conversions_volume_30day": "0.000000"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the stats request with tuple format
        let result = tool.invoke(create_test_input_from_tuple()).await;

        // Verify the response
        match result {
            Output::Ok {
                open,
                high,
                low,
                volume,
                last,
                volume_30day,
                rfq_volume_24hour,
                conversions_volume_24hour,
                rfq_volume_30day,
                conversions_volume_30day,
            } => {
                assert_eq!(open, "5414.18000000");
                assert_eq!(high, "6441.37000000");
                assert_eq!(low, "5261.69000000");
                assert_eq!(volume, "53687.76764233");
                assert_eq!(last, "6250.02000000");
                assert_eq!(volume_30day, Some("786763.72930864".to_string()));
                assert_eq!(rfq_volume_24hour, Some("78.23".to_string()));
                assert_eq!(conversions_volume_24hour, Some("0.000000".to_string()));
                assert_eq!(rfq_volume_30day, Some("0.000000".to_string()));
                assert_eq!(conversions_volume_30day, Some("0.000000".to_string()));
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
    async fn test_successful_stats_request_with_quote_currency() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response
        let mock = server
            .mock("GET", "/products/BTC-USD/stats")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "open": "5414.18000000",
                    "high": "6441.37000000",
                    "low": "5261.69000000",
                    "volume": "53687.76764233",
                    "last": "6250.02000000",
                    "volume_30day": "786763.72930864",
                    "rfq_volume_24hour": "78.23",
                    "conversions_volume_24hour": "0.000000",
                    "rfq_volume_30day": "0.000000",
                    "conversions_volume_30day": "0.000000"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the stats request with separate base and quote currencies
        let result = tool.invoke(create_test_input_with_quote_currency()).await;

        // Verify the response
        match result {
            Output::Ok {
                open,
                high,
                low,
                volume,
                last,
                volume_30day,
                rfq_volume_24hour,
                conversions_volume_24hour,
                rfq_volume_30day,
                conversions_volume_30day,
            } => {
                assert_eq!(open, "5414.18000000");
                assert_eq!(high, "6441.37000000");
                assert_eq!(low, "5261.69000000");
                assert_eq!(volume, "53687.76764233");
                assert_eq!(last, "6250.02000000");
                assert_eq!(volume_30day, Some("786763.72930864".to_string()));
                assert_eq!(rfq_volume_24hour, Some("78.23".to_string()));
                assert_eq!(conversions_volume_24hour, Some("0.000000".to_string()));
                assert_eq!(rfq_volume_30day, Some("0.000000".to_string()));
                assert_eq!(conversions_volume_30day, Some("0.000000".to_string()));
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
    async fn test_successful_stats_request_without_optional_fields() {
        // Create server and tool
        let (mut server, tool) = create_server_and_tool().await;

        // Set up mock response without optional fields
        let mock = server
            .mock("GET", "/products/BTC-USD/stats")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                json!({
                    "open": "5414.18000000",
                    "high": "6441.37000000",
                    "low": "5261.69000000",
                    "volume": "53687.76764233",
                    "last": "6250.02000000",
                    "volume_30day": "786763.72930864"
                })
                .to_string(),
            )
            .create_async()
            .await;

        // Test the stats request
        let result = tool.invoke(create_test_input()).await;

        // Verify the response
        match result {
            Output::Ok {
                open,
                high,
                low,
                volume,
                last,
                volume_30day,
                rfq_volume_24hour,
                conversions_volume_24hour,
                rfq_volume_30day,
                conversions_volume_30day,
            } => {
                assert_eq!(open, "5414.18000000");
                assert_eq!(high, "6441.37000000");
                assert_eq!(low, "5261.69000000");
                assert_eq!(volume, "53687.76764233");
                assert_eq!(last, "6250.02000000");
                assert_eq!(volume_30day, Some("786763.72930864".to_string()));
                assert_eq!(rfq_volume_24hour, None);
                assert_eq!(conversions_volume_24hour, None);
                assert_eq!(rfq_volume_30day, None);
                assert_eq!(conversions_volume_30day, None);
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
            .mock("GET", "/products/INVALID-PAIR/stats")
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

        // Test the stats request
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
