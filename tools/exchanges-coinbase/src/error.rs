use {
    reqwest::StatusCode,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    thiserror::Error,
};

/// Error kind enumeration for Coinbase operations
/// Based on Coinbase CDP API v2 error documentation
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoinbaseErrorKind {
    /// Invalid request (HTTP 400)
    InvalidRequest,
    /// Malformed transaction (HTTP 400)
    MalformedTransaction,
    /// Invalid signature (HTTP 400)
    InvalidSignature,
    /// Policy in use (HTTP 400)
    PolicyInUse,
    /// Invalid SQL query (HTTP 400)
    InvalidSqlQuery,
    /// Document verification failed (HTTP 400)
    DocumentVerificationFailed,
    /// Recipient allowlist violation (HTTP 400)
    RecipientAllowlistViolation,
    /// Recipient allowlist pending (HTTP 400)
    RecipientAllowlistPending,
    /// Travel rules recipient violation (HTTP 400)
    TravelRulesRecipientViolation,
    /// Network not tradable (HTTP 400)
    NetworkNotTradable,
    /// Guest permission denied (HTTP 400)
    GuestPermissionDenied,
    /// Guest region forbidden (HTTP 400)
    GuestRegionForbidden,
    /// Guest transaction limit (HTTP 400)
    GuestTransactionLimit,
    /// Guest transaction count (HTTP 400)
    GuestTransactionCount,
    /// Phone number verification expired (HTTP 400)
    PhoneNumberVerificationExpired,
    /// Unauthorized (HTTP 401)
    Unauthorized,
    /// Payment method required (HTTP 402)
    PaymentMethodRequired,
    /// Forbidden (HTTP 403)
    Forbidden,
    /// Not found (HTTP 404)
    NotFound,
    /// Timed out (HTTP 408)
    TimedOut,
    /// Already exists (HTTP 409)
    AlreadyExists,
    /// Idempotency error (HTTP 422)
    IdempotencyError,
    /// Rate limit exceeded (HTTP 429)
    RateLimitExceeded,
    /// Faucet limit exceeded (HTTP 429)
    FaucetLimitExceeded,
    /// Internal server error (HTTP 500)
    InternalServerError,
    /// Bad gateway (HTTP 502)
    BadGateway,
    /// Service unavailable (HTTP 503)
    ServiceUnavailable,
    /// Network IP blocked
    NetworkIpBlocked,
    /// Network connection failed
    NetworkConnectionFailed,
    /// Network timeout
    NetworkTimeout,
    /// Network DNS failure
    NetworkDnsFailure,
    /// Error parsing response
    Parse,
    /// Unknown error
    Unknown,
}

/// A Coinbase API error returned by the API
/// Based on Coinbase CDP API v2 error response format
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct CoinbaseApiError {
    /// Machine-readable error code (e.g., "invalid_request", "not_found")
    #[serde(rename = "errorType")]
    pub error_type: Option<String>,
    /// Human-readable message providing more detail
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    /// Unique identifier for the request that can help with debugging
    #[serde(rename = "correlationId")]
    pub correlation_id: Option<String>,
    /// Link to detailed documentation about the specific error type
    #[serde(rename = "errorLink")]
    pub error_link: Option<String>,
}

/// Error type for Coinbase operations
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum CoinbaseError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Response parsing error: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Coinbase API error: {0}")]
    ApiError(String),

    #[error("Coinbase API status error: {0}")]
    StatusError(StatusCode),

    #[error("Unknown error: {0}")]
    Other(String),
}

/// Standard error response structure for Coinbase tools
#[derive(Debug, Serialize, Deserialize)]
pub struct CoinbaseErrorResponse {
    /// Detailed error message
    pub reason: String,
    /// Type of error based on Coinbase CDP API v2 error types
    pub kind: CoinbaseErrorKind,
    /// HTTP status code if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    /// Correlation ID for debugging (if available from API)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl CoinbaseErrorKind {
    /// Maps Coinbase API error type string to our error kind
    pub fn from_api_error_type(error_type: &str) -> Self {
        match error_type {
            "invalid_request" => Self::InvalidRequest,
            "malformed_transaction" => Self::MalformedTransaction,
            "invalid_signature" => Self::InvalidSignature,
            "policy_in_use" => Self::PolicyInUse,
            "invalid_sql_query" => Self::InvalidSqlQuery,
            "document_verification_failed" => Self::DocumentVerificationFailed,
            "recipient_allowlist_violation" => Self::RecipientAllowlistViolation,
            "recipient_allowlist_pending" => Self::RecipientAllowlistPending,
            "travel_rules_recipient_violation" => Self::TravelRulesRecipientViolation,
            "network_not_tradable" => Self::NetworkNotTradable,
            "guest_permission_denied" => Self::GuestPermissionDenied,
            "guest_region_forbidden" => Self::GuestRegionForbidden,
            "guest_transaction_limit" => Self::GuestTransactionLimit,
            "guest_transaction_count" => Self::GuestTransactionCount,
            "phone_number_verification_expired" => Self::PhoneNumberVerificationExpired,
            "unauthorized" => Self::Unauthorized,
            "payment_method_required" => Self::PaymentMethodRequired,
            "forbidden" => Self::Forbidden,
            "not_found" => Self::NotFound,
            "timed_out" => Self::TimedOut,
            "already_exists" => Self::AlreadyExists,
            "idempotency_error" => Self::IdempotencyError,
            "rate_limit_exceeded" => Self::RateLimitExceeded,
            "faucet_limit_exceeded" => Self::FaucetLimitExceeded,
            "internal_server_error" => Self::InternalServerError,
            "bad_gateway" => Self::BadGateway,
            "service_unavailable" => Self::ServiceUnavailable,
            _ => Self::Unknown,
        }
    }

    /// Maps HTTP status code to our error kind
    pub fn from_status_code(status_code: u16) -> Self {
        match status_code {
            400 => Self::InvalidRequest,
            401 => Self::Unauthorized,
            402 => Self::PaymentMethodRequired,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            408 => Self::TimedOut,
            409 => Self::AlreadyExists,
            422 => Self::IdempotencyError,
            429 => Self::RateLimitExceeded,
            500 => Self::InternalServerError,
            502 => Self::BadGateway,
            503 => Self::ServiceUnavailable,
            504 => Self::TimedOut,
            _ => Self::Unknown,
        }
    }

    /// Maps network error to our error kind
    pub fn from_network_error(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::NetworkTimeout
        } else if error.is_connect() {
            Self::NetworkConnectionFailed
        } else if error.is_request() {
            Self::NetworkIpBlocked
        } else {
            Self::NetworkConnectionFailed
        }
    }
}

impl CoinbaseApiError {
    /// Converts API error to our error response
    pub fn to_error_response(&self, status_code: u16) -> CoinbaseErrorResponse {
        let kind = if let Some(ref error_type) = self.error_type {
            CoinbaseErrorKind::from_api_error_type(error_type)
        } else {
            CoinbaseErrorKind::from_status_code(status_code)
        };

        CoinbaseErrorResponse {
            reason: self.error_message.clone().unwrap_or_else(|| {
                format!(
                    "API error ({}): {}",
                    status_code,
                    self.error_type.as_deref().unwrap_or("unknown")
                )
            }),
            kind,
            status_code: Some(status_code),
            correlation_id: self.correlation_id.clone(),
        }
    }
}
