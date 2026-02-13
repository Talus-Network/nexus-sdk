use {
    anyhow::Result as AnyResult,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    warp::http::StatusCode,
};

// == Dummy tools setup ==

#[derive(Debug, Deserialize, JsonSchema)]
struct Input {
    prompt: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
enum Output {
    Ok { message: String },
    Err { reason: String },
}

struct DummyTool;

impl NexusTool for DummyTool {
    type Input = Input;
    type Output = Output;

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
        Output::Ok {
            message: format!("You said: {}", prompt),
        }
    }
}

struct DummyErrTool;

impl NexusTool for DummyErrTool {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.dummy.tool@1")
    }

    fn path() -> &'static str {
        "path"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, _: Self::Input) -> Self::Output {
        Output::Err {
            reason: "Something went wrong".to_string(),
        }
    }
}

// == Integration tests ==

#[cfg(test)]
mod tests {
    use {super::*, reqwest::Client, serde_json::json};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_endpoints_generated_correctly() {
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8043), DummyTool) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let meta = Client::new()
            .get("http://localhost:8043/meta")
            .send()
            .await
            .unwrap();

        assert_eq!(meta.status(), 200);

        let meta_json = meta.json::<serde_json::Value>().await.unwrap();

        assert_eq!(meta_json["fqn"], "xyz.dummy.tool@1");
        assert_eq!(meta_json["url"], "http://localhost:8043/");
        assert_eq!(
            meta_json["input_schema"]["properties"]["prompt"]["type"],
            "string"
        );
        assert_eq!(
            meta_json["output_schema"]["oneOf"][0]["properties"]["Ok"]["properties"]["message"]
                ["type"],
            "string"
        );

        let health = Client::new()
            .get("http://localhost:8043/health")
            .send()
            .await
            .unwrap();

        assert_eq!(health.status(), 200);

        let invoke = Client::new()
            .post("http://localhost:8043/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 200);

        let invoke_json = invoke.json::<Output>().await.unwrap();

        assert_eq!(
            invoke_json,
            Output::Ok {
                message: "You said: Hello, world!".to_string(),
            }
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_422_when_input_malformed() {
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8044), DummyTool) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let invoke = Client::new()
            .post("http://localhost:8044/invoke")
            .json(&json!({ "invalid": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 422);

        let invoke_json = invoke.json::<serde_json::Value>().await.unwrap();

        assert_eq!(invoke_json["error"], "input_deserialization_error");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_500_when_execution_fails() {
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8045), [DummyErrTool]) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let invoke = Client::new()
            .post("http://localhost:8045/path/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 200);

        let invoke_json = invoke.json::<Output>().await.unwrap();

        assert_eq!(
            invoke_json,
            Output::Err {
                reason: "Something went wrong".to_string(),
            }
        );

        // Default health ep exists.
        let health = Client::new()
            .get("http://localhost:8045/health")
            .send()
            .await
            .unwrap();

        assert_eq!(health.status(), 200);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_multiple_tools() {
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8046), [DummyTool, DummyErrTool]) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Invoke /path tool.
        let invoke = Client::new()
            .post("http://localhost:8046/path/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 200);

        let invoke_json = invoke.json::<Output>().await.unwrap();

        assert_eq!(
            invoke_json,
            Output::Err {
                reason: "Something went wrong".to_string(),
            }
        );

        // Invoke / tool.
        let invoke = Client::new()
            .post("http://localhost:8046/invoke")
            .json(&json!({ "invalid": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 422);

        let invoke_json = invoke.json::<serde_json::Value>().await.unwrap();

        assert_eq!(invoke_json["error"], "input_deserialization_error");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_meta_invalid_schema() {
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8047), DummyTool) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let meta = Client::new()
            .get("http://localhost:8047/meta")
            .header("X-Forwarded-Proto", "ftp")
            .send()
            .await
            .unwrap();

        assert_eq!(meta.status(), 400);

        let meta_json = meta.json::<serde_json::Value>().await.unwrap();

        assert_eq!(meta_json["error"], "invalid_scheme");
        assert_eq!(
            meta_json["details"],
            "Scheme must be either 'http' or 'https'."
        );
    }
}
