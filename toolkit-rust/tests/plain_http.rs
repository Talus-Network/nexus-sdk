//! Plain HTTP listener used when no TLS credentials are configured. This runs
//! in its own test binary because the TLS integration tests set the TLS
//! environment variables process wide.

use {
    anyhow::Result as AnyResult,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    warp::http::StatusCode,
};

#[derive(Debug, Deserialize, JsonSchema)]
struct Input {
    prompt: String,
}

#[derive(Debug, Serialize, JsonSchema)]
enum Output {
    Ok { message: String },
}

struct DummyTool;

impl NexusTool for DummyTool {
    type Input = Input;
    type Output = Output;

    fn description() -> &'static str {
        "Echoes the submitted prompt."
    }

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.dummy.tool@1")
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, Self::Input { prompt }: Self::Input) -> Self::Output {
        Output::Ok { message: prompt }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serves_plain_http_without_tls_credentials() {
    std::env::remove_var("NEXUS_TOOL_TLS_CERT_PATH");
    std::env::remove_var("NEXUS_TOOL_TLS_KEY_PATH");
    tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8048), DummyTool) });

    // Give the webserver some time to start.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let health = reqwest::get("http://localhost:8048/health").await.unwrap();

    assert_eq!(health.status(), 200);
}
