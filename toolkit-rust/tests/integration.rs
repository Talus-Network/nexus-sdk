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
        Output::Ok {
            message: format!("You said: {}", prompt),
        }
    }
}

struct DummyErrTool;

impl NexusTool for DummyErrTool {
    type Input = Input;
    type Output = Output;

    fn description() -> &'static str {
        "Returns a deterministic Tool error."
    }

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
        nexus_sdk::{move_bindings::primitives::data::NexusValue, types::OffchainToolOutput},
        reqwest::Client,
        serde_json::json,
    };

    fn tls_client() -> Client {
        static TLS: std::sync::OnceLock<(tempfile::TempDir, String)> = std::sync::OnceLock::new();
        let (_, cert_pem) = TLS.get_or_init(|| {
            let rcgen::CertifiedKey { cert, key_pair } =
                rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
            let cert_pem = cert.pem();
            let directory = tempfile::tempdir().unwrap();
            let cert_path = directory.path().join("cert.pem");
            let key_path = directory.path().join("key.pem");
            std::fs::write(&cert_path, &cert_pem).unwrap();
            std::fs::write(&key_path, key_pair.serialize_pem()).unwrap();
            // Every bootstrap test initializes these paths before starting its server.
            std::env::set_var("NEXUS_TOOL_TLS_CERT_PATH", cert_path);
            std::env::set_var("NEXUS_TOOL_TLS_KEY_PATH", key_path);
            (directory, cert_pem)
        });
        Client::builder()
            .add_root_certificate(reqwest::Certificate::from_pem(cert_pem.as_bytes()).unwrap())
            .build()
            .unwrap()
    }

    async fn assert_canonical_output(
        response: reqwest::Response,
        expected_tag: &[u8],
        expected_port: &[u8],
        expected_value: &[u8],
    ) {
        let body = response.bytes().await.unwrap();
        let output: OffchainToolOutput = bcs::from_bytes(&body).unwrap();
        assert_eq!(bcs::to_bytes(&output).unwrap(), body);
        assert_eq!(output.tag, expected_tag);
        let [port] = output.ports.as_slice() else {
            panic!("response should contain one output group")
        };
        let [NexusValue::InlineData { bytes }] = port.values.as_slice() else {
            panic!("response group should contain one inline value")
        };
        assert_eq!(port.port_name, expected_port);
        assert_eq!(bytes, expected_value);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_endpoints_generated_correctly() {
        let client = tls_client();
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8043), DummyTool) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let meta = client
            .get("https://localhost:8043/meta")
            .send()
            .await
            .unwrap();

        assert_eq!(meta.status(), 200);

        let meta_json = meta.json::<serde_json::Value>().await.unwrap();

        assert_eq!(meta_json["fqn"], "xyz.dummy.tool@1");
        assert_eq!(meta_json["url"], "https://localhost:8043/");
        assert_eq!(
            meta_json["input_schema"]["properties"]["prompt"]["type"],
            "string"
        );
        assert_eq!(
            meta_json["output_schema"]["oneOf"][0]["properties"]["Ok"]["properties"]["message"]
                ["type"],
            "string"
        );

        let health = client
            .get("https://localhost:8043/health")
            .send()
            .await
            .unwrap();

        assert_eq!(health.status(), 200);

        let invoke = client
            .post("https://localhost:8043/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 200);

        assert_canonical_output(invoke, b"Ok", b"message", br#""You said: Hello, world!""#).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_422_when_input_malformed() {
        let client = tls_client();
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8044), DummyTool) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let invoke = client
            .post("https://localhost:8044/invoke")
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
        let client = tls_client();
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8045), [DummyErrTool]) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let invoke = client
            .post("https://localhost:8045/path/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 200);

        assert_canonical_output(invoke, b"Err", b"reason", br#""Something went wrong""#).await;

        // Default health ep exists.
        let health = client
            .get("https://localhost:8045/health")
            .send()
            .await
            .unwrap();

        assert_eq!(health.status(), 200);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_multiple_tools() {
        let client = tls_client();
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8046), [DummyTool, DummyErrTool]) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Invoke /path tool.
        let invoke = client
            .post("https://localhost:8046/path/invoke")
            .json(&json!({ "prompt": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 200);

        assert_canonical_output(invoke, b"Err", b"reason", br#""Something went wrong""#).await;

        // Invoke / tool.
        let invoke = client
            .post("https://localhost:8046/invoke")
            .json(&json!({ "invalid": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 422);

        let invoke_json = invoke.json::<serde_json::Value>().await.unwrap();

        assert_eq!(invoke_json["error"], "input_deserialization_error");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_meta_ignores_forwarded_proto() {
        let client = tls_client();
        tokio::spawn(async move { bootstrap!(([127, 0, 0, 1], 8047), DummyTool) });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let meta = client
            .get("https://localhost:8047/meta")
            .header("X-Forwarded-Proto", "ftp")
            .send()
            .await
            .unwrap();

        assert_eq!(meta.status(), 200);

        let meta_json = meta.json::<serde_json::Value>().await.unwrap();

        assert_eq!(meta_json["url"], "https://localhost:8047/");
    }
}
