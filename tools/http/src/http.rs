//! # `xyz.taluslabs.http.generic@1`
//!
//! Generic HTTP tool that can make requests to any API endpoint.

use {
    crate::models::{Input, Output},
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    reqwest::Client,
    warp::http::StatusCode,
};

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
        let client = Client::new();
        
        let request = match input.method.to_uppercase().as_str() {
            "GET" => client.get(&input.url),
            "POST" => client.post(&input.url),
            "PUT" => client.put(&input.url),
            "DELETE" => client.delete(&input.url),
            _ => {
                return Output::Err {
                    message: format!("Unsupported HTTP method: {}", input.method),
                };
            }
        };

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();

                let body = match response.text().await {
                    Ok(text) => text,
                    Err(e) => {
                        return Output::Err {
                            message: format!("Failed to read response body: {}", e),
                        };
                    }
                };

                Output::Ok {
                    status,
                    body,
                }
            }
            Err(e) => Output::Err {
                message: format!("Request failed: {}", e),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_get() {
        let tool = Http::new().await;

        let input = Input {
            method: "GET".to_string(),
            url: "https://httpbin.org/get".to_string(),
        };

        let output = tool.invoke(input).await;

        match output {
            Output::Ok { status, .. } => {
                assert_eq!(status, 200);
            }
            Output::Err { message } => panic!("Expected success, got error: {}", message),
        }
    }

    #[tokio::test]
    async fn test_health() {
        let tool = Http::new().await;
        assert!(matches!(tool.health().await, Ok(StatusCode::OK)));
    }
}