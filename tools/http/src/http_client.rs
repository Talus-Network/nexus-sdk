//! HTTP Generic client implementation
//!
//! This module provides a clean client for making generic HTTP requests.

use {
    crate::{
        errors::HttpToolError,
        models::{AuthConfig, HttpMethod, RequestBody, UrlInput},
    },
    base64::Engine,
    reqwest::{multipart::Form, Client, Method},
    std::collections::HashMap,
    url::Url,
};

/// HTTP Generic client for making requests
pub struct HttpClient {
    /// HTTP client for making requests
    client: Client,
}

impl HttpClient {
    /// Creates a new HTTP client instance with default configuration
    pub fn new() -> Result<Self, HttpToolError> {
        Self::with_config(None, None) // Default: 30s timeout, follow redirects
    }

    /// Creates a new HTTP client with custom configuration
    pub fn with_config(
        timeout_ms: Option<u64>,
        follow_redirects: Option<bool>,
    ) -> Result<Self, HttpToolError> {
        let mut builder = Client::builder();

        // Set timeout with default (5 seconds = 5000ms)
        let timeout_ms = timeout_ms.unwrap_or(5000);
        builder = builder.timeout(std::time::Duration::from_millis(timeout_ms));

        // Set redirect policy with default (don't follow redirects, following curl's philosophy)
        let follow_redirects = follow_redirects.unwrap_or(false);
        if follow_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::limited(10));
        } else {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        let client = builder.build().map_err(HttpToolError::from_network_error)?;

        Ok(Self { client })
    }

    /// Resolves URL from input with proper validation
    pub fn resolve_url(&self, url_input: &UrlInput) -> Result<Url, HttpToolError> {
        match url_input {
            UrlInput::FullUrl(url) => Url::parse(url).map_err(HttpToolError::from_url_parse_error),
            UrlInput::SplitUrl { base_url, path } => {
                let base = Url::parse(base_url).map_err(HttpToolError::from_url_parse_error)?;
                base.join(path).map_err(HttpToolError::from_url_parse_error)
            }
        }
    }

    /// Builds HTTP method from input
    pub fn build_method(&self, method: &HttpMethod) -> Method {
        method.clone().into()
    }

    /// Builds request with authentication
    pub fn build_request(
        &self,
        method: Method,
        url: Url,
        auth: Option<&AuthConfig>,
        headers: Option<&HashMap<String, String>>,
        query: Option<&HashMap<String, String>>,
    ) -> Result<reqwest::RequestBuilder, HttpToolError> {
        let mut request = self.client.request(method, url);

        // Add authentication
        if let Some(auth) = auth {
            request = self.apply_auth(request, auth)?;
        }

        // Add headers
        if let Some(headers) = headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        // Add query parameters
        if let Some(query) = query {
            request = request.query(query);
        }

        Ok(request)
    }

    /// Applies authentication to request
    fn apply_auth(
        &self,
        request: reqwest::RequestBuilder,
        auth: &AuthConfig,
    ) -> Result<reqwest::RequestBuilder, HttpToolError> {
        match auth {
            AuthConfig::None => Ok(request),
            AuthConfig::BearerToken { token } => Ok(request.bearer_auth(token)),
            AuthConfig::ApiKeyHeader { key, header_name } => {
                let header_name = header_name.as_deref().unwrap_or("X-API-Key");
                Ok(request.header(header_name, key))
            }
            AuthConfig::ApiKeyQuery { key, param_name } => {
                let param_name = param_name.as_deref().unwrap_or("api_key");
                Ok(request.query(&[(param_name, key)]))
            }
            AuthConfig::BasicAuth { username, password } => {
                Ok(request.basic_auth(username, Some(password)))
            }
        }
    }

    /// Check if HTTP method supports request body
    fn method_supports_body(method: &reqwest::Method) -> bool {
        match *method {
            reqwest::Method::GET | reqwest::Method::HEAD | reqwest::Method::OPTIONS => false,
            _ => true,
        }
    }

    /// Builds request body
    pub fn build_body(
        &self,
        request: reqwest::RequestBuilder,
        body: &RequestBody,
        method: &reqwest::Method,
    ) -> Result<reqwest::RequestBuilder, HttpToolError> {
        // Skip body for methods that don't support it
        if !Self::method_supports_body(method) {
            return Ok(request);
        }
        match body {
            RequestBody::Json { data } => Ok(request.json(data)),
            RequestBody::Form { data } => Ok(request.form(data)),
            RequestBody::Multipart { fields } => self.build_multipart_form(request, fields),
            RequestBody::Raw { data, content_type } => {
                self.build_raw_body(request, data, content_type)
            }
        }
    }

    /// Builds multipart form from fields
    fn build_multipart_form(
        &self,
        request: reqwest::RequestBuilder,
        fields: &[crate::models::MultipartField],
    ) -> Result<reqwest::RequestBuilder, HttpToolError> {
        let mut form = Form::new();
        for field in fields {
            let mut part = reqwest::multipart::Part::text(field.value.clone());

            // Set content type if provided
            if let Some(content_type) = &field.content_type {
                part = part
                    .mime_str(content_type)
                    .map_err(|e| HttpToolError::ErrInput(format!("Invalid content type: {}", e)))?;
            }

            form = form.part(field.name.clone(), part);
        }
        Ok(request.multipart(form))
    }

    /// Builds raw body from base64 data
    fn build_raw_body(
        &self,
        request: reqwest::RequestBuilder,
        data: &str,
        content_type: &Option<String>,
    ) -> Result<reqwest::RequestBuilder, HttpToolError> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| HttpToolError::ErrBase64Decode(format!("Invalid base64 data: {}", e)))?;

        let content_type = content_type
            .as_deref()
            .unwrap_or("application/octet-stream");

        Ok(request.header("Content-Type", content_type).body(bytes))
    }

    /// Executes the request and returns the response
    pub async fn execute(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, HttpToolError> {
        request
            .send()
            .await
            .map_err(HttpToolError::from_network_error)
    }

    /// Executes a request with retry logic
    pub async fn execute_with_retry(
        &self,
        request: reqwest::RequestBuilder,
        retries: u32,
    ) -> Result<reqwest::Response, HttpToolError> {
        if retries == 0 {
            // No retries needed, execute once
            return self.execute(request).await;
        }

        let mut last_error = None;

        for attempt in 0..=retries {
            match self
                .execute(request.try_clone().ok_or_else(|| {
                    HttpToolError::ErrInput("Request cannot be cloned for retry".to_string())
                })?)
                .await
            {
                Ok(response) => {
                    let status = response.status().as_u16();

                    // Check if it's a retryable error (5xx server errors)
                    if status >= 500 && attempt < retries {
                        // Server error, retry with linear backoff
                        let delay_ms = 1000 * (attempt + 1) as u64; // 1s, 2s, 3s...
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        continue;
                    }

                    // Success or non-retryable error (4xx), return response
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < retries {
                        // Network error, retry with exponential backoff
                        let delay_ms = 100 * (2_u64.pow(attempt));
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        // All retries failed
        Err(last_error.unwrap_or_else(|| {
            HttpToolError::ErrInput("Request failed after all retries".to_string())
        }))
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP client")
    }
}
