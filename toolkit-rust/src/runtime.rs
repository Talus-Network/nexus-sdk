//! See <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

use {
    crate::{
        config::{ConfigWatcher, ToolkitRuntimeConfig},
        signed_http_warp::{handle_invoke, InvokeAuth, InvokeAuthRuntime},
        NexusTool,
    },
    nexus_sdk::signed_http::v1::wire::HttpRequestMeta,
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

/// Load toolkit configuration from environment.
///
/// **This is an internal function used by [bootstrap!] macro and should not be
/// used directly.**
#[doc(hidden)]
pub async fn load_config_() -> anyhow::Result<Arc<ConfigWatcher>> {
    ConfigWatcher::from_env().await
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
/// - Applies replay protection via `(leader_id, nonce)` (retries are safe; conflicting replays are rejected).
/// - Signs the JSON response with the tool's Ed25519 signing key so the Leader can verify provenance.
///   This includes error responses after the request has been authenticated (e.g. `403`, `422`, `500`).
///
/// Operational note: your gateway/proxy must forward the `X-Nexus-Sig-*` headers in both directions.
///
/// ## Example config
/// ```json
/// {
///   "version": 1,
///   "invoke_max_body_bytes": 10485760,
///   "signed_http": {
///     "mode": "required",
///     "allowed_leaders_path": "./allowed_leaders.json",
///     "tools": {
///       "xyz.dummy.tool@1": {
///         "tool_kid": 0,
///         "tool_signing_key": "0000000000000000000000000000000000000000000000000000000000000000"
///       }
///     }
///   }
/// }
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
        use {
            ::std::sync::Arc,
            $crate::warp::{http::StatusCode, Filter},
        };

        // Load toolkit config (shared across all tool routes).
        let toolkit_cfg = $crate::runtime::load_config_()
            .await
            .expect("Failed to load Nexus toolkit config");

        // Create routes for each Tool in the bundle using reloadable config.
        let routes = $crate::runtime::routes_for_with_watcher_::<$tool>(toolkit_cfg.clone())
            .await
            .expect("Failed to create routes for tool");
        $(
            let routes = routes.or(
                $crate::runtime::routes_for_with_watcher_::<$next_tool>(toolkit_cfg.clone())
                    .await
                    .expect("Failed to create routes for tool")
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
        // Serve the routes.
        $crate::warp::serve(routes).run($addr).await
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

/// This function generates the necessary routes for a given [NexusTool].
///
/// **This is an internal function used by [bootstrap!] macro and should not be
/// used directly.**
#[doc(hidden)]
pub fn routes_for_<T: NexusTool>() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let toolkit_cfg =
        Arc::new(ToolkitRuntimeConfig::from_env().expect("Failed to load Nexus toolkit config"));
    routes_for_with_config_::<T>(toolkit_cfg)
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
    let invoke_auth = InvokeAuthRuntime::from_toolkit_config_for_tool_id(&toolkit_cfg, &tool_id)
        .expect("Failed to load signed HTTP configuration");

    // Invoke path is tool base URL path and `/invoke`.
    let invoke_route = warp::post()
        .and(base_path)
        .and(warp::path("invoke"))
        .and(warp::path::full())
        .and(warp::query::raw().or(warp::any().map(String::new)).unify())
        .and(warp::header::headers_cloned())
        .and(warp::body::content_length_limit(invoke_max_body_bytes))
        .and(warp::body::bytes())
        .and(warp::any().map(move || invoke_auth.clone()))
        .and_then(invoke_handler::<T>);

    health_route.or(meta_route).or(invoke_route)
}

/// Internal route builder with hot-reload support.
///
/// Used by bootstrap! macro. Supports automatic config reload.
#[doc(hidden)]
pub async fn routes_for_with_watcher_<T: NexusTool>(
    toolkit_cfg: Arc<ConfigWatcher>,
) -> anyhow::Result<impl Filter<Extract = impl Reply, Error = Rejection> + Clone> {
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

    let current_config = toolkit_cfg.current().await;
    let invoke_max_body_bytes = current_config.invoke_max_body_bytes();

    let tool_id = T::fqn().to_string();
    let invoke_auth = InvokeAuth::new(toolkit_cfg, tool_id).await?;

    // Invoke path is tool base URL path and `/invoke`.
    let invoke_route = warp::post()
        .and(base_path)
        .and(warp::path("invoke"))
        .and(warp::path::full())
        .and(warp::query::raw().or(warp::any().map(String::new)).unify())
        .and(warp::header::headers_cloned())
        .and(warp::body::content_length_limit(invoke_max_body_bytes))
        .and(warp::body::bytes())
        .and(warp::any().map(move || invoke_auth.clone()))
        .and_then(invoke_handler_::<T>);

    Ok(health_route.or(meta_route).or(invoke_route))
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
///
/// This intermediate form exists so:
/// - the response body can be signed as raw bytes (signed HTTP), and
/// - unsigned mode can still reuse the exact same serialization path.
struct InvokePipelineResponse {
    status: StatusCode,
    body: Vec<u8>,
}

impl InvokePipelineResponse {
    fn json(status: StatusCode, value: serde_json::Value) -> Self {
        let (status, body) = json_bytes_or_fallback(status, value);
        Self { status, body }
    }
}

/// Tool invocation pipeline that returns raw JSON bytes and an HTTP status code.
///
/// Keeping this as a distinct step makes signed response behavior easy to reason about: the
/// signature covers *exactly* these bytes.
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

        match serde_json::to_vec(&crate::WithSerdeErrorPath(output)) {
            Ok(body) => InvokePipelineResponse {
                status: StatusCode::OK,
                body,
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

async fn invoke_handler<T: NexusTool>(
    full_path: FullPath,
    raw_query: String,
    headers: HeaderMap,
    body: bytes::Bytes,
    auth: InvokeAuthRuntime,
) -> Result<warp::reply::Response, Rejection> {
    let body_bytes = body.to_vec();

    let http = HttpRequestMeta {
        method: "POST",
        path: full_path.as_str(),
        query: &raw_query,
    };

    Ok(handle_invoke(
        auth,
        http,
        headers,
        body_bytes,
        |auth_ctx, body_bytes| async move {
            let pipeline = InvokePipeline::run::<T>(&body_bytes, auth_ctx).await;
            (pipeline.status, pipeline.body)
        },
    )
    .await)
}

async fn invoke_handler_<T: NexusTool>(
    full_path: FullPath,
    raw_query: String,
    headers: HeaderMap,
    body: bytes::Bytes,
    auth: InvokeAuth,
) -> Result<warp::reply::Response, Rejection> {
    let body_bytes = body.to_vec();

    let http = HttpRequestMeta {
        method: "POST",
        path: full_path.as_str(),
        query: &raw_query,
    };

    let current_auth = auth.current().await;

    Ok(handle_invoke(
        current_auth,
        http,
        headers,
        body_bytes,
        |auth_ctx, body_bytes| async move {
            let pipeline = InvokePipeline::run::<T>(&body_bytes, auth_ctx).await;
            (pipeline.status, pipeline.body)
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
            signed_http::v1::wire::{
                decode_signature_headers_v1,
                encode_signature_headers_v1,
                now_ms,
                sha256,
                sha256_hex,
                sign_invoke_request_v1,
                verify_invoke_response_v1,
                InvokeRequestClaimsV1,
                VerifyOptions,
                HEADER_SIG,
                HEADER_SIG_INPUT,
                HEADER_SIG_VERSION,
                SIG_VERSION_V1,
            },
            ToolFqn,
        },
        schemars::JsonSchema,
        serde::{Deserialize, Serialize},
        std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    static AUTHORIZE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[derive(Deserialize, JsonSchema)]
    struct Input {
        message: String,
    }

    #[derive(Serialize, JsonSchema)]
    enum Output {
        Ok { message: String },
    }

    struct DenyTool;

    impl NexusTool for DenyTool {
        type Input = Input;
        type Output = Output;

        fn fqn() -> ToolFqn {
            fqn!("xyz.taluslabs.deny@1")
        }

        async fn new() -> Self {
            Self
        }

        async fn authorize(&self, _ctx: crate::AuthContext) -> anyhow::Result<()> {
            AUTHORIZE_CALLS.fetch_add(1, Ordering::SeqCst);
            anyhow::bail!("leader not allowed")
        }

        async fn invoke(&self, input: Self::Input) -> Self::Output {
            Output::Ok {
                message: input.message,
            }
        }

        async fn health(&self) -> anyhow::Result<StatusCode> {
            Ok(StatusCode::OK)
        }
    }

    fn make_config_json(
        tool_id: &str,
        tool_sk_hex: &str,
        leader_id: &str,
        leader_pk_hex: &str,
    ) -> String {
        format!(
            r#"{{
  "version": 1,
  "invoke_max_body_bytes": 1048576,
  "signed_http": {{
    "mode": "required",
    "allowed_leaders": {{
      "version": 1,
      "leaders": [{{"leader_id":"{leader_id}","keys":[{{"kid":0,"public_key":"{leader_pk_hex}"}}]}}]
    }},
    "tools": {{
      "{tool_id}": {{
        "tool_kid": 0,
        "tool_signing_key": "{tool_sk_hex}"
      }}
    }}
  }}
}}"#
        )
    }

    fn build_signed_request(
        leader_id: &str,
        leader_sk: &ed25519_dalek::SigningKey,
        tool_id: &str,
        nonce: &str,
        body: &[u8],
    ) -> (EncodedRequest, [u8; 32]) {
        let iat_ms = now_ms();
        let exp_ms = iat_ms + 60_000;

        let claims = InvokeRequestClaimsV1 {
            leader_id: leader_id.to_string(),
            leader_kid: 0,
            tool_id: tool_id.to_string(),
            iat_ms,
            exp_ms,
            nonce: nonce.to_string(),
            method: "POST".to_string(),
            path: "/invoke".to_string(),
            query: "".to_string(),
            body_sha256: sha256_hex(body),
        };

        let (sig_input, sig) = sign_invoke_request_v1(&claims, leader_sk).unwrap();
        let req_hash = sha256(&sig_input);
        let headers = encode_signature_headers_v1(&sig_input, &sig);

        (
            EncodedRequest {
                sig_input_b64: headers.sig_input_b64,
                sig_b64: headers.sig_b64,
                body: body.to_vec(),
            },
            req_hash,
        )
    }

    struct EncodedRequest {
        sig_input_b64: String,
        sig_b64: String,
        body: Vec<u8>,
    }

    fn verify_signed_response(
        resp: &warp::http::Response<bytes::Bytes>,
        tool_id: &str,
        req_hash: [u8; 32],
        tool_pk: [u8; 32],
    ) {
        let sig_v = resp
            .headers()
            .get(HEADER_SIG_VERSION)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(sig_v, SIG_VERSION_V1);

        let sig_input_b64 = resp
            .headers()
            .get(HEADER_SIG_INPUT)
            .unwrap()
            .to_str()
            .unwrap();
        let sig_b64 = resp.headers().get(HEADER_SIG).unwrap().to_str().unwrap();

        let decoded =
            decode_signature_headers_v1(Some(sig_v), Some(sig_input_b64), Some(sig_b64)).unwrap();

        let verified = verify_invoke_response_v1(
            decoded,
            resp.body(),
            tool_id,
            req_hash,
            tool_pk,
            &VerifyOptions::default(),
        )
        .unwrap();

        assert_eq!(verified.claims.status, resp.status().as_u16());
    }

    #[tokio::test]
    async fn signed_error_responses_are_signed_and_cached() {
        let _guard = TEST_LOCK.lock().unwrap();
        AUTHORIZE_CALLS.store(0, Ordering::SeqCst);

        let leader_id = "0x1111";
        let leader_sk = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());

        let tool_id = DenyTool::fqn().to_string();
        let tool_sk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let tool_pk = tool_sk.verifying_key().to_bytes();
        let tool_sk_hex = hex::encode(tool_sk.to_bytes());

        let cfg_json = make_config_json(&tool_id, &tool_sk_hex, leader_id, &leader_pk_hex);
        let cfg = Arc::new(ToolkitRuntimeConfig::from_json_str(&cfg_json).unwrap());
        let routes = routes_for_with_config_::<DenyTool>(cfg);

        let body = br#"{"message":"hi"}"#;
        let (req, req_hash) = build_signed_request(leader_id, &leader_sk, &tool_id, "abc", body);

        let resp1 = warp::test::request()
            .method("POST")
            .path("/invoke")
            .header("content-length", req.body.len().to_string())
            .header(HEADER_SIG_VERSION, SIG_VERSION_V1)
            .header(HEADER_SIG_INPUT, req.sig_input_b64.clone())
            .header(HEADER_SIG, req.sig_b64.clone())
            .body(req.body.clone())
            .reply(&routes)
            .await;

        assert_eq!(resp1.status(), StatusCode::FORBIDDEN);
        verify_signed_response(&resp1, &tool_id, req_hash, tool_pk);
        assert_eq!(AUTHORIZE_CALLS.load(Ordering::SeqCst), 1);

        // Exact retry returns cached bytes and does not re-run authorization or tool code.
        let resp2 = warp::test::request()
            .method("POST")
            .path("/invoke")
            .header("content-length", req.body.len().to_string())
            .header(HEADER_SIG_VERSION, SIG_VERSION_V1)
            .header(HEADER_SIG_INPUT, req.sig_input_b64.clone())
            .header(HEADER_SIG, req.sig_b64.clone())
            .body(req.body.clone())
            .reply(&routes)
            .await;

        assert_eq!(resp2.status(), StatusCode::FORBIDDEN);
        assert_eq!(resp2.body(), resp1.body());
        assert_eq!(
            resp2.headers().get(HEADER_SIG_INPUT),
            resp1.headers().get(HEADER_SIG_INPUT)
        );
        assert_eq!(
            resp2.headers().get(HEADER_SIG),
            resp1.headers().get(HEADER_SIG)
        );
        assert_eq!(AUTHORIZE_CALLS.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn conflicting_replay_is_signed_and_does_not_overwrite_cache() {
        let _guard = TEST_LOCK.lock().unwrap();
        AUTHORIZE_CALLS.store(0, Ordering::SeqCst);

        let leader_id = "0x1111";
        let leader_sk = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk_hex = hex::encode(leader_sk.verifying_key().to_bytes());

        let tool_id = DenyTool::fqn().to_string();
        let tool_sk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let tool_pk = tool_sk.verifying_key().to_bytes();
        let tool_sk_hex = hex::encode(tool_sk.to_bytes());

        let cfg_json = make_config_json(&tool_id, &tool_sk_hex, leader_id, &leader_pk_hex);
        let cfg = Arc::new(ToolkitRuntimeConfig::from_json_str(&cfg_json).unwrap());
        let routes = routes_for_with_config_::<DenyTool>(cfg);

        let body1 = br#"{"message":"hi"}"#;
        let (req1, req_hash1) = build_signed_request(leader_id, &leader_sk, &tool_id, "abc", body1);

        let resp1 = warp::test::request()
            .method("POST")
            .path("/invoke")
            .header("content-length", req1.body.len().to_string())
            .header(HEADER_SIG_VERSION, SIG_VERSION_V1)
            .header(HEADER_SIG_INPUT, req1.sig_input_b64.clone())
            .header(HEADER_SIG, req1.sig_b64.clone())
            .body(req1.body.clone())
            .reply(&routes)
            .await;

        assert_eq!(resp1.status(), StatusCode::FORBIDDEN);
        verify_signed_response(&resp1, &tool_id, req_hash1, tool_pk);
        assert_eq!(AUTHORIZE_CALLS.load(Ordering::SeqCst), 1);

        // Conflicting request: same nonce, different payload/signature => signed replay rejection.
        let body2 = br#"{"message":"different"}"#;
        let (req2, req_hash2) = build_signed_request(leader_id, &leader_sk, &tool_id, "abc", body2);

        let resp2 = warp::test::request()
            .method("POST")
            .path("/invoke")
            .header("content-length", req2.body.len().to_string())
            .header(HEADER_SIG_VERSION, SIG_VERSION_V1)
            .header(HEADER_SIG_INPUT, req2.sig_input_b64.clone())
            .header(HEADER_SIG, req2.sig_b64.clone())
            .body(req2.body.clone())
            .reply(&routes)
            .await;

        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);
        verify_signed_response(&resp2, &tool_id, req_hash2, tool_pk);
        assert_eq!(AUTHORIZE_CALLS.load(Ordering::SeqCst), 1);

        // The cached response for the original request is still served for exact retries.
        let resp3 = warp::test::request()
            .method("POST")
            .path("/invoke")
            .header("content-length", req1.body.len().to_string())
            .header(HEADER_SIG_VERSION, SIG_VERSION_V1)
            .header(HEADER_SIG_INPUT, req1.sig_input_b64.clone())
            .header(HEADER_SIG, req1.sig_b64.clone())
            .body(req1.body.clone())
            .reply(&routes)
            .await;

        assert_eq!(resp3.status(), StatusCode::FORBIDDEN);
        assert_eq!(resp3.body(), resp1.body());
        assert_eq!(
            resp3.headers().get(HEADER_SIG_INPUT),
            resp1.headers().get(HEADER_SIG_INPUT)
        );
        assert_eq!(
            resp3.headers().get(HEADER_SIG),
            resp1.headers().get(HEADER_SIG)
        );
        assert_eq!(AUTHORIZE_CALLS.load(Ordering::SeqCst), 1);
    }
}
