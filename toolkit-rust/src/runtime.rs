//! See <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

use {
    crate::{
        config::Config,
        signed_http_warp::{handle_invoke, InvokeAuth},
        NexusTool,
        ToolkitRuntimeConfig,
    },
    nexus_sdk::move_bindings::{
        primitives::{
            data::{DataTypeHint, NexusData, TypedNexusData},
            tagged_output::TaggedOutput,
        },
        sui_framework::vec_map::{Entry as VecMapEntry, VecMap},
    },
    reqwest::Url,
    serde_json::json,
    std::sync::Arc,
    warp::{
        filters::{host::Authority, path::FullPath},
        http::{HeaderMap, StatusCode},
        Filter,
        Rejection,
        Reply,
    },
};

/// Direct TLS configuration for the Toolkit server.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolTlsConfig {
    Disabled,
    Enabled { cert_path: String, key_path: String },
}

/// Validate the all-or-none TLS certificate/key pair used by [`bootstrap!`].
#[doc(hidden)]
pub fn tool_tls_config_(
    cert_path: Option<String>,
    key_path: Option<String>,
) -> anyhow::Result<ToolTlsConfig> {
    match (cert_path, key_path) {
        (None, None) => Ok(ToolTlsConfig::Disabled),
        (Some(cert_path), Some(key_path)) if !cert_path.is_empty() && !key_path.is_empty() => {
            Ok(ToolTlsConfig::Enabled {
                cert_path,
                key_path,
            })
        }
        _ => anyhow::bail!(
            "NEXUS_TOOL_TLS_CERT_PATH and NEXUS_TOOL_TLS_KEY_PATH must both be set to non-empty paths"
        ),
    }
}

/// Load toolkit configuration from environment.
///
/// **This is an internal function used by [bootstrap!] macro and should not be
/// used directly.**
#[doc(hidden)]
pub fn load_config_() -> anyhow::Result<Arc<ToolkitRuntimeConfig>> {
    ToolkitRuntimeConfig::from_env().map(Arc::new)
}

/// Build a placeholder URL for `--meta` output from a tool's [`NexusTool::path()`].
///
/// The URL is a `http://localhost`-based placeholder — the real URL is set
/// during registration via `--url`. This function normalises the path so that
/// a missing leading `/` does not corrupt the URL's authority component.
///
/// **This is an internal function used by [bootstrap!] macro and should not be
/// used directly.**
#[doc(hidden)]
pub fn meta_placeholder_url_(path: &str) -> Url {
    let base = if path.is_empty() {
        "http://localhost/".to_string()
    } else if path.starts_with('/') {
        format!("http://localhost{path}")
    } else {
        format!("http://localhost/{path}")
    };
    Url::parse(&base).expect("placeholder URL must be valid")
}

fn json_bytes_or_fallback(status: StatusCode, value: serde_json::Value) -> (StatusCode, Vec<u8>) {
    match serde_json::to_vec(&value) {
        Ok(body) => (status, body),
        Err(e) => {
            let fallback = json!({
                "error": "serialization_error",
                "details": e.to_string(),
            });
            let body = serde_json::to_vec(&fallback)
                .unwrap_or_else(|_| br#"{"error":"serialization_error"}"#.to_vec());
            (StatusCode::INTERNAL_SERVER_ERROR, body)
        }
    }
}

/// Macro to bootstrap the runtime for a set of tools. The macro generates the
/// necessary routes for each tool and serves them on the provided address.
///
/// # Signed HTTP (Leader <-> Tool authentication)
/// The runtime can optionally require signed `/invoke` requests and sign the responses.
///
/// Configuration is file-based and loaded from [`crate::ENV_TOOLKIT_CONFIG_PATH`]. The file schema
/// is documented on [`crate::ToolkitRuntimeConfig`].
///
/// When enabled (`signed_http.mode = "required"`), the runtime:
/// - Rejects unsigned or invalidly signed `/invoke` requests (fail-closed).
/// - Verifies the Leader signature against a local allowlist (`allowed_leaders` / `allowed_leaders_path`).
/// - Caches completed responses by deterministic nonce and canonical input hash; in-flight nonce reuse is rejected.
/// - Returns exact BCS `TaggedOutput` bytes and signs the nonce-bound v2 Tool response message.
/// - Keeps nonce replay and cached-response handling entirely offchain.
///
/// Operational note: your gateway/proxy must forward the `X-Nexus-Sig-*` headers in both directions.
/// The request body, nonce, and unsigned identity headers require authenticated HTTPS transport.
/// Set both `NEXUS_TOOL_TLS_CERT_PATH` and `NEXUS_TOOL_TLS_KEY_PATH` to terminate TLS directly in
/// the Toolkit server. If TLS terminates at a gateway, the gateway-to-Tool hop must provide an
/// equivalently protected transport; signed HTTP does not make a plaintext hop safe.
///
/// ## Example config
/// ```json
/// {
///   "version": 2,
///   "invoke_max_body_bytes": 10485760,
///   "signed_http": {
///     "mode": "required",
///     "allowed_leaders_path": "./allowed_leaders.json",
///     "tools": {
///       "xyz.dummy.tool@1": {
///         "tool_signing_key": "0000000000000000000000000000000000000000000000000000000000000000",
///         "replay_cache_ttl_ms": 300000
///       }
///     }
///   }
/// }
/// ```
///
/// ## `--meta` flag
/// When the binary is invoked with `--meta`, the macro prints a JSON array of
/// tool metadata (one entry per tool) to stdout and exits immediately — no HTTP
/// server is started. This is used by CI pipelines to extract registration data
/// from a Docker image without running the tool.
///
/// ```shell
/// ./my-tool --meta   # prints JSON array and exits
/// ```
///
/// ## Request body limits
/// `/invoke` enforces a `Content-Length` limit via `warp::body::content_length_limit`.
/// Requests without a `Content-Length` header are rejected.
///
/// # Examples
///
/// ### One tool running on `127.0.0.1:8080`
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!(YourTool);
/// }
/// ```
///
/// ### Multiple tools running on `127.0.0.1:8080`
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!([YourTool, AnotherTool]);
/// }
/// ```
///
/// ### One tool running on the provided address
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!(([127, 0, 0, 1], 8081), YourTool);
/// }
/// ```
///
/// ### Multiple tools running on the provided address
///
/// ```ignore
/// use nexus_toolkit::bootstrap;
///
/// #[tokio::main]
/// async fn main() {
///     bootstrap!(([127, 0, 0, 1], 8081), [YourTool, AnotherTool]);
/// }
/// ```
#[macro_export]
macro_rules! bootstrap {
    (@get_addr) => {{
        let addr_str = std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        addr_str
            .parse::<std::net::SocketAddr>()
            .expect("Invalid socket address in BIND_ADDR")
    }};
    ($addr:expr, [$tool:ty $(, $next_tool:ty)* $(,)?]) => {{
        let _ = $crate::env_logger::try_init();

        // Handle --meta: print tool metadata as a JSON array and exit
        // without starting the HTTP server. Used by CI to extract
        // registration data from a Docker image. Uses `return` to exit
        // the enclosing function (typically `main`).
        if ::std::env::args().any(|a| a == "--meta") {
            let meta = $crate::serde_json::json!([
                <$tool as $crate::NexusTool>::meta(
                    $crate::runtime::meta_placeholder_url_(<$tool as $crate::NexusTool>::path())
                )
                $(
                    , <$next_tool as $crate::NexusTool>::meta(
                        $crate::runtime::meta_placeholder_url_(<$next_tool as $crate::NexusTool>::path())
                    )
                )*
            ]);

            println!("{}", $crate::serde_json::to_string_pretty(&meta)
                .expect("meta serialization must not fail"));
            return;
        }

        use {
            ::std::sync::Arc,
            $crate::warp::{http::StatusCode, Filter},
        };

        // Load toolkit config (shared across all tool routes).
        let toolkit_cfg = $crate::runtime::load_config_()
            .expect("Failed to load Nexus toolkit config");

        // Create routes for each Tool in the bundle.
        let routes = $crate::runtime::routes_for_with_config_::<$tool>(toolkit_cfg.clone());
        $(
            let routes = routes.or(
                $crate::runtime::routes_for_with_config_::<$next_tool>(toolkit_cfg.clone())
            );
        )*

        // Collect paths of all tools.
        let mut paths = vec![<$tool as $crate::NexusTool>::path()];
        $(
            paths.push(<$next_tool as $crate::NexusTool>::path());
        )*

        // Add a default health route in case there is none in the root.
        let default_health_route = $crate::warp::get()
            .and($crate::warp::path("health"))
            .map(|| $crate::warp::reply::with_status("", StatusCode::OK));

        // Add a default tools route to list all tools available at that webserver.
        let default_tools_route = $crate::warp::get()
            .and($crate::warp::path("tools"))
            .map(move || $crate::warp::reply::json(&paths));

        let routes = routes.or(default_health_route).or(default_tools_route);
        // Serve the routes, terminating TLS directly when a certificate/key pair is configured.
        let tls_config = $crate::runtime::tool_tls_config_(
            ::std::env::var("NEXUS_TOOL_TLS_CERT_PATH").ok(),
            ::std::env::var("NEXUS_TOOL_TLS_KEY_PATH").ok(),
        )
        .expect("Invalid Nexus Tool TLS configuration");
        match tls_config {
            $crate::runtime::ToolTlsConfig::Disabled => {
                $crate::warp::serve(routes).run($addr).await
            }
            $crate::runtime::ToolTlsConfig::Enabled { cert_path, key_path } => {
                $crate::warp::serve(routes)
                    .tls()
                    .cert_path(cert_path)
                    .key_path(key_path)
                    .run($addr)
                    .await
            }
        }
    }};
    // Default address.
    ([$($tool:ty),+ $(,)?]) => {{
        let addr = bootstrap!(@get_addr);
        bootstrap!(addr, [$($tool,)*])
    }};
    // Only 1 tool.
    ($addr:expr, $tool:ty) => {{
        bootstrap!($addr, [$tool])
    }};
    // Only 1 tool with default address.
    ($tool:ty) => {{
        let addr = bootstrap!(@get_addr);
        bootstrap!(addr, [$tool])
    }};
}

#[cfg(test)]
mod tls_tests {
    use super::*;

    #[test]
    fn toolkit_tls_paths_are_all_or_none() {
        assert_eq!(
            tool_tls_config_(None, None).unwrap(),
            ToolTlsConfig::Disabled
        );
        assert_eq!(
            tool_tls_config_(Some("cert.pem".into()), Some("key.pem".into())).unwrap(),
            ToolTlsConfig::Enabled {
                cert_path: "cert.pem".into(),
                key_path: "key.pem".into(),
            }
        );
        for (cert, key) in [
            (Some("cert.pem".into()), None),
            (None, Some("key.pem".into())),
            (Some(String::new()), Some("key.pem".into())),
            (Some("cert.pem".into()), Some(String::new())),
        ] {
            assert!(tool_tls_config_(cert, key).is_err());
        }
    }
}

/// This function generates the necessary routes for a given [NexusTool] using an already-loaded
/// [`ToolkitRuntimeConfig`].
///
/// This exists so callers (like [`bootstrap!`]) can load and validate config once per process and
/// share it across multiple tool route bundles.
#[doc(hidden)]
pub fn routes_for_with_config_<T: NexusTool>(
    toolkit_cfg: Arc<ToolkitRuntimeConfig>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    // Wrap config with file watching support
    let config = Config::from_config(toolkit_cfg.clone());
    // Force output schema to be an enum.
    let output_schema = json!(schemars::schema_for!(T::Output));

    if output_schema["oneOf"].is_null() {
        panic!("The output type must be an enum to generate the correct output schema.");
    }

    let base_path = T::path()
        .split("/")
        .filter(|s| !s.is_empty())
        .fold(warp::any().boxed(), |filter, segment| {
            filter.and(warp::path(segment.to_string())).boxed()
        });

    let health_route = warp::get()
        .and(base_path.clone())
        .and(warp::path("health"))
        .and_then(health_handler::<T>);

    // Meta path is tool base URL path and `/meta`.
    let meta_route = warp::get()
        .and(base_path.clone())
        .and(warp::path("meta"))
        .and(warp::header::optional::<Authority>("X-Forwarded-Host"))
        .and(warp::header::optional::<String>("X-Forwarded-Proto"))
        .and(warp::filters::host::optional())
        .and(warp::path::full())
        .and_then(meta_handler::<T>);

    let invoke_max_body_bytes = toolkit_cfg.invoke_max_body_bytes();

    let tool_id = T::fqn().to_string();
    let invoke_auth = InvokeAuth::new_sync(config, tool_id, T::timeout())
        .expect("Failed to load signed HTTP configuration");

    // Invoke path is tool base URL path and `/invoke`.
    let invoke_route = warp::post()
        .and(base_path)
        .and(warp::path("invoke"))
        .and(warp::header::headers_cloned())
        .and(warp::body::content_length_limit(invoke_max_body_bytes))
        .and(warp::body::bytes())
        .and(warp::any().map(move || invoke_auth.clone()))
        .and_then(invoke_handler::<T>);

    health_route.or(meta_route).or(invoke_route)
}

async fn health_handler<T: NexusTool>() -> Result<impl Reply, Rejection> {
    let tool = T::new().await;

    let status = tool
        .health()
        .await
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    Ok(warp::reply::with_status("", status))
}

async fn meta_handler<T: NexusTool>(
    x_forwarded_host: Option<Authority>,
    x_forwarded_proto: Option<String>,
    host: Option<Authority>,
    path: FullPath,
) -> Result<impl Reply, Rejection> {
    // We always need the most "external" host, as this is what will be called by users.
    let host = x_forwarded_host.or(host);

    // If the host is malformed or not present, return a 400.
    let host = match host {
        Some(host) => host,
        None => {
            let reply = json!({
                "error": "host_header_required",
                "details": "Host header is required.",
            });

            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    // Stripping 'meta' suffix from the path will give us the base path.
    let base_path = match path.as_str().strip_suffix("meta") {
        Some(base_path) => base_path,
        None => {
            // This is probably never reached as we create the endpoints
            // ourselves.
            let reply = json!({
                "error": "invalid_path",
                "details": "Meta path must end with '/meta'.",
            });

            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    // As in the case of the host, we need to use the most "external" scheme,
    // which is basically the scheme used by the client to access the tool.
    // If the scheme is not present, we check the environment variable, which
    // might have been set for operational purposes.
    // As a last resort, we use http as the default scheme.
    //
    // Ref: https://github.com/Talus-Network/nexus-sdk/issues/77
    let scheme = x_forwarded_proto.unwrap_or_else(|| "http".to_string());

    // Validate scheme to prevent URL injection attacks
    if scheme != "http" && scheme != "https" {
        let reply = json!({
            "error": "invalid_scheme",
            "details": "Scheme must be either 'http' or 'https'.",
        });

        return Ok(warp::reply::with_status(
            warp::reply::json(&reply),
            StatusCode::BAD_REQUEST,
        ));
    }

    let url = match Url::parse(&format!("{scheme}://{host}{base_path}")) {
        Ok(url) => url,
        Err(e) => {
            let reply = json!({
                "error": "url_parsing_error",
                "details": e.to_string(),
            });

            return Ok(warp::reply::with_status(
                warp::reply::json(&reply),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&T::meta(url)),
        StatusCode::OK,
    ))
}

/// Result of the tool invocation pipeline before being turned into an HTTP response.
struct InvokePipelineResponse {
    status: StatusCode,
    body: Vec<u8>,
    is_result: bool,
}

impl InvokePipelineResponse {
    fn json(status: StatusCode, value: serde_json::Value) -> Self {
        let (status, body) = json_bytes_or_fallback(status, value);
        Self {
            status,
            body,
            is_result: false,
        }
    }
}

/// Tool invocation pipeline returning canonical BCS result bytes or a local JSON error.
struct InvokePipeline;

impl InvokePipeline {
    async fn run<T: NexusTool>(
        body_bytes: &[u8],
        auth_ctx: Option<crate::AuthContext>,
    ) -> InvokePipelineResponse {
        let input = match serde_json::from_slice::<crate::WithSerdeErrorPath<T::Input>>(body_bytes)
        {
            Ok(v) => v.0,
            Err(e) => {
                return InvokePipelineResponse::json(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    json!({
                        "error": "input_deserialization_error",
                        "details": e.to_string(),
                    }),
                );
            }
        };

        let tool = T::new().await;

        if let Some(ctx) = auth_ctx {
            if let Err(e) = tool.authorize(ctx).await {
                return InvokePipelineResponse::json(
                    StatusCode::FORBIDDEN,
                    json!({
                        "error": "permission_denied",
                        "details": e.to_string(),
                    }),
                );
            }
        }

        let output = tool.invoke(input).await;

        match encode_tagged_output(output) {
            Ok(body) => InvokePipelineResponse {
                status: StatusCode::OK,
                body,
                is_result: true,
            },
            Err(e) => InvokePipelineResponse::json(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error": "output_serialization_error",
                    "details": e.to_string(),
                }),
            ),
        }
    }
}

fn encode_tagged_output<T: serde::Serialize>(output: T) -> anyhow::Result<Vec<u8>> {
    let value = serde_json::to_value(crate::WithSerdeErrorPath(output))?;
    let serde_json::Value::Object(variants) = value else {
        anyhow::bail!("tool output must serialize as an externally tagged enum")
    };
    if variants.len() != 1 {
        anyhow::bail!("tool output must contain exactly one variant")
    }
    let (tag, payload) = variants.into_iter().next().expect("length checked");
    let serde_json::Value::Object(payload) = payload else {
        anyhow::bail!("tool output variant payload must be an object")
    };
    let mut named_payload = payload
        .into_iter()
        .map(|(name, value)| {
            Ok(VecMapEntry {
                key: name.into_bytes(),
                value: typed_nexus_data(value)?,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    named_payload.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(bcs::to_bytes(&TaggedOutput {
        tag: tag.into_bytes(),
        named_payload: VecMap {
            contents: named_payload,
        },
    })?)
}

fn typed_nexus_data(value: serde_json::Value) -> anyhow::Result<TypedNexusData> {
    if let serde_json::Value::Array(values) = value {
        let encoded = values
            .iter()
            .map(encoded_value)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let type_hint = encoded
            .first()
            .map(|(hint, _)| *hint)
            .filter(|hint| encoded.iter().all(|(candidate, _)| candidate == hint))
            .unwrap_or(DataTypeHint::Raw);
        let many = if type_hint == DataTypeHint::Raw {
            values
                .iter()
                .map(serde_json::to_vec)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            encoded.into_iter().map(|(_, bytes)| bytes).collect()
        };
        return Ok(TypedNexusData {
            type_hint,
            data: NexusData::inline_many(many),
        });
    }

    let (type_hint, bytes) = encoded_value(&value)?;
    Ok(TypedNexusData {
        type_hint,
        data: NexusData::inline_one(bytes),
    })
}

fn encoded_value(value: &serde_json::Value) -> anyhow::Result<(DataTypeHint, Vec<u8>)> {
    let encoded = match value {
        serde_json::Value::String(value) => (DataTypeHint::String, value.as_bytes().to_vec()),
        serde_json::Value::Number(_) => (DataTypeHint::Number, serde_json::to_vec(value)?),
        serde_json::Value::Bool(_) => (DataTypeHint::Bool, serde_json::to_vec(value)?),
        _ => (DataTypeHint::Raw, serde_json::to_vec(value)?),
    };
    Ok(encoded)
}

async fn invoke_handler<T: NexusTool>(
    headers: HeaderMap,
    body: bytes::Bytes,
    auth: InvokeAuth,
) -> Result<warp::reply::Response, Rejection> {
    let body_bytes = body.to_vec();

    let auth_runtime = auth.current().await;
    Ok(handle_invoke(
        &auth_runtime,
        auth.replay(),
        headers,
        body_bytes,
        |auth_ctx, body_bytes| async move {
            let pipeline = InvokePipeline::run::<T>(&body_bytes, auth_ctx).await;
            (pipeline.status, pipeline.body, pipeline.is_result)
        },
    )
    .await)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{fqn, ToolFqn},
        schemars::JsonSchema,
        serde::{Deserialize, Serialize},
        serde_json::json,
    };

    #[derive(Deserialize, JsonSchema)]
    struct Input {
        message: String,
    }

    #[derive(Serialize, JsonSchema)]
    enum Output {
        Ok {
            message: String,
            count: u64,
            flags: Vec<bool>,
            metadata: serde_json::Value,
        },
    }

    struct TestTool;

    impl NexusTool for TestTool {
        type Input = Input;
        type Output = Output;

        fn fqn() -> ToolFqn {
            fqn!("xyz.taluslabs.test@1")
        }

        async fn new() -> Self {
            Self
        }

        async fn invoke(&self, input: Self::Input) -> Self::Output {
            Output::Ok {
                message: input.message,
                count: 2,
                flags: vec![true, false],
                metadata: json!({"source": "test"}),
            }
        }

        async fn health(&self) -> anyhow::Result<StatusCode> {
            Ok(StatusCode::OK)
        }
    }

    #[test]
    fn tool_output_encodes_as_canonical_generated_tagged_output() {
        let bytes = encode_tagged_output(Output::Ok {
            message: "hello".to_string(),
            count: 2,
            flags: vec![true, false],
            metadata: json!({"source": "test"}),
        })
        .unwrap();
        let output: TaggedOutput = bcs::from_bytes(&bytes).unwrap();

        assert_eq!(bcs::to_bytes(&output).unwrap(), bytes);
        assert_eq!(output.tag, b"Ok");
        assert_eq!(output.named_payload.contents.len(), 4);
        assert_eq!(
            output
                .named_payload
                .contents
                .iter()
                .map(|entry| entry.key.as_slice())
                .collect::<Vec<_>>(),
            vec![
                b"count".as_slice(),
                b"flags".as_slice(),
                b"message".as_slice(),
                b"metadata".as_slice(),
            ]
        );

        let payload = |name: &[u8]| {
            &output
                .named_payload
                .contents
                .iter()
                .find(|entry| entry.key == name)
                .expect("named payload entry")
                .value
        };
        let message = payload(b"message");
        assert_eq!(message.type_hint, DataTypeHint::String);
        assert_eq!(message.data.inline_one_bytes(), Some(b"hello".as_slice()));

        let flags = payload(b"flags");
        assert_eq!(flags.type_hint, DataTypeHint::Bool);
        assert_eq!(flags.data.many, vec![b"true".to_vec(), b"false".to_vec()]);

        let metadata = payload(b"metadata");
        assert_eq!(metadata.type_hint, DataTypeHint::Raw);
        assert_eq!(metadata.data.one, br#"{"source":"test"}"#);
    }

    #[test]
    fn tagged_output_encoding_rejects_non_enum_shapes() {
        assert!(encode_tagged_output(json!("plain value"))
            .unwrap_err()
            .to_string()
            .contains("externally tagged enum"));
        assert!(encode_tagged_output(json!({"Ok": {}, "Err": {}}))
            .unwrap_err()
            .to_string()
            .contains("exactly one variant"));
        assert!(encode_tagged_output(json!({"Ok": 1}))
            .unwrap_err()
            .to_string()
            .contains("payload must be an object"));
    }

    #[test]
    fn array_payloads_use_typed_encoding_only_when_all_elements_match() {
        let homogeneous = typed_nexus_data(json!(["a", "b"])).unwrap();
        assert_eq!(homogeneous.type_hint, DataTypeHint::String);
        assert_eq!(homogeneous.data.many, vec![b"a".to_vec(), b"b".to_vec()]);

        let mixed = typed_nexus_data(json!([1, true])).unwrap();
        assert_eq!(mixed.type_hint, DataTypeHint::Raw);
        assert_eq!(mixed.data.many, vec![b"1".to_vec(), b"true".to_vec()]);

        let empty = typed_nexus_data(json!([])).unwrap();
        assert_eq!(empty.type_hint, DataTypeHint::Raw);
        assert!(empty.data.many.is_empty());
    }

    #[tokio::test]
    async fn invoke_pipeline_returns_exact_tagged_output_bytes() {
        let response = InvokePipeline::run::<TestTool>(br#"{"message":"hello"}"#, None).await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(response.is_result);
        let output: TaggedOutput = bcs::from_bytes(&response.body).unwrap();
        assert_eq!(bcs::to_bytes(&output).unwrap(), response.body);
    }

    #[tokio::test]
    async fn invalid_input_remains_local_json_error() {
        let response = InvokePipeline::run::<TestTool>(b"{}", None).await;
        assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(!response.is_result);
        assert!(serde_json::from_slice::<serde_json::Value>(&response.body).is_ok());
    }

    #[test]
    fn meta_placeholder_url_empty_path() {
        let url = super::meta_placeholder_url_("");
        assert_eq!(url.as_str(), "http://localhost/");
    }

    #[test]
    fn meta_placeholder_url_no_leading_slash() {
        let url = super::meta_placeholder_url_("path");
        assert_eq!(url.host_str(), Some("localhost"));
        assert_eq!(url.path(), "/path");
    }

    #[test]
    fn meta_placeholder_url_with_leading_slash() {
        let url = super::meta_placeholder_url_("/foo/");
        assert_eq!(url.host_str(), Some("localhost"));
        assert_eq!(url.path(), "/foo/");
    }
}
