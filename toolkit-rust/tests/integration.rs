use {
    anyhow::Result as AnyResult,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serial_test::serial,
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
    use {super::*, serde_json::json, tempfile::NamedTempFile};

    async fn setup_test_tls_key() -> (NamedTempFile, String) {
        // Install default crypto provider for rustls
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();

        let key_file = NamedTempFile::new().unwrap();
        let key_path = key_file.path().to_string_lossy().to_string();
        let spki_hash = generate_key_and_hash(&key_path).unwrap();
        (key_file, spki_hash)
    }

    #[tokio::test]
    #[serial]
    async fn test_endpoints_generated_correctly() {
        let (_key_file, _spki_hash) = setup_test_tls_key().await;
        let key_path = _key_file.path().to_string_lossy().to_string();

        // Set TLS_KEY environment variable
        std::env::set_var("TLS_KEY", &key_path);

        tokio::spawn(async move {
            bootstrap!(
                std::net::SocketAddr::from(([127, 0, 0, 1], 8043)),
                DummyTool
            )
        });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Create TLS client with proper pinning
        let client = reqwest_with_pin([_spki_hash.as_str()]).unwrap();

        let meta = client
            .get("https://localhost:8043/meta")
            .header("Host", "localhost:8043")
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

        let invoke_json = invoke.json::<Output>().await.unwrap();

        assert_eq!(
            invoke_json,
            Output::Ok {
                message: "You said: Hello, world!".to_string(),
            }
        );

        // Clean up environment variable
        std::env::remove_var("TLS_KEY");
    }

    #[tokio::test]
    #[serial]
    async fn test_422_when_input_malformed() {
        let (_key_file, _spki_hash) = setup_test_tls_key().await;
        let key_path = _key_file.path().to_string_lossy().to_string();

        // Set TLS_KEY environment variable
        std::env::set_var("TLS_KEY", &key_path);

        tokio::spawn(async move {
            bootstrap!(
                std::net::SocketAddr::from(([127, 0, 0, 1], 8044)),
                DummyTool
            )
        });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Create TLS client with proper pinning
        let client = reqwest_with_pin([_spki_hash.as_str()]).unwrap();

        let invoke = client
            .post("https://localhost:8044/invoke")
            .json(&json!({ "invalid": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 422);

        let invoke_json = invoke.json::<serde_json::Value>().await.unwrap();

        assert_eq!(invoke_json["error"], "input_deserialization_error");

        // Clean up environment variable
        std::env::remove_var("TLS_KEY");
    }

    #[tokio::test]
    #[serial]
    async fn test_500_when_execution_fails() {
        let (_key_file, _spki_hash) = setup_test_tls_key().await;
        let key_path = _key_file.path().to_string_lossy().to_string();

        // Set TLS_KEY environment variable
        std::env::set_var("TLS_KEY", &key_path);

        tokio::spawn(async move {
            bootstrap!(
                std::net::SocketAddr::from(([127, 0, 0, 1], 8045)),
                [DummyErrTool]
            )
        });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Create TLS client with proper pinning
        let client = reqwest_with_pin([_spki_hash.as_str()]).unwrap();

        let invoke = client
            .post("https://localhost:8045/path/invoke")
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
        let health = client
            .get("https://localhost:8045/health")
            .send()
            .await
            .unwrap();

        assert_eq!(health.status(), 200);

        // Clean up environment variable
        std::env::remove_var("TLS_KEY");
    }

    #[tokio::test]
    #[serial]
    async fn test_multiple_tools() {
        let (_key_file, _spki_hash) = setup_test_tls_key().await;
        let key_path = _key_file.path().to_string_lossy().to_string();

        // Set TLS_KEY environment variable
        std::env::set_var("TLS_KEY", &key_path);

        tokio::spawn(async move {
            bootstrap!(
                std::net::SocketAddr::from(([127, 0, 0, 1], 8046)),
                [DummyTool, DummyErrTool]
            )
        });

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Create TLS client with proper pinning
        let client = reqwest_with_pin([_spki_hash.as_str()]).unwrap();

        // Invoke /path tool.
        let invoke = client
            .post("https://localhost:8046/path/invoke")
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
        let invoke = client
            .post("https://localhost:8046/invoke")
            .json(&json!({ "invalid": "Hello, world!" }))
            .send()
            .await
            .unwrap();

        assert_eq!(invoke.status(), 422);

        let invoke_json = invoke.json::<serde_json::Value>().await.unwrap();

        assert_eq!(invoke_json["error"], "input_deserialization_error");

        // Clean up environment variable
        std::env::remove_var("TLS_KEY");
    }

    #[tokio::test]
    #[serial]
    async fn test_tls_server_and_client_bootstrap() {
        let (_key_file, spki_hash) = setup_test_tls_key().await;
        let key_path = _key_file.path().to_string_lossy().to_string();

        // Set the TLS_KEY environment variable so bootstrap! uses TLS
        std::env::set_var("TLS_KEY", &key_path);

        // Bootstrap TLS server with DummyTool
        let server_handle = tokio::spawn(async move {
            bootstrap!(
                std::net::SocketAddr::from(([127, 0, 0, 1], 8443)),
                DummyTool
            )
        });

        // Give the server time to start
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Create a client with SPKI pinning
        let client = reqwest_with_pin([spki_hash.as_str()]).unwrap();

        // Test health endpoint
        let response = client
            .get("https://localhost:8443/health")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);

        // Test invoke endpoint
        let test_data = serde_json::json!({
            "prompt": "Hello, TLS World!"
        });

        let response = client
            .post("https://localhost:8443/invoke")
            .json(&test_data)
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        let json_response: Output = response.json().await.unwrap();
        assert_eq!(
            json_response,
            Output::Ok {
                message: "You said: Hello, TLS World!".to_string(),
            }
        );

        // Clean up environment variable
        std::env::remove_var("TLS_KEY");
        server_handle.abort();
    }

    #[tokio::test]
    #[serial]
    async fn test_tls_client_with_wrong_pin_fails() {
        let (_key_file, _correct_hash) = setup_test_tls_key().await;
        let key_path = _key_file.path().to_string_lossy().to_string();

        // Set the TLS_KEY environment variable so bootstrap! uses TLS
        std::env::set_var("TLS_KEY", &key_path);

        // Bootstrap TLS server with DummyTool
        let server_handle = tokio::spawn(async move {
            bootstrap!(
                std::net::SocketAddr::from(([127, 0, 0, 1], 8444)),
                DummyTool
            )
        });

        // Give the server time to start
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Create a client with WRONG SPKI pin (all zeros)
        let wrong_pin = "0000000000000000000000000000000000000000000000000000000000000000";
        let client = reqwest_with_pin([wrong_pin]).unwrap();

        // This request should fail due to pin mismatch
        let result = client.get("https://localhost:8444/health").send().await;

        assert!(result.is_err(), "Request should fail with wrong pin");

        // Clean up environment variable
        std::env::remove_var("TLS_KEY");
        server_handle.abort();
    }
}
