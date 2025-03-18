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
}

struct DummyTool;

impl NexusTool for DummyTool {
    type Input = Input;
    type Output = Output;

    fn fqn() -> ToolFqn {
        fqn!("xyz.dummy.tool@1")
    }

    async fn health() -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(Self::Input { prompt }: Self::Input) -> AnyResult<Self::Output> {
        Ok(Self::Output::Ok {
            message: format!("You said: {}", prompt),
        })
    }
}

struct Dummy500Tool;

impl NexusTool for Dummy500Tool {
    type Input = Input;
    type Output = Output;

    fn fqn() -> ToolFqn {
        fqn!("xyz.dummy.tool@1")
    }

    fn path() -> &'static str {
        "path"
    }

    async fn health() -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(_: Self::Input) -> AnyResult<Self::Output> {
        anyhow::bail!("Something went wrong")
    }
}

// == Integration tests ==

#[cfg(test)]
mod tests {
    use {super::*, reqwest::Client, serde_json::json};

    #[tokio::test]
    async fn test_endpoints_generated_correctly() {
        tokio::spawn(async move {
            bootstrap!(([127, 0, 0, 1], 8043), DummyTool);
        });

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

    #[tokio::test]
    async fn test_422_when_input_malformed() {
        tokio::spawn(async move {
            bootstrap!(([127, 0, 0, 1], 8044), DummyTool);
        });

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

    #[tokio::test]
    async fn test_500_when_execution_fails() {
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8045), [Dummy500Tool]) });

        let invoke = Client::new()
            .post("http://localhost:8045/path/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 500);

        let invoke_json = invoke.json::<serde_json::Value>().await.unwrap();

        assert_eq!(invoke_json["error"], "tool_invocation_error");
        assert_eq!(invoke_json["details"], "Something went wrong");

        // Default health ep exists.
        let health = Client::new()
            .get("http://localhost:8045/health")
            .send()
            .await
            .unwrap();

        assert_eq!(health.status(), 200);
    }

    #[tokio::test]
    async fn test_multiple_tools() {
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8046), [DummyTool, Dummy500Tool]) });

        // Invoke /path tool.
        let invoke = Client::new()
            .post("http://localhost:8046/path/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 500);

        let invoke_json = invoke.json::<serde_json::Value>().await.unwrap();

        assert_eq!(invoke_json["error"], "tool_invocation_error");
        assert_eq!(invoke_json["details"], "Something went wrong");

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
}
