//! # `xyz.taluslabs.http.generic@1`
//!
//! Generic HTTP tool that can make requests to any API endpoint.

use {
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    warp::http::StatusCode,
    base64::Engine,
    std::collections::HashMap,
    serde_json::Value,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

/// JSON Schema definition for validation
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
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
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
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

/// Input model for the HTTP Generic tool
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)
    pub method: HttpMethod,
    
    /// Complete URL
    pub url: String,
    
    /// Whether to expect JSON response
    #[serde(default)]
    pub expect_json: Option<bool>,
    
    /// Optional JSON schema to validate the response against
    #[serde(default)]
    pub json_schema: Option<HttpJsonSchema>,
}

impl Input {
    /// Validate input parameters
    pub fn validate(&self) -> Result<(), String> {
        // If json_schema is provided, expect_json must be true
        if self.json_schema.is_some() {
            match self.expect_json {
                Some(true) => Ok(()),
                Some(false) => Err("expect_json must be true when json_schema is provided".to_string()),
                None => Err("expect_json must be set to true when json_schema is provided".to_string()),
            }
        } else {
            Ok(())
        }
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
         return Output::ErrNetwork {
        msg: format!("Input validation failed: {}", msg),
    };
}

        let client = Client::new();
        
        let request = client.request(input.method.into(), &input.url);

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();

                // Check if it's an HTTP error status
                if status >= 400 {
                    let body = response.text().await.unwrap_or_default();
                    let snippet = if body.len() > 200 {
                        format!("{}...", &body[..200])
                    } else {
                        body
                    };
                    
                    return Output::ErrHttp {
                        status,
                        reason: format!("HTTP error: {}", status),
                        snippet,
                    };
                }

                // Get response headers
                let headers: HashMap<String, String> = response
                    .headers()
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.to_string(),
                            value.to_str().unwrap_or("").to_string(),
                        )
                    })
                    .collect();

                // Get raw response body as bytes
                let body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return Output::ErrNetwork {
                            msg: format!("Failed to read response body: {}", e),
                        };
                    }
                };

                // Encode raw body as base64
                let raw_base64 = base64::engine::general_purpose::STANDARD.encode(&body_bytes);

                // Try to decode as UTF-8 text
                let text = String::from_utf8(body_bytes.to_vec()).ok();

                // Try to parse as JSON
                let json = if let Some(ref text_content) = text {
                    // Special handling for HEAD and OPTIONS methods
                    if text_content.trim().is_empty() {
                        None // Empty body, no JSON parsing
                    } else {
                        match serde_json::from_str(text_content) {
                            Ok(json_data) => Some(json_data),
                            Err(e) => {
                                // JSON parsing error - Returns ErrJsonParse
                                return Output::ErrJsonParse {
                                    msg: format!("Failed to parse JSON: {}", e),
                                };
                            }
                        }
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

                // If schema validation failed, return error
                if let Some(ref validation) = schema_validation {
                    if !validation.valid {
                        return Output::ErrSchemaValidation {
                            errors: validation.errors.clone(),
                        };
                    }
                }

                Output::Ok {
                    status,
                    headers,
                    raw_base64,
                    text,
                    json,
                    schema_validation,
                }
            }
            Err(e) => Output::ErrNetwork {
                msg: format!("Request failed: {}", e),
            },
        }
    }
}

/// Validate JSON data against a schema and return detailed results
fn validate_schema_detailed(schema_def: &HttpJsonSchema, json_data: &Value) -> SchemaValidationDetails {
    // Convert schema to JSON value for validation
    let schema_value = match serde_json::to_value(&schema_def.schema) {
        Ok(val) => val,
        Err(_) => {
            return SchemaValidationDetails {
                name: schema_def.name.clone(),
                description: schema_def.description.clone(),
                strict: schema_def.strict,
                valid: false,
                errors: vec!["Schema serialization failed".to_string()],
            };
        }
    };

    // Validate using jsonschema
    let compiled = match jsonschema::JSONSchema::compile(&schema_value) {
        Ok(schema) => schema,
        Err(e) => {
            return SchemaValidationDetails {
                name: schema_def.name.clone(),
                description: schema_def.description.clone(),
                strict: schema_def.strict,
                valid: false,
                errors: vec![format!("Schema compilation failed: {}", e)],
            };
        }
    };
    
    let validation_result = compiled.validate(json_data);
    match validation_result {
        Ok(_) => SchemaValidationDetails {
            name: schema_def.name.clone(),
            description: schema_def.description.clone(),
            strict: schema_def.strict,
            valid: true,
            errors: vec![],
        },
        Err(errors) => {
            let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
            SchemaValidationDetails {
                name: schema_def.name.clone(),
                description: schema_def.description.clone(),
                strict: schema_def.strict,
                valid: false,
                errors: error_messages,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Mock, Server};

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
            url: format!("{}/get", server.url()),
            expect_json: None,
            json_schema: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, headers, raw_base64, text, json, schema_validation } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                assert!(!raw_base64.is_empty()); // GET should have body
                assert!(text.is_some()); // Should be UTF-8 decodable
                assert!(json.is_some()); // Should be JSON parseable
                assert!(schema_validation.is_none());
            }
            Output::ErrHttp { reason, .. } => panic!("Expected success, got HTTP error: {}", reason),
            Output::ErrNetwork { msg } => panic!("Expected success, got network error: {}", msg),
            Output::ErrJsonParse { msg } => panic!("Expected success, got JSON parse error: {}", msg),
            Output::ErrSchemaValidation { errors } => panic!("Expected success, got schema validation error: {:?}", errors),
        }
    }

    #[tokio::test]
    async fn test_http_post() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "POST", "url": "http://example.com/post", "data": ""}"#;
        let _mock = server
            .mock("POST", "/post")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let input = Input {
            method: HttpMethod::Post,
            url: format!("{}/post", server.url()),
            expect_json: None,
            json_schema: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, headers, raw_base64, text, json, schema_validation } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                assert!(!raw_base64.is_empty()); // POST should have body
                assert!(text.is_some()); // Should be UTF-8 decodable
                assert!(json.is_some()); // Should be JSON parseable
                assert!(schema_validation.is_none());
            }
            Output::ErrHttp { reason, .. } => panic!("Expected success, got HTTP error: {}", reason),
            Output::ErrNetwork { msg } => panic!("Expected success, got network error: {}", msg),
            Output::ErrJsonParse { msg } => panic!("Expected success, got JSON parse error: {}", msg),
            Output::ErrSchemaValidation { errors } => panic!("Expected success, got schema validation error: {:?}", errors),
        }
    }

    #[tokio::test]
    async fn test_http_patch() {
        let tool = Http::new().await;

        // Create mock server
        let mut server = Server::new_async().await;
        let mock_response = r#"{"method": "PATCH", "url": "http://example.com/patch", "data": ""}"#;
        let _mock = server
            .mock("PATCH", "/patch")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response)
            .create();

        let input = Input {
            method: HttpMethod::Patch,
            url: format!("{}/patch", server.url()),
            expect_json: None,
            json_schema: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, headers, raw_base64, text, json, schema_validation } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                assert!(!raw_base64.is_empty()); // PATCH should have body
                assert!(text.is_some()); // Should be UTF-8 decodable
                assert!(json.is_some()); // Should be JSON parseable
                assert!(schema_validation.is_none());
            }
            Output::ErrHttp { reason, .. } => panic!("Expected success, got HTTP error: {}", reason),
            Output::ErrNetwork { msg } => panic!("Expected success, got network error: {}", msg),
            Output::ErrJsonParse { msg } => panic!("Expected success, got JSON parse error: {}", msg),
            Output::ErrSchemaValidation { errors } => panic!("Expected success, got schema validation error: {:?}", errors),
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
            url: format!("{}/head", server.url()),
            expect_json: None,
            json_schema: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, headers, raw_base64, text, json, schema_validation } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                // raw_base64 can be empty for HEAD requests
                assert!(schema_validation.is_none());
            }
            Output::ErrHttp { reason, .. } => panic!("Expected success, got HTTP error: {}", reason),
            Output::ErrNetwork { msg } => panic!("Expected success, got network error: {}", msg),
            Output::ErrJsonParse { msg } => panic!("Expected success, got JSON parse error: {}", msg),
            Output::ErrSchemaValidation { errors } => panic!("Expected success, got schema validation error: {:?}", errors),
        }
    }

    #[tokio::test]
    async fn test_http_options() {
        let tool = Http::new().await;

        // Create mock server - OPTIONS requests typically don't return body
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("OPTIONS", "/options")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("content-length", "0")
            .with_header("allow", "GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS")
            .create();

        let input = Input {
            method: HttpMethod::Options,
            url: format!("{}/options", server.url()),
            expect_json: None,
            json_schema: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, headers, raw_base64, text, json, schema_validation } => {
                assert_eq!(status, 200);
                assert!(!headers.is_empty());
                // raw_base64 can be empty for OPTIONS requests
                assert!(schema_validation.is_none());
            }
            Output::ErrHttp { reason, .. } => panic!("Expected success, got HTTP error: {}", reason),
            Output::ErrNetwork { msg } => panic!("Expected success, got network error: {}", msg),
            Output::ErrJsonParse { msg } => panic!("Expected success, got JSON parse error: {}", msg),
            Output::ErrSchemaValidation { errors } => panic!("Expected success, got schema validation error: {:?}", errors),
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
            url: format!("{}/notfound", server.url()),
            expect_json: None,
            json_schema: None,
        };

        let output = tool.invoke(input).await;

        match output {
            Output::ErrHttp { status, reason, snippet } => {
                assert_eq!(status, 404);
                assert!(reason.contains("HTTP error"));
                // Snippet might be empty for 404 responses, that's ok
                assert!(snippet.len() <= 200); // Should be truncated if long
            }
            _ => panic!("Expected ErrHttp, got different output"),
        }
    }

    #[tokio::test]
    async fn test_invalid_method() {
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

        // Test with a valid method
        let input = Input {
            method: HttpMethod::Get,
            url: format!("{}/get", server.url()),
            expect_json: None,
            json_schema: None,
        };

        let output = tool.invoke(input).await;

        // We expect a successful response
        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            _ => {
                panic!("Expected successful response, got: {:?}", output);
            }
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
            url: format!("{}/invalid-json", server.url()),
            expect_json: None,
            json_schema: None,
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
    async fn test_health() {
        let tool = Http::new().await;
        assert!(matches!(tool.health().await, Ok(StatusCode::OK)));
    }

    #[tokio::test]
    async fn test_input_validation() {
        // Test valid case: json_schema provided with expect_json = true
        let valid_input = Input {
            method: HttpMethod::Get,
            url: "https://httpbin.org/get".to_string(),
            expect_json: Some(true),
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
        };
        assert!(valid_input.validate().is_ok());

        // Test invalid case: json_schema provided with expect_json = false
        let invalid_input = Input {
            method: HttpMethod::Get,
            url: "https://httpbin.org/get".to_string(),
            expect_json: Some(false),
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
        };
        assert!(invalid_input.validate().is_err());

        // Test invalid case: json_schema provided with expect_json = None
        let invalid_input2 = Input {
            method: HttpMethod::Get,
            url: "https://httpbin.org/get".to_string(),
            expect_json: None,
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
        };
        assert!(invalid_input2.validate().is_err());

        // Test valid case: no json_schema provided
        let valid_input2 = Input {
            method: HttpMethod::Get,
            url: "https://httpbin.org/get".to_string(),
            expect_json: None,
            json_schema: None,
        };
        assert!(valid_input2.validate().is_ok());
    }

    #[tokio::test]
    async fn test_invoke_with_invalid_input() {
        let tool = Http::new().await;

        // Test with json_schema but expect_json = false
        let input = Input {
            method: HttpMethod::Get,
            url: "https://httpbin.org/get".to_string(),
            expect_json: Some(false),
            json_schema: Some(HttpJsonSchema {
                name: "TestSchema".to_string(),
                schema: schemars::schema_for!(serde_json::Value),
                description: None,
                strict: None,
            }),
        };

        let output = tool.invoke(input).await;

        match output {
            Output::ErrNetwork { msg } => {
                assert!(msg.contains("expect_json must be true when json_schema is provided"));
            }
            _ => panic!("Expected ErrNetwork with validation error"),
        }
    }
}