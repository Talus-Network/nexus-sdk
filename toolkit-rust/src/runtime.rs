//! See <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

#[doc(hidden)]
pub mod tls;

use {
    crate::{
        config::Config,
        signed_http_warp::{handle_invoke, InvokeAuth},
        NexusTool,
        ToolkitRuntimeConfig,
    },
    futures::FutureExt,
    nexus_sdk::move_bindings::interface::meta_schema::MetaSchema,
    reqwest::Url,
    serde_json::json,
    std::{panic::AssertUnwindSafe, sync::Arc},
    warp::{
        filters::{host::Authority, path::FullPath},
        http::{HeaderMap, StatusCode},
        Filter,
        Rejection,
        Reply,
    },
};

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
/// - Returns exact ordered BCS Tool output bytes and signs the nonce-bound v3 Tool response message.
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
///   "invoke_max_body_bytes": 10485760,
///   "signed_http": {
///     "mode": "required",
///     "allowed_leaders_path": "./allowed_leaders.json",
///     "tools": {
///       "xyz.dummy.tool@1": {
///         "response_signing_key": "0000000000000000000000000000000000000000000000000000000000000000",
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
/// # Invocation errors and timeouts
///
/// The runtime catches unwinding panics during input decoding, tool construction,
/// authorization, invocation, and output serialization. It returns a generic
/// HTTP 500 response without the panic payload or a tool result signature.
///
/// Recovery requires `panic = "unwind"` (Cargo's default). The final application's
/// workspace controls this setting; with `panic = "abort"`, a panic terminates the process.
///
/// Invocation deadlines follow [`NexusTool::timeout()`].
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
            .map(|| {
                $crate::warp::reply::with_status($crate::warp::reply(), StatusCode::OK)
            });

        // Add a default tools route to list all tools available at that webserver.
        let default_tools_route = $crate::warp::get()
            .and($crate::warp::path("tools"))
            .map(move || $crate::warp::reply::json(&paths));

        let routes = routes
            .or(default_health_route)
            .or(default_tools_route)
            .boxed();
        // Terminate TLS directly when both credential paths are configured.
        match $crate::runtime::tls::Config::from_env()
            .expect("Invalid Nexus Tool TLS configuration")
        {
            $crate::runtime::tls::Config::Disabled => {
                $crate::warp::serve(routes).run($addr).await
            }
            $crate::runtime::tls::Config::Enabled { cert_path, key_path } => {
                let (_, incoming) =
                    $crate::runtime::tls::bind($addr.into(), cert_path, key_path)
                        .await
                        .expect("Failed to start Nexus Tool TLS server");
                $crate::runtime::tls::serve(routes, incoming).await
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

    Ok(warp::reply::with_status(warp::reply(), status))
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
        let invocation =
            AssertUnwindSafe(Self::run_inner::<T>(body_bytes, auth_ctx)).catch_unwind();
        match tokio::time::timeout(T::timeout(), invocation).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                log::error!("Tool invocation panicked");
                InvokePipelineResponse::json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": "tool_invocation_failed"}),
                )
            }
            Err(_) => InvokePipelineResponse::json(
                StatusCode::GATEWAY_TIMEOUT,
                json!({"error": "tool_invocation_timeout"}),
            ),
        }
    }

    async fn run_inner<T: NexusTool>(
        body_bytes: &[u8],
        auth_ctx: Option<crate::AuthContext>,
    ) -> InvokePipelineResponse {
        let input = match decode_tool_input::<T>(body_bytes, auth_ctx.as_ref()) {
            Ok(value) => value,
            Err(ToolInputDecodeError::Integrity) => {
                return InvokePipelineResponse::json(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    json!({
                        "error": "input_integrity_error",
                        "details": "resolved Tool input does not match authenticated input hash",
                    }),
                );
            }
            Err(ToolInputDecodeError::Deserialization(e)) => {
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

        match T::encode_output(output).and_then(|output| {
            let schema = MetaSchema::from_offchain_json_schemas(
                &serde_json::to_vec(&schemars::schema_for!(T::Input))?,
                &serde_json::to_vec(&schemars::schema_for!(T::Output))?,
            )?;
            schema.canonical_output_ports(&output)?;
            Ok(bcs::to_bytes(&output)?)
        }) {
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

enum ToolInputDecodeError {
    Deserialization(anyhow::Error),
    Integrity,
}

fn decode_tool_input<T: NexusTool>(
    body_bytes: &[u8],
    auth_ctx: Option<&crate::AuthContext>,
) -> Result<T::Input, ToolInputDecodeError> {
    if let Some(auth_ctx) = auth_ctx {
        let schema = MetaSchema::from_offchain_json_schemas(
            &serde_json::to_vec(&schemars::schema_for!(T::Input))
                .map_err(anyhow::Error::from)
                .map_err(ToolInputDecodeError::Deserialization)?,
            &serde_json::to_vec(&schemars::schema_for!(T::Output))
                .map_err(anyhow::Error::from)
                .map_err(ToolInputDecodeError::Deserialization)?,
        )
        .map_err(ToolInputDecodeError::Deserialization)?;
        let resolved_inputs = serde_json::from_slice::<serde_json::Value>(body_bytes)
            .map_err(anyhow::Error::from)
            .and_then(|value| schema.resolved_inputs_from_json(&value))
            .map_err(ToolInputDecodeError::Deserialization)?;
        if schema.resolved_inputs_sha256(&resolved_inputs).ok() != Some(auth_ctx.input_hash) {
            return Err(ToolInputDecodeError::Integrity);
        }
        return serde_json::from_value::<crate::WithSerdeErrorPath<T::Input>>(
            schema
                .resolved_inputs_to_semantic_json(&resolved_inputs)
                .map_err(ToolInputDecodeError::Deserialization)?,
        )
        .map(|value| value.0)
        .map_err(|error| ToolInputDecodeError::Deserialization(error.into()));
    }
    serde_json::from_slice::<crate::WithSerdeErrorPath<T::Input>>(body_bytes)
        .map(|value| value.0)
        .map_err(|error| ToolInputDecodeError::Deserialization(error.into()))
}

#[cfg(test)]
fn encode_tagged_output<T: serde::Serialize>(output: T) -> anyhow::Result<Vec<u8>> {
    Ok(bcs::to_bytes(
        &nexus_sdk::types::OffchainToolOutput::from_json(serde_json::to_value(
            crate::WithSerdeErrorPath(output),
        )?)?,
    )?)
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
        nexus_sdk::{
            fqn,
            signed_http::v3::wire::AuthenticatedRequest,
            types::{NexusData, OffchainToolOutput},
            ToolFqn,
        },
        schemars::JsonSchema,
        serde::{Deserialize, Serialize},
        serde_json::json,
    };

    fn resolved_input(port_name: &str, value: serde_json::Value) -> serde_json::Value {
        let value = NexusData::inline_data(serde_json::to_vec(&value).unwrap()).unwrap();
        json!({
            "ports": [{
                "port_name": port_name,
                "value": value.to_json_value().unwrap(),
            }],
        })
    }

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

    #[derive(Serialize)]
    enum DirectErrOutput {
        #[serde(rename = "_err_eval")]
        ErrEval { reason: String },
    }

    struct TestTool;

    impl NexusTool for TestTool {
        type Input = Input;
        type Output = Output;

        fn description() -> &'static str {
            "Returns a structured test response."
        }

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
    fn tool_output_encodes_as_ordered_offchain_output() {
        let bytes = encode_tagged_output(Output::Ok {
            message: "hello".to_string(),
            count: 2,
            flags: vec![true, false],
            metadata: json!({"source": "test"}),
        })
        .unwrap();
        let output: OffchainToolOutput = bcs::from_bytes(&bytes).unwrap();

        assert_eq!(bcs::to_bytes(&output).unwrap(), bytes);
        assert_eq!(output.tag, b"Ok");
        assert_eq!(
            output
                .ports
                .iter()
                .map(|port| port.port_name.as_slice())
                .collect::<Vec<_>>(),
            vec![
                b"message".as_slice(),
                b"count".as_slice(),
                b"flags".as_slice(),
                b"metadata".as_slice(),
            ]
        );

        let message = NexusData::from_values(output.ports[0].values.clone(), false).unwrap();
        assert_eq!(message.inline_data_bytes(), Some(br#""hello""#.to_vec()));

        let flags = NexusData::from_values(output.ports[2].values.clone(), true).unwrap();
        assert_eq!(
            flags
                .values()
                .unwrap()
                .into_iter()
                .map(|value| match value {
                    nexus_sdk::move_bindings::primitives::data::NexusValue::InlineData {
                        bytes,
                    } => bytes.clone(),
                    _ => panic!("flags should be inline Data"),
                })
                .collect::<Vec<_>>(),
            vec![b"true".to_vec(), b"false".to_vec()]
        );

        let metadata = NexusData::from_values(output.ports[3].values.clone(), false).unwrap();
        assert_eq!(
            metadata.inline_data_bytes(),
            Some(br#"{"source":"test"}"#.to_vec())
        );
    }

    #[test]
    fn direct_err_eval_encodes_exact_named_signed_output() {
        let bytes = encode_tagged_output(DirectErrOutput::ErrEval {
            reason: "forced failure".to_string(),
        })
        .unwrap();
        let output: OffchainToolOutput = bcs::from_bytes(&bytes).unwrap();

        assert_eq!(output.tag, b"_err_eval");
        assert_eq!(output.ports.len(), 1);
        assert_eq!(output.ports[0].port_name, b"reason");
        let reason = NexusData::from_values(output.ports[0].values.clone(), false).unwrap();
        assert_eq!(
            reason.inline_data_bytes(),
            Some(br#""forced failure""#.to_vec())
        );
    }

    #[test]
    fn canonical_response_encoding_rejects_non_enum_shapes() {
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
    fn array_payloads_encode_each_json_value() {
        let port_data = |value| -> anyhow::Result<NexusData> {
            let output = OffchainToolOutput::from_json(json!({"ok":{"items":value}}))?;
            NexusData::from_values(output.ports[0].values.clone(), true)
        };
        let inline_values = |data: &NexusData| {
            data.values()
                .expect("encoded Toolkit output should decode")
                .into_iter()
                .map(|element| match element {
                    nexus_sdk::move_bindings::primitives::data::NexusValue::InlineData {
                        bytes,
                    } => bytes.clone(),
                    _ => panic!("array payload should contain inline Data"),
                })
                .collect::<Vec<_>>()
        };
        let homogeneous = port_data(json!(["a", "b"])).unwrap();
        assert_eq!(
            inline_values(&homogeneous),
            vec![br#""a""#.to_vec(), br#""b""#.to_vec()]
        );

        let mixed = port_data(json!([1, true])).unwrap();
        assert_eq!(inline_values(&mixed), vec![b"1".to_vec(), b"true".to_vec()]);

        let empty = port_data(json!([])).expect_err("empty arrays must not produce Many values");
        assert!(empty.to_string().contains("requires at least one value"));
    }

    #[tokio::test]
    async fn invoke_pipeline_returns_exact_canonical_response_bytes() {
        let body = serde_json::to_vec(&json!({"message": "hello"})).unwrap();
        let response = InvokePipeline::run::<TestTool>(&body, None).await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(response.is_result);
        let output: OffchainToolOutput = bcs::from_bytes(&response.body).unwrap();
        assert_eq!(bcs::to_bytes(&output).unwrap(), response.body);
    }

    #[tokio::test]
    async fn invalid_input_remains_local_json_error() {
        let response = InvokePipeline::run::<TestTool>(b"{}", None).await;
        assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(!response.is_result);
        assert!(serde_json::from_slice::<serde_json::Value>(&response.body).is_ok());
    }

    #[derive(JsonSchema)]
    enum PanickingOutput {
        Ok { message: String },
    }

    impl Serialize for PanickingOutput {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            match self {
                Self::Ok { message } => panic!("PRIVATE_SERIALIZATION_MARKER {message}"),
            }
        }
    }

    struct FailingTool<const PHASE: u8>;

    impl<const PHASE: u8> NexusTool for FailingTool<PHASE> {
        type Input = Input;
        type Output = PanickingOutput;

        fn fqn() -> ToolFqn {
            fqn!("xyz.taluslabs.failure.test@1")
        }

        fn description() -> &'static str {
            "Exercise invocation failure isolation."
        }

        fn timeout() -> std::time::Duration {
            std::time::Duration::from_millis(10)
        }

        async fn new() -> Self {
            assert!(PHASE != 0, "PRIVATE_CONSTRUCTOR_MARKER");
            Self
        }

        async fn authorize(&self, _: crate::AuthContext) -> anyhow::Result<()> {
            assert!(PHASE != 1, "PRIVATE_AUTHORIZATION_MARKER");
            Ok(())
        }

        async fn invoke(&self, _: Input) -> PanickingOutput {
            if PHASE == 4 {
                std::future::pending::<()>().await;
            }
            assert!(PHASE != 2, "PRIVATE_INVOCATION_MARKER");
            PanickingOutput::Ok {
                message: "hello".into(),
            }
        }

        async fn health(&self) -> anyhow::Result<StatusCode> {
            Ok(StatusCode::OK)
        }
    }

    #[tokio::test]
    async fn panics_are_local_errors_and_later_requests_succeed() {
        let body = br#"{"message":"hello"}"#;
        let responses = [
            InvokePipeline::run::<FailingTool<0>>(body, None).await,
            InvokePipeline::run::<FailingTool<2>>(body, None).await,
            InvokePipeline::run::<FailingTool<3>>(body, None).await,
        ];
        for response in responses {
            assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
            assert!(!response.is_result);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&response.body).unwrap(),
                json!({"error": "tool_invocation_failed"})
            );
        }
        assert_eq!(
            InvokePipeline::run::<TestTool>(body, None).await.status,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn authorization_panics_do_not_escape_or_become_tool_results() {
        let transport = resolved_input("message", json!("hello"));
        let schema = MetaSchema::from_offchain_json_schemas(
            &serde_json::to_vec(&schemars::schema_for!(Input)).unwrap(),
            &serde_json::to_vec(&schemars::schema_for!(PanickingOutput)).unwrap(),
        )
        .unwrap();
        let resolved = schema.resolved_inputs_from_json(&transport).unwrap();
        let auth = AuthenticatedRequest {
            leader_id: "leader".into(),
            leader_key_id: 0,
            input_hash: schema.resolved_inputs_sha256(&resolved).unwrap(),
            leader_signature: [2; 64],
            nonce: [3; 32],
        };
        let response = InvokePipeline::run::<FailingTool<1>>(
            &serde_json::to_vec(&transport).unwrap(),
            Some(auth),
        )
        .await;
        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!response.is_result);
        assert!(!String::from_utf8(response.body)
            .unwrap()
            .contains("PRIVATE"));
    }

    #[tokio::test]
    async fn asynchronous_work_obeys_the_tool_deadline() {
        let response = InvokePipeline::run::<FailingTool<4>>(br#"{"message":"hello"}"#, None).await;
        assert_eq!(response.status, StatusCode::GATEWAY_TIMEOUT);
        assert!(!response.is_result);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&response.body).unwrap(),
            json!({"error": "tool_invocation_timeout"})
        );
    }

    #[tokio::test]
    async fn invoke_pipeline_rejects_authenticated_hash_mismatch() {
        let transport = resolved_input("message", json!("hello"));
        let body = serde_json::to_vec(&transport).unwrap();
        let schema = MetaSchema::from_offchain_json_schemas(
            &serde_json::to_vec(&schemars::schema_for!(Input)).unwrap(),
            &serde_json::to_vec(&schemars::schema_for!(Output)).unwrap(),
        )
        .unwrap();
        let resolved = schema.resolved_inputs_from_json(&transport).unwrap();
        let actual_hash = schema.resolved_inputs_sha256(&resolved).unwrap();
        let auth = |input_hash| AuthenticatedRequest {
            leader_id: "leader".to_string(),
            leader_key_id: 0,
            input_hash,
            leader_signature: [2; 64],
            nonce: [3; 32],
        };

        let mismatch = InvokePipeline::run::<TestTool>(&body, Some(auth([9; 32]))).await;
        assert_eq!(mismatch.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&mismatch.body).unwrap()["error"],
            "input_integrity_error"
        );

        let accepted = InvokePipeline::run::<TestTool>(&body, Some(auth(actual_hash))).await;
        assert_eq!(accepted.status, StatusCode::OK);

        let semantic_body = serde_json::to_vec(&json!({"message": "hello"})).unwrap();
        let injected = InvokePipeline::run::<TestTool>(
            &semantic_body,
            Some(auth(nexus_sdk::signed_http::v3::wire::sha256(
                &semantic_body,
            ))),
        )
        .await;
        assert_eq!(injected.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(!injected.is_result);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&injected.body).unwrap()["error"],
            "input_deserialization_error"
        );
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

    #[derive(Serialize, JsonSchema)]
    enum ExplicitOutput {
        Ok {
            #[schemars(with = "String")]
            message: NexusData,
        },
    }

    struct ExplicitTool<const INVALID: bool>;
    impl<const INVALID: bool> NexusTool for ExplicitTool<INVALID> {
        type Input = Input;
        type Output = ExplicitOutput;

        fn fqn() -> ToolFqn {
            fqn!("xyz.taluslabs.explicit@1")
        }

        fn description() -> &'static str {
            "Returns explicit protocol data."
        }

        async fn new() -> Self {
            Self
        }

        async fn health(&self) -> anyhow::Result<StatusCode> {
            Ok(StatusCode::OK)
        }

        async fn invoke(&self, _: Input) -> ExplicitOutput {
            ExplicitOutput::Ok {
                message: NexusData::walrus_data(
                    b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    [1; 32],
                )
                .unwrap(),
            }
        }

        fn encode_output(output: ExplicitOutput) -> anyhow::Result<OffchainToolOutput> {
            let ExplicitOutput::Ok { message } = output;
            OffchainToolOutput::from_ports(
                if INVALID {
                    b"Unknown".to_vec()
                } else {
                    b"Ok".to_vec()
                },
                [("message".into(), message)],
            )
        }
    }

    #[tokio::test]
    async fn explicit_references_remain_canonical_and_invalid_outputs_are_unsigned() {
        let input = br#"{"message":"input"}"#;
        let response = InvokePipeline::run::<ExplicitTool<false>>(input, None).await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(response.is_result);
        let output: OffchainToolOutput = bcs::from_bytes(&response.body).unwrap();
        assert!(matches!(
            output.ports[0].values[0],
            nexus_sdk::types::NexusValue::WalrusData { .. }
        ));
        let response = InvokePipeline::run::<ExplicitTool<true>>(input, None).await;
        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!response.is_result);
    }

    #[test]
    fn meta_placeholder_url_with_leading_slash() {
        let url = super::meta_placeholder_url_("/foo/");
        assert_eq!(url.host_str(), Some("localhost"));
        assert_eq!(url.path(), "/foo/");
    }
}
