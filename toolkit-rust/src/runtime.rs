//! See <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

use {
    crate::{config::ToolkitRuntimeConfig, NexusTool},
    nexus_sdk::signed_http::v1::{
        decode_signature_headers_v1,
        encode_signature_headers_v1,
        now_ms,
        sha256_hex,
        sign_invoke_response_v1,
        verify_invoke_request_v1,
        AllowedLeadersV1,
        HttpRequestMeta,
        InvokeResponseClaimsV1,
        SignedHttpError,
        VerifiedInvokeRequestV1,
        VerifyOptions,
        HEADER_SIG,
        HEADER_SIG_INPUT,
        HEADER_SIG_VERSION,
        SIG_VERSION_V1,
    },
    reqwest::Url,
    serde_json::json,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
    warp::{
        filters::{host::Authority, path::FullPath},
        http::{header::HeaderValue, HeaderMap, StatusCode},
        Filter,
        Rejection,
        Reply,
    },
};

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

/// Low-level HTTP response builder utilities for the toolkit runtime.
///
/// This is intentionally separated from tool execution and signing logic.
struct HttpResponder;

impl HttpResponder {
    fn json_value(status: StatusCode, value: serde_json::Value) -> warp::reply::Response {
        let (status, body) = json_bytes_or_fallback(status, value);
        Self::json_bytes(status, body)
    }

    fn json_bytes(status: StatusCode, body: Vec<u8>) -> warp::reply::Response {
        let mut response = warp::reply::Response::new(warp::hyper::Body::from(body));
        *response.status_mut() = status;
        response
            .headers_mut()
            .insert("content-type", HeaderValue::from_static("application/json"));
        response
    }

    fn auth_failed(err: SignedHttpError) -> warp::reply::Response {
        Self::json_value(
            StatusCode::UNAUTHORIZED,
            json!({
                "error": "auth_failed",
                "details": err.to_string(),
            }),
        )
    }

    fn signed_response(resp: StoredResponse) -> warp::reply::Response {
        let mut response = warp::reply::Response::new(warp::hyper::Body::from(resp.body));
        *response.status_mut() = resp.status;

        let headers = response.headers_mut();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(HEADER_SIG_VERSION, HeaderValue::from_static(SIG_VERSION_V1));
        headers.insert(
            HEADER_SIG_INPUT,
            HeaderValue::from_str(&resp.sig_headers.0)
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers.insert(
            HEADER_SIG,
            HeaderValue::from_str(&resp.sig_headers.1)
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );

        response
    }
}

#[derive(Clone)]
/// Per-tool runtime state for signed HTTP.
///
/// When signed HTTP is enabled, the toolkit runtime needs:
/// - static configuration (which tool id am I, which signing key do I use, which Leaders are
///   allowed, what time windows are acceptable), and
/// - dynamic state (nonce/replay tracking to distinguish safe retries from adversarial replays).
///
/// This struct is created once per tool type `T: NexusTool` inside [`routes_for_`], cloned into the
/// warp filter chain, and used by [`invoke_handler`] to:
/// - verify the Leader's request signature against [`AllowedLeadersV1`],
/// - enforce time window limits (`max_clock_skew_ms`, `max_validity_ms`),
/// - reject replays and safely handle retries via `nonce_cache`,
/// - sign the tool response with `tool_signing_key`.
struct SignedHttpToolState {
    /// Tool identifier (string form of `ToolFqn`).
    tool_id: String,
    /// Tool signing key id (for rotation).
    tool_kid: u64,
    /// Tool Ed25519 signing key used to sign responses.
    ///
    /// This should be treated as the secret that allows the process to "speak as" the ToolId.
    tool_signing_key: ed25519_dalek::SigningKey,
    /// Local allowlist of permitted Leaders (no RPC).
    allowed_leaders: AllowedLeadersV1,
    /// Maximum accepted clock skew when validating request `iat_ms`/`exp_ms`.
    max_clock_skew_ms: u64,
    /// Maximum accepted validity window (`exp_ms - iat_ms`) for requests/responses.
    max_validity_ms: u64,
    /// In-memory nonce cache used for replay resistance and safe retries.
    ///
    /// The cache key is `(leader_id, nonce)` encoded as a string (`"{leader_id}:{nonce}"`).
    ///
    /// This is an in-process cache. If you run multiple replicas, nonce/retry behavior is only
    /// guaranteed per replica unless you replace this with shared storage.
    nonce_cache: NonceCache,
}

#[derive(Clone)]
/// Entry stored in [`SignedHttpToolState::nonce_cache`].
///
/// Each nonce is accepted within a bounded time window and is associated with:
/// - `request_hash`: `sha256(sig_input_bytes)` for the verified request (stable binding),
/// - `expires_at_ms`: taken from the request claims `exp_ms`, so entries are self-expiring,
/// - `state`: whether a request is currently being processed or already completed.
struct NonceEntry {
    request_hash: [u8; 32],
    expires_at_ms: u64,
    state: NonceState,
}

#[derive(Clone)]
/// State of a nonce in the replay cache.
///
/// - `InFlight`: a request with this nonce is currently executing; a second request is treated as a
///   concurrent retry and rejected with `409 Conflict` to avoid double execution.
/// - `Complete`: the tool already produced a response; identical retries are served from cache.
enum NonceState {
    InFlight,
    Complete(StoredResponse),
}

#[derive(Clone)]
/// Cached response for a completed nonce.
///
/// We cache the already-signed response headers and body so that retries return the exact same
/// bytes, which makes idempotency and auditing straightforward.
struct StoredResponse {
    status: StatusCode,
    body: Vec<u8>,
    sig_headers: (String, String), // (sig_input_b64, sig_b64)
}

/// In-memory nonce cache for replay resistance and safe retries.
///
/// A "nonce" is a per-request unique token chosen by the Leader and included in the signed claims.
/// The tool runtime uses it to detect replays and make network retries idempotent.
///
/// This is intentionally small and self-contained:
/// - It is keyed by `(leader_id, nonce)` (`"{leader_id}:{nonce}"`).
/// - It stores `InFlight` and `Complete` state, allowing idempotent retries.
/// - It purges entries based on the request claims' `exp_ms`.
///
/// If you run multiple replicas and need global replay protection, replace this with shared storage.
#[derive(Clone)]
struct NonceCache {
    inner: Arc<Mutex<HashMap<String, NonceEntry>>>,
}

impl NonceCache {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn remove(&self, nonce_key: &str) {
        let mut cache = self.inner.lock().unwrap();
        cache.remove(nonce_key);
    }

    /// Determine whether a request can proceed, is a safe retry, or is a replay.
    ///
    /// Behavior:
    /// - If the nonce key is unseen, insert `InFlight` and return [`NonceDecision::Proceed`].
    /// - If seen with a different `request_hash`, return [`NonceDecision::Reject`].
    /// - If seen `InFlight`, return [`NonceDecision::InFlight`].
    /// - If seen `Complete`, return the stored response (`Return`) for idempotent retries.
    fn begin_or_replay(
        &self,
        nonce_key: &str,
        request_hash: [u8; 32],
        expires_at_ms: u64,
    ) -> NonceDecision {
        let mut cache = self.inner.lock().unwrap();
        purge_expired(&mut cache, now_ms());

        match cache.get(nonce_key) {
            None => {
                cache.insert(
                    nonce_key.to_string(),
                    NonceEntry {
                        request_hash,
                        expires_at_ms,
                        state: NonceState::InFlight,
                    },
                );
                NonceDecision::Proceed
            }
            Some(entry) if entry.request_hash != request_hash => NonceDecision::Reject,
            Some(entry) => match &entry.state {
                NonceState::InFlight => NonceDecision::InFlight,
                NonceState::Complete(resp) => NonceDecision::Return(resp.clone()),
            },
        }
    }

    /// Mark a nonce as completed and store the signed response for future retries.
    fn complete(
        &self,
        nonce_key: &str,
        request_hash: [u8; 32],
        expires_at_ms: u64,
        response: StoredResponse,
    ) {
        let mut cache = self.inner.lock().unwrap();
        cache.insert(
            nonce_key.to_string(),
            NonceEntry {
                request_hash,
                expires_at_ms,
                state: NonceState::Complete(response),
            },
        );
    }
}

/// Drop guard that cleans up an `InFlight` nonce entry on early-return.
///
/// In the normal success path we "disarm" the guard and transition the nonce to `Complete`.
/// On error (panic-safe early return), dropping the guard removes the entry so a client can retry.
struct NonceGuard {
    key: String,
    cache: NonceCache,
    armed: bool,
}

impl NonceGuard {
    fn new(cache: NonceCache, key: String) -> Self {
        Self {
            key,
            cache,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for NonceGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.cache.remove(&self.key);
    }
}

/// Verified and replay-safe invocation context for a signed `/invoke` request.
///
/// This is the "consolidated" object returned by authentication. It exists so the handler doesn't
/// have to juggle multiple `Option`s (verified request + nonce guard + nonce key).
///
/// - If the request later fails (e.g. input JSON is invalid), dropping this context removes the
///   `InFlight` nonce entry so the client can retry.
/// - On success, the handler disarms the guard and stores the signed response in the nonce cache.
struct SignedInvokeContext {
    verified: VerifiedInvokeRequestV1,
    nonce_key: String,
    nonce_guard: NonceGuard,
}

impl SignedHttpToolState {
    fn from_toolkit_config_for_tool_id(
        toolkit_cfg: &ToolkitRuntimeConfig,
        tool_id: String,
    ) -> anyhow::Result<Option<Self>> {
        let Some(signed_http) = toolkit_cfg.signed_http() else {
            return Ok(None);
        };

        let tool = signed_http.tools.get(&tool_id).ok_or_else(|| {
            let src = toolkit_cfg
                .source_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<defaults>".to_string());
            anyhow::anyhow!(
                "signed_http is enabled but no signing key is configured for tool_id='{tool_id}' (config={src})"
            )
        })?;

        Ok(Some(Self {
            tool_id,
            tool_kid: tool.tool_kid,
            tool_signing_key: tool.tool_signing_key.clone(),
            allowed_leaders: signed_http.allowed_leaders.clone(),
            max_clock_skew_ms: signed_http.max_clock_skew_ms,
            max_validity_ms: signed_http.max_validity_ms,
            nonce_cache: NonceCache::new(),
        }))
    }

    fn verify_opts(&self) -> VerifyOptions {
        VerifyOptions {
            now_ms: now_ms(),
            max_clock_skew_ms: self.max_clock_skew_ms,
            max_validity_ms: self.max_validity_ms,
        }
    }

    /// Authenticate and replay-check an incoming `/invoke` request.
    ///
    /// Returns a fully verified context (claims + leader key + nonce guard) or a ready-to-return
    /// HTTP response explaining why authentication failed.
    fn authenticate_invoke(
        &self,
        full_path: &str,
        raw_query: &str,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Result<SignedInvokeContext, warp::reply::Response> {
        let sig_v = header_str(headers, HEADER_SIG_VERSION);
        let sig_input_b64 = header_str(headers, HEADER_SIG_INPUT);
        let sig_b64 = header_str(headers, HEADER_SIG);

        let decoded = decode_signature_headers_v1(sig_v, sig_input_b64, sig_b64)
            .map_err(HttpResponder::auth_failed)?;

        let http = HttpRequestMeta {
            method: "POST",
            path: full_path,
            query: raw_query,
        };

        let opts = self.verify_opts();
        let verified = verify_invoke_request_v1(
            decoded,
            http,
            body,
            &self.tool_id,
            &self.allowed_leaders,
            &opts,
        )
        .map_err(HttpResponder::auth_failed)?;

        let nonce_key = format!("{}:{}", verified.claims.leader_id, verified.claims.nonce);
        let expires_at_ms = verified.claims.exp_ms;

        match self
            .nonce_cache
            .begin_or_replay(&nonce_key, verified.sig_input_sha256, expires_at_ms)
        {
            NonceDecision::Proceed => Ok(SignedInvokeContext {
                verified,
                nonce_guard: NonceGuard::new(self.nonce_cache.clone(), nonce_key.clone()),
                nonce_key,
            }),
            NonceDecision::Return(resp) => Err(HttpResponder::signed_response(resp)),
            NonceDecision::Reject => {
                let reply = json!({
                    "error": "replay_rejected",
                    "details": "nonce already used with different request",
                });
                let (status, body) = json_bytes_or_fallback(StatusCode::UNAUTHORIZED, reply);
                match self.sign_invoke_response(&verified, status, &body) {
                    Ok(stored) => Err(HttpResponder::signed_response(stored)),
                    Err(e) => {
                        let reply = json!({
                            "error": "response_signing_error",
                            "details": e.to_string(),
                        });
                        Err(HttpResponder::json_value(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            reply,
                        ))
                    }
                }
            }
            NonceDecision::InFlight => {
                let reply = json!({
                    "error": "request_in_flight",
                    "details": "request with same nonce is still processing",
                });
                let (status, body) = json_bytes_or_fallback(StatusCode::CONFLICT, reply);
                match self.sign_invoke_response(&verified, status, &body) {
                    Ok(stored) => Err(HttpResponder::signed_response(stored)),
                    Err(e) => {
                        let reply = json!({
                            "error": "response_signing_error",
                            "details": e.to_string(),
                        });
                        Err(HttpResponder::json_value(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            reply,
                        ))
                    }
                }
            }
        }
    }

    /// Sign a tool invocation response and return a cached representation.
    ///
    /// The returned [`StoredResponse`] contains:
    /// - the response status + body bytes, and
    /// - the signature headers (`X-Nexus-Sig-*`) produced from the signed response claims.
    ///
    /// Callers typically persist this via [`NonceCache::complete`] so identical retries return the
    /// exact same bytes.
    fn sign_invoke_response(
        &self,
        verified: &VerifiedInvokeRequestV1,
        status: StatusCode,
        response_body: &[u8],
    ) -> Result<StoredResponse, SignedHttpError> {
        let iat_ms = now_ms();
        let exp_ms = iat_ms.saturating_add(self.max_validity_ms);

        let claims = InvokeResponseClaimsV1 {
            tool_id: self.tool_id.clone(),
            tool_kid: self.tool_kid,
            iat_ms,
            exp_ms,
            nonce: verified.claims.nonce.clone(),
            req_sig_input_sha256: hex::encode(verified.sig_input_sha256),
            status: status.as_u16(),
            body_sha256: sha256_hex(response_body),
        };

        let (sig_input, sig) = sign_invoke_response_v1(&claims, &self.tool_signing_key)?;

        let sig_headers = encode_signature_headers_v1(&sig_input, &sig);
        Ok(StoredResponse {
            status,
            body: response_body.to_vec(),
            sig_headers: (sig_headers.sig_input_b64, sig_headers.sig_b64),
        })
    }

    /// Finish a signed invocation by signing, caching, and building the HTTP response.
    ///
    /// This method is the single place where signed-HTTP response behavior is enforced:
    /// - all post-auth responses are signed (including errors),
    /// - the `(leader_id, nonce)` entry is transitioned to `Complete` and cached, and
    /// - retries for the same nonce return the exact same bytes.
    fn finish_signed_invoke(
        &self,
        mut ctx: SignedInvokeContext,
        status: StatusCode,
        body: Vec<u8>,
    ) -> warp::reply::Response {
        match self.sign_invoke_response(&ctx.verified, status, &body) {
            Ok(stored) => {
                self.nonce_cache.complete(
                    &ctx.nonce_key,
                    ctx.verified.sig_input_sha256,
                    ctx.verified.claims.exp_ms,
                    stored.clone(),
                );
                ctx.nonce_guard.disarm();
                HttpResponder::signed_response(stored)
            }
            Err(e) => {
                let reply = json!({
                    "error": "response_signing_error",
                    "details": e.to_string(),
                });
                HttpResponder::json_value(StatusCode::INTERNAL_SERVER_ERROR, reply)
            }
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

        // Load toolkit config once per process (shared across all tool routes).
        let toolkit_cfg = Arc::new(
            $crate::ToolkitRuntimeConfig::from_env().expect("Failed to load Nexus toolkit config"),
        );

        // Create routes for each Tool in the bundle.
        let routes = $crate::routes_for_with_config_::<$tool>(toolkit_cfg.clone());
        $(let routes = routes.or($crate::routes_for_with_config_::<$next_tool>(toolkit_cfg.clone()));)*

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
    let signed_http =
        SignedHttpToolState::from_toolkit_config_for_tool_id(&toolkit_cfg, tool_id.clone())
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
        .and(warp::any().map(move || signed_http.clone()))
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

/// An `/invoke` request session (unsigned vs signed HTTP).
///
/// This consolidates the state needed to:
/// - build an optional [`crate::AuthContext`] for tool-level authorization, and
/// - produce the final HTTP response (signed + cached, or plain JSON).
enum InvokeSession {
    Unsigned,
    Signed {
        cfg: SignedHttpToolState,
        ctx: SignedInvokeContext,
    },
}

impl InvokeSession {
    fn auth_context(&self) -> Option<crate::AuthContext> {
        match self {
            Self::Unsigned => None,
            Self::Signed { ctx, .. } => Some(crate::AuthContext::from_verified(&ctx.verified)),
        }
    }

    fn finish(self, pipeline: InvokePipelineResponse) -> warp::reply::Response {
        match self {
            Self::Unsigned => HttpResponder::json_bytes(pipeline.status, pipeline.body),
            Self::Signed { cfg, ctx } => {
                cfg.finish_signed_invoke(ctx, pipeline.status, pipeline.body)
            }
        }
    }
}

async fn invoke_handler<T: NexusTool>(
    full_path: FullPath,
    raw_query: String,
    headers: HeaderMap,
    body: bytes::Bytes,
    signed_http: Option<SignedHttpToolState>,
) -> Result<warp::reply::Response, Rejection> {
    let body_bytes = body.to_vec();

    let session = match signed_http {
        None => InvokeSession::Unsigned,
        Some(cfg) => {
            match cfg.authenticate_invoke(full_path.as_str(), &raw_query, &headers, &body_bytes) {
                Ok(ctx) => InvokeSession::Signed { cfg, ctx },
                Err(resp) => return Ok(resp),
            }
        }
    };

    let pipeline = InvokePipeline::run::<T>(&body_bytes, session.auth_context()).await;
    Ok(session.finish(pipeline))
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// Decision returned by the nonce cache when processing a `(leader_id, nonce)`.
///
/// This models the retry/replay behavior:
/// - `Proceed`: first time seen; mark `InFlight` and allow execution.
/// - `Return`: identical retry; return the cached response bytes.
/// - `Reject`: conflicting replay (same nonce, different request hash).
/// - `InFlight`: concurrent retry while the original is still executing.
enum NonceDecision {
    Proceed,
    Return(StoredResponse),
    Reject,
    InFlight,
}

/// Remove expired nonce entries from the cache.
///
/// Entries expire at the request claims' `exp_ms` (with skew tolerance enforced during verification).
fn purge_expired(cache: &mut HashMap<String, NonceEntry>, now_ms: u64) {
    cache.retain(|_, entry| entry.expires_at_ms >= now_ms);
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            fqn,
            signed_http::v1::{
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

        fn authorize(
            &self,
            _ctx: crate::AuthContext,
        ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
            async move {
                AUTHORIZE_CALLS.fetch_add(1, Ordering::SeqCst);
                anyhow::bail!("leader not allowed")
            }
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
