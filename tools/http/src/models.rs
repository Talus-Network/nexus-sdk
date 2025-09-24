use {
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

/// JSON Schema definition for validation
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct HttpJsonSchema {
    /// The name of the schema
    pub name: String,
    /// The JSON schema for validation
    pub schema: schemars::Schema,
    /// Description of the expected format
    #[serde(default)]
    pub description: Option<String>,
    /// Whether to enable strict schema adherence
    #[serde(default)]
    pub strict: Option<bool>,
}

/// Schema validation details returned in response
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaValidationDetails {
    /// Name of the schema that was used
    pub name: String,
    /// Description of the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether strict mode was enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    /// Validation result
    pub valid: bool,
    /// Validation errors (if any)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// HTTP Method enum for type-safe method handling
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl Default for HttpMethod {
    fn default() -> Self {
        HttpMethod::Get
    }
}

/// Convert HttpMethod to reqwest::Method
impl From<HttpMethod> for reqwest::Method {
    fn from(method: HttpMethod) -> Self {
        match method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
            HttpMethod::Put => reqwest::Method::PUT,
            HttpMethod::Delete => reqwest::Method::DELETE,
            HttpMethod::Patch => reqwest::Method::PATCH,
            HttpMethod::Head => reqwest::Method::HEAD,
            HttpMethod::Options => reqwest::Method::OPTIONS,
        }
    }
}

/// URL input - either complete URL or split into base_url + path
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum UrlInput {
    /// Complete URL (e.g., "https://api.example.com/users")
    FullUrl(String),
    /// Split URL into base_url and path
    SplitUrl {
        /// Base URL (e.g., "https://api.example.com")
        base_url: String,
        /// Path (e.g., "/users")
        path: String,
    },
}

/// Authentication configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum AuthConfig {
    /// No authentication
    None,
    /// Bearer token authentication
    BearerToken {
        /// The bearer token
        token: String,
    },
    /// API key in header
    ApiKeyHeader {
        /// The API key
        key: String,
        /// Custom header name (default: "X-API-Key")
        #[serde(default)]
        header_name: Option<String>,
    },
    /// API key in query parameter
    ApiKeyQuery {
        /// The API key
        key: String,
        /// Custom parameter name (default: "api_key")
        #[serde(default)]
        param_name: Option<String>,
    },
    /// Basic authentication
    BasicAuth {
        /// Username
        username: String,
        /// Password
        password: String,
    },
}

/// Request body configuration
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestBody {
    /// JSON request body
    Json {
        /// JSON data as serde_json::Value
        data: Value,
    },
    /// Form URL encoded request body
    Form {
        /// Form data as key-value pairs
        data: std::collections::HashMap<String, String>,
    },
    /// Multipart form data (for file uploads)
    Multipart {
        /// Multipart fields
        fields: Vec<MultipartField>,
    },
    /// Raw bytes request body
    Raw {
        /// Raw data as base64 encoded string
        data: String,
        /// Optional content type (default: application/octet-stream)
        #[serde(skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
    },
}

/// Multipart field for form data
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct MultipartField {
    /// Field name
    pub name: String,
    /// Field value (text only)
    pub value: String,
    /// Optional content type (default: text/plain)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}
