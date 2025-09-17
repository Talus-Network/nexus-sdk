//! # `xyz.taluslabs.http.generic@1`
//!
//! Generic HTTP tool that can make requests to any API endpoint.

use {
    crate::{
        errors::HttpToolError,
        helpers::validate_schema_detailed,
        models::{
            AuthConfig,
            HttpJsonSchema,
            HttpMethod,
            RequestBody,
            SchemaValidationDetails,
            UrlInput,
        },
    },
    base64::Engine,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    std::collections::HashMap,
    url::Url,
    warp::http::StatusCode,
};

/// Input model for the HTTP Generic tool
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)
    pub method: HttpMethod,

    /// URL input - either complete URL or split into base_url + path
    pub url: UrlInput,

    /// HTTP headers to include in the request
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,

    /// Query parameters to include in the request
    #[serde(default)]
    pub query: Option<HashMap<String, String>>,

    /// Authentication configuration
    #[serde(default)]
    pub auth: Option<AuthConfig>,

    /// Request body configuration
    #[serde(default)]
    pub body: Option<RequestBody>,

    /// Whether to expect JSON response
    #[serde(default)]
    pub expect_json: Option<bool>,

    /// Optional JSON schema to validate the response against
    #[serde(default)]
    pub json_schema: Option<HttpJsonSchema>,

    /// Request timeout in milliseconds (default: 30000)
    #[serde(default)]
    pub timeout_ms: Option<u64>,

    /// Number of retries on failure (default: 0)
    #[serde(default)]
    pub retries: Option<u32>,

    /// Whether to follow redirects (default: true)
    #[serde(default)]
    pub follow_redirects: Option<bool>,

    /// If true, tolerate empty body when JSON is expected (e.g., 204 No Content)
    #[serde(default)]
    pub allow_empty_json: Option<bool>,
}

impl Input {
    /// Validate input parameters
    pub fn validate(&self) -> Result<(), String> {
        // If json_schema is provided, expect_json must be true
        if self.json_schema.is_some() {
            match self.expect_json {
                Some(true) => Ok(()),
                Some(false) => {
                    Err("expect_json must be true when json_schema is provided".to_string())
                }
                None => {
                    Err("expect_json must be set to true when json_schema is provided".to_string())
                }
            }
        } else {
            Ok(())
        }?;

        // Validate body configuration
        if let Some(body) = &self.body {
            match body {
                RequestBody::Multipart { fields } => {
                    for field in fields {
                        if field.name.is_empty() {
                            return Err("Multipart field name cannot be empty".to_string());
                        }
                        if field.value.is_empty() {
                            return Err("Multipart field value cannot be empty".to_string());
                        }
                    }
                }
                RequestBody::Raw { data, .. } => {
                    if data.is_empty() {
                        return Err("Raw body data cannot be empty".to_string());
                    }
                    // Validate base64 encoding
                    if base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .is_err()
                    {
                        return Err("Raw body data must be valid base64".to_string());
                    }
                }
                RequestBody::Form { data } => {
                    if data.is_empty() {
                        return Err("Form body data cannot be empty".to_string());
                    }
                }
                RequestBody::Json { data } => {
                    // JSON validation is handled by serde
                    if data.is_null() {
                        return Err("JSON body data cannot be null".to_string());
                    }
                }
            }
        }

        // Validate timeout_ms
        if let Some(timeout_ms) = self.timeout_ms {
            if timeout_ms == 0 {
                return Err("timeout_ms must be greater than 0".to_string());
            }
            if timeout_ms > 300000 {
                // 5 minutes max
                return Err("timeout_ms cannot exceed 300000ms (5 minutes)".to_string());
            }
        }

        // Validate retries
        if let Some(retries) = self.retries {
            if retries > 10 {
                return Err("retries cannot exceed 10".to_string());
            }
        }

        Ok(())
    }
}

/// Output model for the HTTP Generic tool
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    /// Successful response
    Ok {
        /// HTTP status code
        status: u16,
        /// Response headers
        headers: HashMap<String, String>,
        /// Raw response body (base64 encoded)
        raw_base64: String,
        /// Text representation (if UTF-8 decodable)
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// JSON data (if parseable)
        #[serde(skip_serializing_if = "Option::is_none")]
        json: Option<Value>,
        /// Schema validation details (if validation was performed)
        #[serde(skip_serializing_if = "Option::is_none")]
        schema_validation: Option<SchemaValidationDetails>,
    },
    /// HTTP error response
    ErrHttp {
        /// HTTP status code
        status: u16,
        /// Error reason
        reason: String,
        /// Response snippet for debugging
        snippet: String,
    },
    /// JSON parsing error
    ErrJsonParse {
        /// Error message
        msg: String,
    },
    /// Schema validation error
    ErrSchemaValidation {
        /// List of validation errors
        errors: Vec<String>,
    },
    /// Network error
    ErrNetwork {
        /// Error message
        msg: String,
    },
    /// Input validation error
    ErrInput {
        /// Error message
        msg: String,
    },
}

/// HTTP Generic tool implementation
pub(crate) struct Http;

impl NexusTool for Http {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.http.generic@1")
    }

    fn path() -> &'static str {
        "/http"
    }

    fn description() -> &'static str {
        "Generic HTTP tool that can make requests to any API endpoint."
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        // Validate input parameters
        if let Err(msg) = input.validate() {
            return HttpToolError::Input(format!("Input validation failed: {}", msg)).to_output();
        }

        // Resolve URL from input with proper validation
        let resolved_url = match &input.url {
            UrlInput::FullUrl(url) => match Url::parse(url) {
                Ok(url) => url,
                Err(e) => {
                    return HttpToolError::UrlParse(e).to_output();
                }
            },
            UrlInput::SplitUrl { base_url, path } => {
                let base = match Url::parse(base_url) {
                    Ok(url) => url,
                    Err(e) => {
                        return HttpToolError::UrlParse(e).to_output();
                    }
                };
                match base.join(path.trim_start_matches('/')) {
                    Ok(url) => url,
                    Err(e) => {
                        return HttpToolError::UrlParse(e).to_output();
                    }
                }
            }
        };

        // Build client with timeout and redirect configuration
        let timeout_ms = input.timeout_ms.unwrap_or(30000);
        let follow_redirects = input.follow_redirects.unwrap_or(true);
        let client = match Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if follow_redirects {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                return HttpToolError::Network(e).to_output();
            }
        };

        // Execute with retry logic
        let retries = input.retries.unwrap_or(0);
        let mut last_error = None;

        for attempt in 0..=retries {
            let mut request = client.request(input.method.clone().into(), resolved_url.as_str());

            // Add headers if provided
            if let Some(headers) = &input.headers {
                for (key, value) in headers {
                    request = request.header(key, value);
                }
            }

            // Add query parameters if provided
            if let Some(query) = &input.query {
                request = request.query(query);
            }

            // Handle authentication
            if let Some(auth) = &input.auth {
                match auth {
                    AuthConfig::None => {
                        // No authentication needed
                    }
                    AuthConfig::BearerToken { token } => {
                        request = request.header("Authorization", format!("Bearer {}", token));
                    }
                    AuthConfig::ApiKeyHeader { key, header_name } => {
                        let header = header_name.as_deref().unwrap_or("X-API-Key");
                        request = request.header(header, key);
                    }
                    AuthConfig::ApiKeyQuery { key, param_name } => {
                        let param = param_name.as_deref().unwrap_or("api_key");
                        // Add to existing query parameters or create new ones
                        let mut query_params = input.query.clone().unwrap_or_default();
                        query_params.insert(param.to_string(), key.clone());
                        request = request.query(&query_params);
                    }
                    AuthConfig::BasicAuth { username, password } => {
                        request = request.basic_auth(username, Some(password));
                    }
                }
            }

            // Handle request body
            if let Some(body) = &input.body {
                request = match body {
                    RequestBody::Json { data } => request.json(data),
                    RequestBody::Form { data } => request.form(data),
                    RequestBody::Multipart { fields } => {
                        let mut form = reqwest::multipart::Form::new();
                        for field in fields {
                            // Try to decode as base64 to decide whether it's a file upload
                            let maybe_bytes: Option<Vec<u8>> =
                                base64::engine::general_purpose::STANDARD
                                    .decode(&field.value)
                                    .ok();

                            // Build initial part
                            let mut part = if let Some(ref bytes) = maybe_bytes {
                                // Treat as file bytes
                                reqwest::multipart::Part::bytes(bytes.clone())
                            } else {
                                // Treat as text field
                                reqwest::multipart::Part::text(field.value.clone())
                            };

                            // Apply content type if provided; handle consume semantics safely
                            if let Some(ref ct) = field.content_type {
                                let applied = part.mime_str(ct);
                                part = match applied {
                                    Ok(p) => p,
                                    Err(_) => {
                                        // Rebuild base part on failure
                                        if let Some(ref bytes) = maybe_bytes {
                                            reqwest::multipart::Part::bytes(bytes.clone())
                                        } else {
                                            reqwest::multipart::Part::text(field.value.clone())
                                        }
                                    }
                                };
                            }

                            // Apply filename if provided
                            if let Some(ref fname) = field.filename {
                                part = part.file_name(fname.clone());
                            }

                            form = form.part(field.name.clone(), part);
                        }
                        request.multipart(form)
                    }
                    RequestBody::Raw { data, content_type } => {
                        let bytes = match base64::engine::general_purpose::STANDARD.decode(data) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                return HttpToolError::Base64Decode(format!(
                                    "Invalid base64 data in multipart field: {}",
                                    e
                                ))
                                .to_output();
                            }
                        };

                        let mut request = request.body(bytes);

                        if let Some(ct) = content_type {
                            request = request.header("Content-Type", ct);
                        }

                        request
                    }
                };
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();

                    // Check if it's an HTTP error status
                    if status >= 400 {
                        // Only retry on 5xx server errors, do not retry on 4xx client errors
                        if status >= 500 && attempt < retries {
                            let delay_ms = 1000 * (attempt + 1) as u64; // 1s, 2s, 3s...
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            continue; // Retry
                        }

                        let reason_phrase = response.status().canonical_reason().unwrap_or("");
                        let body = response.text().await.unwrap_or_default();
                        let snippet = if body.len() > 200 {
                            format!("{}...", &body[..200])
                        } else {
                            body
                        };

                        return HttpToolError::Http {
                            status,
                            reason: if reason_phrase.is_empty() {
                                format!("HTTP error: {}", status)
                            } else {
                                format!("HTTP error: {} ({})", status, reason_phrase)
                            },
                            snippet,
                        }
                        .to_output();
                    }

                    // Get response headers
                    let headers: HashMap<String, String> = response
                        .headers()
                        .iter()
                        .map(|(name, value)| {
                            (name.to_string(), value.to_str().unwrap_or("").to_string())
                        })
                        .collect();

                    // Get raw response body as bytes
                    let body_bytes = match response.bytes().await {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            return HttpToolError::Network(e).to_output();
                        }
                    };

                    // Encode raw body as base64
                    let raw_base64 = base64::engine::general_purpose::STANDARD.encode(&body_bytes);

                    // Try to decode as UTF-8 text
                    let text = String::from_utf8(body_bytes.to_vec()).ok();

                    // Detect JSON content-type from collected headers
                    let is_json_content_type = headers
                        .get("content-type")
                        .map(|s| {
                            let s_lower = s.to_ascii_lowercase();
                            s_lower.contains("application/json") || s_lower.contains("+json")
                        })
                        .unwrap_or(false);

                    // Parse JSON only if expected or content-type signals JSON
                    let should_try_parse_json =
                        input.expect_json.unwrap_or(false) || is_json_content_type;
                    let json = if should_try_parse_json {
                        if let Some(ref text_content) = text {
                            if text_content.trim().is_empty() {
                                // If expect_json=true but we tolerate empty body (e.g., 204)
                                if input.expect_json.unwrap_or(false)
                                    && !input.allow_empty_json.unwrap_or(false)
                                {
                                    return HttpToolError::Input(
                                        "Empty response body but JSON expected".to_string(),
                                    )
                                    .to_output();
                                }
                                None
                            } else {
                                match serde_json::from_str(text_content) {
                                    Ok(json_data) => Some(json_data),
                                    Err(e) => {
                                        if input.expect_json.unwrap_or(false)
                                            || is_json_content_type
                                        {
                                            return HttpToolError::JsonParse(e).to_output();
                                        }
                                        None
                                    }
                                }
                            }
                        } else {
                            if input.expect_json.unwrap_or(false)
                                && !input.allow_empty_json.unwrap_or(false)
                            {
                                return HttpToolError::Input(
                                    "Non-text response body but JSON expected".to_string(),
                                )
                                .to_output();
                            }
                            None
                        }
                    } else {
                        None
                    };

                    // Schema validation (if schema provided)
                    let schema_validation = if let Some(schema_def) = &input.json_schema {
                        if let Some(ref json_data) = json {
                            Some(validate_schema_detailed(schema_def, json_data))
                        } else {
                            // JSON could not be parsed, schema validation failed
                            Some(SchemaValidationDetails {
                                name: schema_def.name.clone(),
                                description: schema_def.description.clone(),
                                strict: schema_def.strict,
                                valid: false,
                                errors: vec!["JSON could not be parsed".to_string()],
                            })
                        }
                    } else {
                        None // No schema, no validation performed
                    };

                    // If schema validation failed, handle based on strict mode
                    if let Some(ref validation) = schema_validation {
                        if !validation.valid {
                            if validation.strict.unwrap_or(false) {
                                // Strict mode: Return error immediately
                                return HttpToolError::SchemaValidation {
                                    errors: validation.errors.clone(),
                                }
                                .to_output();
                            }
                            // Non-strict mode: Continue with validation details in response
                            // (validation.valid = false, errors filled, but return Output::Ok)
                        }
                    }

                    return Output::Ok {
                        status,
                        headers,
                        raw_base64,
                        text,
                        json,
                        schema_validation,
                    };
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < retries {
                        // Exponential backoff: 100ms, 200ms, 400ms, etc.
                        let delay_ms = 100 * (2_u64.pow(attempt));
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        // If we get here, all retries failed
        HttpToolError::Input(format!(
            "Request failed after {} retries: {:?}",
            retries, last_error
        ))
        .to_output()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server};

    #[tokio::test]
    async fn test_http_get() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "GET", "url": "http://example.com/get", "args": {}}"#;
        let _mock = server
            .mock("GET", "/get")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/get", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok {
                status,
                headers,
                raw_base64,
                text,
                json,
                schema_validation,
            } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                assert!(!raw_base64.is_empty()); // GET should have body
                assert!(text.is_some()); // Should be UTF-8 decodable
                assert!(json.is_some()); // Should be JSON parseable
                assert!(schema_validation.is_none());
            }
            Output::ErrHttp { reason, .. } => {
                panic!("Expected success, got HTTP error: {}", reason)
            }
            Output::ErrNetwork { msg } => panic!("Expected success, got network error: {}", msg),
            Output::ErrJsonParse { msg } => {
                panic!("Expected success, got JSON parse error: {}", msg)
            }
            Output::ErrSchemaValidation { errors } => panic!(
                "Expected success, got schema validation error: {:?}",
                errors
            ),
            Output::ErrInput { msg } => panic!("Expected success, got input error: {}", msg),
        }
    }

    #[tokio::test]
    async fn test_http_head() {
        let tool = Http::new().await;

        // Create mock server - HEAD requests typically don't return body
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("HEAD", "/head")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("content-length", "0")
            .create();

        let input = Input {
            method: HttpMethod::Head,
            url: UrlInput::FullUrl(format!("{}/head", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok {
                status,
                headers,
                raw_base64: _,
                text: _,
                json: _,
                schema_validation,
            } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                // raw_base64 can be empty for HEAD requests
                assert!(schema_validation.is_none());
            }
            Output::ErrHttp { reason, .. } => {
                panic!("Expected success, got HTTP error: {}", reason)
            }
            Output::ErrNetwork { msg } => panic!("Expected success, got network error: {}", msg),
            Output::ErrJsonParse { msg } => {
                panic!("Expected success, got JSON parse error: {}", msg)
            }
            Output::ErrSchemaValidation { errors } => panic!(
                "Expected success, got schema validation error: {:?}",
                errors
            ),
            Output::ErrInput { msg } => panic!("Expected success, got input error: {}", msg),
        }
    }

    #[tokio::test]
    async fn test_http_404_error() {
        let tool = Http::new().await;

        // Create mock server for 404 error
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/notfound")
            .with_status(404)
            .with_header("content-type", "text/html")
            .with_body("<html><body><h1>404 Not Found</h1></body></html>")
            .create();

        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/notfound", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::ErrHttp {
                status,
                reason,
                snippet,
            } => {
                assert_eq!(status, 404);
                assert!(reason.contains("HTTP error"));
                // Snippet might be empty for 404 responses, that's ok
                assert!(snippet.len() <= 200); // Should be truncated if long
            }
            _ => panic!("Expected ErrHttp, got different output"),
        }
    }

    #[tokio::test]
    async fn test_schema_validation_function() {
        // Test the validate_schema function directly
        let schema = HttpJsonSchema {
            name: "TestSchema".to_string(),
            schema: schemars::schema_for!(serde_json::Value),
            description: Some("Test schema".to_string()),
            strict: Some(false),
        };

        let valid_json = serde_json::json!({"name": "test", "value": 123});
        let invalid_json = serde_json::json!("invalid");

        // Test valid JSON
        let result = validate_schema_detailed(&schema, &valid_json);
        assert!(result.valid);
        assert_eq!(result.name, "TestSchema");
        assert_eq!(result.description, Some("Test schema".to_string()));
        assert_eq!(result.strict, Some(false));
        assert!(result.errors.is_empty());

        // Test invalid JSON (should still pass because schema is very permissive)
        let result2 = validate_schema_detailed(&schema, &invalid_json);
        assert!(result2.valid); // Very permissive schema
    }

    #[tokio::test]
    async fn test_json_parse_error() {
        let tool = Http::new().await;

        // Create mock server that returns invalid JSON
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/invalid-json")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("This is not valid JSON")
            .create();

        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/invalid-json", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::ErrJsonParse { msg } => {
                assert!(msg.contains("Failed to parse JSON"));
                println!("JSON parse error message: {}", msg);
            }
            Output::Ok { text, .. } => {
                // If response is not JSON but text, we should not get a JSON parse error
                if let Some(text_content) = text {
                    if !text_content.trim().is_empty() {
                        // Text response but not JSON - this is normal
                        println!("Got text response (not JSON): {}", text_content);
                    }
                }
            }
            _ => {
                // Other cases might also be possible (network error vs.)
                println!("Got different output type");
            }
        }
    }

    #[tokio::test]
    async fn test_url_split() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response =
            r#"{"method": "GET", "url": "http://example.com/api/users", "args": {}}"#;
        let _mock = server
            .mock("GET", "/api/users")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test SplitUrl
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::SplitUrl {
                base_url: server.url(),
                path: "/api/users".to_string(),
            },
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok {
                status,
                headers,
                raw_base64,
                text,
                json,
                schema_validation,
            } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                assert!(!raw_base64.is_empty());
                assert!(text.is_some());
                assert!(json.is_some());
                assert!(schema_validation.is_none());
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_headers_and_query() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "GET", "url": "http://example.com/api/users?page=1&limit=10", "headers": {"Authorization": "Bearer token123", "Content-Type": "application/json"}}"#;
        let _mock = server
            .mock("GET", "/api/users")
            .match_query(mockito::Matcher::Regex(
                r"page=1.*limit=10|limit=10.*page=1".to_string(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test with headers and query parameters
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::SplitUrl {
                base_url: server.url(),
                path: "/api/users".to_string(),
            },
            headers: Some(HashMap::from([
                ("Authorization".to_string(), "Bearer token123".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ])),
            query: Some(HashMap::from([
                ("page".to_string(), "1".to_string()),
                ("limit".to_string(), "10".to_string()),
            ])),
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok {
                status,
                headers,
                raw_base64,
                text,
                json,
                schema_validation,
            } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                assert!(!raw_base64.is_empty());
                assert!(text.is_some());
                assert!(json.is_some());
                assert!(schema_validation.is_none());
            }
            _ => {
                println!("Got unexpected output: {:?}", output);
                panic!("Expected successful response");
            }
        }
    }

    #[tokio::test]
    async fn test_auth_bearer_token() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"authenticated": true, "token": "test-token"}"#;
        let _mock = server
            .mock("GET", "/auth")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test Bearer token authentication
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/auth", server.url())),
            headers: None,
            query: None,
            auth: Some(AuthConfig::BearerToken {
                token: "test-token".to_string(),
            }),
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_auth_api_key_header() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"authenticated": true, "api_key": "test-key"}"#;
        let _mock = server
            .mock("GET", "/auth")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test API key in header
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/auth", server.url())),
            headers: None,
            query: None,
            auth: Some(AuthConfig::ApiKeyHeader {
                key: "test-key".to_string(),
                header_name: Some("X-API-Key".to_string()),
            }),
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_auth_api_key_query() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"authenticated": true, "api_key": "test-key"}"#;
        let _mock = server
            .mock("GET", "/auth")
            .match_query("api_key=test-key")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test API key in query
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/auth", server.url())),
            headers: None,
            query: None,
            auth: Some(AuthConfig::ApiKeyQuery {
                key: "test-key".to_string(),
                param_name: Some("api_key".to_string()),
            }),
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_auth_basic() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"authenticated": true, "user": "testuser"}"#;
        let _mock = server
            .mock("GET", "/auth")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test Basic authentication
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/auth", server.url())),
            headers: None,
            query: None,
            auth: Some(AuthConfig::BasicAuth {
                username: "testuser".to_string(),
                password: "testpass".to_string(),
            }),
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_health() {
        let tool = Http::new().await;
        assert!(matches!(tool.health().await, Ok(StatusCode::OK)));
    }

    #[tokio::test]
    async fn test_input_validation() {
        // Test valid case: json_schema provided with expect_json = true
        let valid_input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl("https://httpbin.org/get".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: Some(true),
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };
        assert!(valid_input.validate().is_ok());

        // Test invalid case: json_schema provided with expect_json = false
        let invalid_input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl("https://httpbin.org/get".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: Some(false),
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };
        assert!(invalid_input.validate().is_err());

        // Test invalid case: json_schema provided with expect_json = None
        let invalid_input2 = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl("https://httpbin.org/get".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };
        assert!(invalid_input2.validate().is_err());

        // Test valid case: no json_schema provided
        let valid_input2 = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl("https://httpbin.org/get".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };
        assert!(valid_input2.validate().is_ok());
    }

    #[tokio::test]
    async fn test_invoke_with_invalid_input() {
        let tool = Http::new().await;

        // Test with json_schema but expect_json = false
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl("https://httpbin.org/get".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: Some(false),
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::ErrInput { msg } => {
                assert!(msg.contains("expect_json must be true when json_schema is provided"));
            }
            _ => panic!("Expected ErrInput with validation error"),
        }
    }

    #[tokio::test]
    async fn test_json_body() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "POST", "url": "http://example.com/post", "data": {"name": "test", "value": 123}}"#;
        let _mock = server
            .mock("POST", "/post")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let input = Input {
            method: HttpMethod::Post,
            url: UrlInput::FullUrl(format!("{}/post", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: Some(RequestBody::Json {
                data: serde_json::json!({
                    "name": "test",
                    "value": 123
                }),
            }),
            expect_json: Some(true),
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_raw_body() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response =
            r#"{"method": "POST", "url": "http://example.com/post", "data": "binary data"}"#;
        let _mock = server
            .mock("POST", "/post")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let input = Input {
            method: HttpMethod::Post,
            url: UrlInput::FullUrl(format!("{}/post", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: Some(RequestBody::Raw {
                data: base64::engine::general_purpose::STANDARD.encode("Hello World"),
                content_type: Some("application/octet-stream".to_string()),
            }),
            expect_json: Some(true),
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_body_validation() {
        // Test empty multipart field name
        let input = Input {
            method: HttpMethod::Post,
            url: UrlInput::FullUrl("https://httpbin.org/post".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: Some(RequestBody::Multipart {
                fields: vec![crate::models::MultipartField {
                    name: "".to_string(),
                    value: "test".to_string(),
                    filename: None,
                    content_type: None,
                }],
            }),
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        assert!(input.validate().is_err());

        // Test empty raw body data
        let input2 = Input {
            method: HttpMethod::Post,
            url: UrlInput::FullUrl("https://httpbin.org/post".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: Some(RequestBody::Raw {
                data: "".to_string(),
                content_type: None,
            }),
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        assert!(input2.validate().is_err());

        // Test invalid base64
        let input3 = Input {
            method: HttpMethod::Post,
            url: UrlInput::FullUrl("https://httpbin.org/post".to_string()),
            headers: None,
            query: None,
            auth: None,
            body: Some(RequestBody::Raw {
                data: "invalid base64!".to_string(),
                content_type: None,
            }),
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        assert!(input3.validate().is_err());
    }

    #[tokio::test]
    async fn test_timeout_configuration() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "GET", "url": "http://example.com/get", "args": {}}"#;
        let _mock = server
            .mock("GET", "/get")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test with custom timeout
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/get", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: Some(5000), // 5 second timeout
            retries: None,
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_retries_configuration() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "GET", "url": "http://example.com/get", "args": {}}"#;
        let _mock = server
            .mock("GET", "/get")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test with retries = 2
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/get", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: Some(2), // 2 retries
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => panic!("Expected successful response"),
        }
    }

    #[tokio::test]
    async fn test_retry_on_server_errors() {
        let tool = Http::new().await;

        // Create mock server that returns 500 error first, then 200
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "GET", "url": "http://example.com/get", "args": {}}"#;

        // First request returns 500
        let _error_mock = server
            .mock("GET", "/retry-test")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "Internal Server Error"}"#)
            .expect(1) // Expect exactly 1 call
            .create();

        // Second request returns 200
        let _success_mock = server
            .mock("GET", "/retry-test")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .expect(1) // Expect exactly 1 call
            .create();

        // Test with retries = 1
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/retry-test", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: Some(1), // 1 retry
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200); // Should succeed after retry
            }
            _ => panic!("Expected successful response after retry"),
        }
    }

    #[tokio::test]
    async fn test_no_retry_on_client_errors() {
        let tool = Http::new().await;

        // Create mock server that returns 404 error
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/notfound")
            .with_status(404)
            .with_header("content-type", "text/html")
            .with_body("<html><body><h1>404 Not Found</h1></body></html>")
            .expect(1) // Should only be called once (no retry)
            .create();

        // Test with retries = 2
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/notfound", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: Some(2), // 2 retries available
            follow_redirects: None,
            allow_empty_json: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::ErrHttp { status, .. } => {
                assert_eq!(status, 404); // Should return 404 without retry
            }
            _ => panic!("Expected ErrHttp with 404 status"),
        }
    }

    #[tokio::test]
    async fn test_follow_redirects_configuration() {
        let tool = Http::new().await;

        // Create mock server with redirect
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "GET", "url": "http://example.com/get", "args": {}}"#;

        // Mock redirect endpoint
        let _redirect_mock = server
            .mock("GET", "/redirect")
            .with_status(302)
            .with_header("location", "/get")
            .create();

        // Mock final endpoint
        let _final_mock = server
            .mock("GET", "/get")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        // Test with follow_redirects = true (default behavior)
        let input = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/redirect", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: Some(true),
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: Some(true),
            allow_empty_json: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200); // Should follow redirect and get 200
            }
            _ => panic!("Expected successful response with redirect following"),
        }

        // Test with follow_redirects = false
        let input_no_redirect = Input {
            method: HttpMethod::Get,
            url: UrlInput::FullUrl(format!("{}/redirect", server.url())),
            headers: None,
            query: None,
            auth: None,
            body: None,
            expect_json: None,
            json_schema: None,
            timeout_ms: None,
            retries: None,
            follow_redirects: Some(false),
            allow_empty_json: None,
        };

        let result_no_redirect = tool.invoke(input_no_redirect).await;
        match result_no_redirect {
            Output::Ok { status, .. } => {
                assert_eq!(status, 302); // Should get redirect status without following
            }
            _ => panic!("Expected redirect response without following"),
        }
    }
}
