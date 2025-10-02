# `xyz.taluslabs.market.coinbase.get-spot-price@1`

Standard Nexus Tool that retrieves the current spot price for a currency pair from Coinbase. Coinbase API [reference](https://docs.cdp.coinbase.com/coinbase-app/track-apis/prices)

## Input

**`currency_pair`: [`String` | `Vec<String>`]**

The currency pair to get spot price for. Can be provided in multiple formats:

- **Full pair string**: `"BTC-USD"`, `"ETH-EUR"`, `"SUI-USD"`
- **Array format**: `["BTC", "USD"]`, `["ETH", "EUR"]`, `["SUI", "USD"]`
- **Base currency only**: `"BTC"`, `"ETH"`, `"SUI"` (when `quote_currency` is provided)

**`quote_currency`: [`String`] (optional)**

The quote currency to pair with the base currency. When provided, `currency_pair` should contain only the base currency (e.g., `"BTC"` with `quote_currency: "USD"`).

**`date`: [`String`] (optional)**

The date for historical spot price in YYYY-MM-DD format (e.g., `"2025-08-21"`). If not provided, returns the current spot price.

## Output Variants & Ports

**`ok`**

The spot price was retrieved successfully.

- **`ok.amount`: [`String`]** - The price amount as a string
- **`ok.base`: [`String`]** - The base currency (e.g., "BTC", "ETH")
- **`ok.currency`: [`String`]** - The quote currency (e.g., "USD", "USDT")

**`err`**

The spot price request failed due to an error.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
