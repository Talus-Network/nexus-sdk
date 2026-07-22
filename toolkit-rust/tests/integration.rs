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

#[derive(Debug, Serialize, JsonSchema)]
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
    use {
        super::*,
        nexus_sdk::move_bindings::primitives::{data::DataTypeHint, tagged_output::TaggedOutput},
        reqwest::Client,
        serde_json::json,
    };

    async fn assert_tagged_output(
        response: reqwest::Response,
        expected_tag: &[u8],
        expected_field: &[u8],
        expected_value: &[u8],
    ) {
        let body = response.bytes().await.unwrap();
        let output: TaggedOutput = bcs::from_bytes(&body).unwrap();
        assert_eq!(bcs::to_bytes(&output).unwrap(), body);
        assert_eq!(output.tag, expected_tag);
        assert_eq!(output.named_payload.contents.len(), 1);
        let field = &output.named_payload.contents[0];
        assert_eq!(field.key, expected_field);
        assert_eq!(field.value.type_hint, DataTypeHint::String);
        assert_eq!(field.value.data.inline_one_bytes(), Some(expected_value));
    }

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

        assert_tagged_output(invoke, b"Ok", b"message", b"You said: Hello, world!").await;
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
    async fn test_err_variant_returns_tagged_output() {
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

        assert_tagged_output(invoke, b"Err", b"reason", b"Something went wrong").await;

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

        assert_tagged_output(invoke, b"Err", b"reason", b"Something went wrong").await;

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
