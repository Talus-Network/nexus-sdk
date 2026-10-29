//! Warp integration for the minimal signed Tool transport.

use {
    crate::{config::Config, AuthContext, SignedHttpVersion, ToolkitRuntimeConfig},
    ed25519_dalek::SigningKey,
    nexus_sdk::signed_http::{
        v3::{wire as v3_wire, wire::AllowedLeaders},
        v4::wire as v4_wire,
    },
    serde_json::json,
    std::{
        collections::HashMap,
        future::Future,
        sync::{Arc, Mutex, RwLock},
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
    warp::http::{header::HeaderValue, HeaderMap, StatusCode},
};
#[cfg(test)]
const TEST_TOOL_ID: [u8; 32] = [42; 32];
const DEFAULT_IN_FLIGHT_LEASE_MS: u64 = 60_000;
const JSON_CONTENT_TYPE: &str = "application/json";

struct RefreshingAllowedLeadersResolver {
    allowed_leaders: RwLock<AllowedLeaders>,
}

impl RefreshingAllowedLeadersResolver {
    fn new(allowed_leaders: AllowedLeaders) -> Self {
        Self {
            allowed_leaders: RwLock::new(allowed_leaders),
        }
    }

    fn refresh_from_source_path(&self) {
        let Some(path) = self
            .allowed_leaders
            .read()
            .unwrap()
            .source_path()
            .map(ToOwned::to_owned)
        else {
            return;
        };
        let Ok(updated) = AllowedLeaders::from_path(path) else {
            return;
        };
        *self.allowed_leaders.write().unwrap() = updated;
    }
}

impl v4_wire::LeaderKeyResolver for RefreshingAllowedLeadersResolver {
    fn leader_public_key(&self, leader_id: &str, leader_key_id: u64) -> Option<[u8; 32]> {
        self.refresh_from_source_path();
        self.allowed_leaders
            .read()
            .unwrap()
            .leader_public_key(leader_id, leader_key_id)
    }
}

#[derive(Clone)]
pub(crate) struct SignedInvokeRuntime {
    registered_tool_id: [u8; 32],
    response_signing_key: Option<SigningKey>,
    allowed_leaders: Arc<RefreshingAllowedLeadersResolver>,
    replay_cache_ttl_ms: u64,
}

/// Auth mode for `/invoke` handling.
#[derive(Clone)]
pub(crate) enum InvokeAuthRuntime {
    Unsigned,
    Signed(Box<SignedInvokeRuntime>),
}

impl InvokeAuthRuntime {
    pub fn from_toolkit_config_for_tool_id(
        toolkit_cfg: &ToolkitRuntimeConfig,
        tool_id: &str,
    ) -> anyhow::Result<Self> {
        let Some(signed_http) = toolkit_cfg.signed_http() else {
            return Ok(Self::Unsigned);
        };
        let tool = signed_http.tools.get(tool_id).ok_or_else(|| {
            let source = toolkit_cfg
                .source_path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<defaults>".to_string());
            anyhow::anyhow!(
                "signed_http is enabled but tool_id='{tool_id}' has no configuration (config={source})"
            )
        })?;

        Ok(Self::Signed(Box::new(SignedInvokeRuntime {
            registered_tool_id: tool.registered_tool_id,
            response_signing_key: tool.response_signing_key.clone(),
            allowed_leaders: Arc::new(RefreshingAllowedLeadersResolver::new(
                (*signed_http.allowed_leaders).clone(),
            )),
            replay_cache_ttl_ms: tool.replay_cache_ttl_ms,
        })))
    }
}

#[derive(Clone)]
pub(crate) struct InvokeAuth {
    state: Arc<RwLock<InvokeAuthState>>,
    replay: ReplayCache,
    tool_id: String,
    config: Arc<Config>,
}

struct InvokeAuthState {
    auth: Arc<InvokeAuthRuntime>,
    config_ptr: usize,
}

impl InvokeAuth {
    pub(crate) fn new_sync(
        config: Arc<Config>,
        tool_id: String,
        invocation_timeout: Duration,
    ) -> anyhow::Result<Self> {
        let current_config = config.current();
        let auth = InvokeAuthRuntime::from_toolkit_config_for_tool_id(&current_config, &tool_id)?;
        let config_ptr = Arc::as_ptr(&current_config) as usize;
        let in_flight_lease_ms = u64::try_from(invocation_timeout.as_millis())
            .unwrap_or(DEFAULT_IN_FLIGHT_LEASE_MS)
            .max(1);
        Ok(Self {
            state: Arc::new(RwLock::new(InvokeAuthState {
                auth: Arc::new(auth),
                config_ptr,
            })),
            replay: ReplayCache::new(in_flight_lease_ms),
            tool_id,
            config,
        })
    }

    pub(crate) async fn current(&self) -> Arc<InvokeAuthRuntime> {
        let current_config = self.config.current();
        let current_ptr = Arc::as_ptr(&current_config) as usize;
        {
            let state = self.state.read().unwrap();
            if state.config_ptr == current_ptr {
                return Arc::clone(&state.auth);
            }
        }

        let mut state = self.state.write().unwrap();
        if state.config_ptr != current_ptr {
            if let Ok(auth) =
                InvokeAuthRuntime::from_toolkit_config_for_tool_id(&current_config, &self.tool_id)
            {
                state.auth = Arc::new(auth);
                state.config_ptr = current_ptr;
            }
        }
        Arc::clone(&state.auth)
    }

    pub(crate) fn replay(&self) -> &ReplayCache {
        &self.replay
    }
}

/// Handle one `/invoke` request.
///
/// The callback's boolean marks whether its body is a canonical ordered BCS Tool output. Only those
/// result bodies are signed; local HTTP errors remain JSON and never become verifier evidence.
pub(crate) async fn handle_invoke<F, Fut>(
    auth: &InvokeAuthRuntime,
    replay: &ReplayCache,
    headers: HeaderMap,
    body_bytes: Vec<u8>,
    run: F,
) -> warp::reply::Response
where
    F: FnOnce(Option<AuthContext>, Vec<u8>) -> Fut,
    Fut: Future<Output = (StatusCode, Vec<u8>, bool)> + Send,
{
    match auth {
        InvokeAuthRuntime::Unsigned => {
            let (status, body, is_result) = run(None, body_bytes).await;
            response(status, body, is_result, None)
        }
        InvokeAuthRuntime::Signed(runtime) => {
            let authenticated = match authenticate_request(&headers, runtime) {
                Ok(authenticated) => authenticated,
                Err(error) => return auth_failed(error),
            };
            match replay.begin(authenticated.nonce, authenticated.input_hash, now_ms()) {
                ReplayDecision::Return(cached) => {
                    cached.into_response(&authenticated, runtime.response_signing_key.as_ref())
                }
                ReplayDecision::InFlight => json_response(
                    StatusCode::CONFLICT,
                    json!({
                        "error": "request_in_flight",
                        "details": "request with the same nonce is still processing",
                    }),
                ),
                ReplayDecision::Proceed(reservation) => {
                    let (status, body, is_result) =
                        run(Some(authenticated.clone()), body_bytes).await;
                    let cached = CachedResponse {
                        status,
                        body,
                        is_result,
                    };
                    reservation.complete(cached.clone(), now_ms(), runtime.replay_cache_ttl_ms);
                    cached.into_response(&authenticated, runtime.response_signing_key.as_ref())
                }
            }
        }
    }
}

#[derive(Clone)]
struct CachedResponse {
    status: StatusCode,
    body: Vec<u8>,
    is_result: bool,
}

impl CachedResponse {
    fn into_response(
        self,
        authenticated: &AuthContext,
        response_signing_key: Option<&SigningKey>,
    ) -> warp::reply::Response {
        let response_headers = self.is_result.then(|| match response_signing_key {
            Some(key) => match authenticated.protocol_version {
                SignedHttpVersion::V3 => v3_wire::sign_response(
                    &authenticated.leader_signature,
                    &authenticated.nonce,
                    &self.body,
                    key,
                )
                .to_pairs(),
                SignedHttpVersion::V4 => v4_wire::sign_response(
                    &authenticated.leader_signature,
                    &authenticated.nonce,
                    &self.body,
                    key,
                )
                .to_pairs(),
            },
            None => {
                let version = match authenticated.protocol_version {
                    SignedHttpVersion::V3 => v3_wire::SIGNATURE_VERSION_V3,
                    SignedHttpVersion::V4 => v4_wire::SIGNATURE_VERSION_V4,
                };
                vec![(v4_wire::HEADER_SIGNATURE_VERSION, version.to_string())]
            }
        });
        response(
            self.status,
            self.body,
            self.is_result,
            response_headers.as_deref(),
        )
    }
}

fn authenticate_request(
    headers: &HeaderMap,
    runtime: &SignedInvokeRuntime,
) -> Result<AuthContext, String> {
    match header_str(headers, v4_wire::HEADER_SIGNATURE_VERSION) {
        Some(v3_wire::SIGNATURE_VERSION_V3) => {
            let request_headers =
                v3_wire::RequestHeadersRef::from_getter(|name| header_str(headers, name));
            v3_wire::authenticate_request(request_headers, runtime.allowed_leaders.as_ref())
                .map(AuthContext::from_v3)
                .map_err(|error| error.to_string())
        }
        Some(v4_wire::SIGNATURE_VERSION_V4) => {
            let request_headers =
                v4_wire::RequestHeadersRef::from_getter(|name| header_str(headers, name));
            let authenticated =
                v4_wire::authenticate_request(request_headers, runtime.allowed_leaders.as_ref())
                    .map_err(|error| error.to_string())?;
            if authenticated.tool_id != runtime.registered_tool_id {
                return Err("signed Tool ID does not match this Toolkit configuration".to_string());
            }
            Ok(AuthContext::from_v4(authenticated))
        }
        Some(version) => Err(format!(
            "unsupported signed HTTP version '{version}', expected '3' or '4'"
        )),
        None => Err("missing signed HTTP version header".to_string()),
    }
}

#[derive(Clone)]
pub(crate) struct ReplayCache {
    inner: Arc<Mutex<HashMap<ReplayKey, ReplayEntry>>>,
    in_flight_lease_ms: u64,
}

type ReplayKey = [u8; 32];

#[derive(Clone)]
struct ReplayEntry {
    input_hash: [u8; 32],
    expires_at_ms: u64,
    state: ReplayState,
}

#[derive(Clone)]
enum ReplayState {
    InFlight,
    Complete(CachedResponse),
}

enum ReplayDecision {
    Proceed(ReplayReservation),
    Return(CachedResponse),
    InFlight,
}

struct ReplayReservation {
    cache: ReplayCache,
    nonce: [u8; 32],
    input_hash: [u8; 32],
    lease_expires_at_ms: u64,
    completed: bool,
}

impl ReplayCache {
    fn new(in_flight_lease_ms: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            in_flight_lease_ms,
        }
    }

    fn begin(&self, nonce: [u8; 32], input_hash: [u8; 32], now_ms: u64) -> ReplayDecision {
        let mut entries = self.inner.lock().unwrap();
        entries.retain(|_, entry| entry.expires_at_ms > now_ms);
        let key = nonce;
        match entries.get(&key) {
            Some(ReplayEntry {
                input_hash: cached_input_hash,
                state: ReplayState::Complete(response),
                ..
            }) if *cached_input_hash == input_hash => ReplayDecision::Return(response.clone()),
            Some(ReplayEntry {
                state: ReplayState::InFlight,
                ..
            }) => ReplayDecision::InFlight,
            Some(ReplayEntry {
                state: ReplayState::Complete(_),
                ..
            })
            | None => {
                let lease_expires_at_ms = now_ms.saturating_add(self.in_flight_lease_ms);
                entries.insert(
                    key,
                    ReplayEntry {
                        input_hash,
                        expires_at_ms: lease_expires_at_ms,
                        state: ReplayState::InFlight,
                    },
                );
                ReplayDecision::Proceed(ReplayReservation {
                    cache: self.clone(),
                    nonce,
                    input_hash,
                    lease_expires_at_ms,
                    completed: false,
                })
            }
        }
    }
}

impl ReplayReservation {
    fn complete(mut self, response: CachedResponse, now_ms: u64, completed_ttl_ms: u64) {
        let mut entries = self.cache.inner.lock().unwrap();
        if let Some(entry) = entries.get_mut(&self.nonce) {
            if entry.input_hash == self.input_hash
                && entry.expires_at_ms == self.lease_expires_at_ms
                && matches!(entry.state, ReplayState::InFlight)
            {
                entry.state = ReplayState::Complete(response);
                entry.expires_at_ms = now_ms.saturating_add(completed_ttl_ms);
            }
        }
        self.completed = true;
    }
}

impl Drop for ReplayReservation {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let mut entries = self.cache.inner.lock().unwrap();
        let key = self.nonce;
        if entries.get(&key).is_some_and(|entry| {
            entry.input_hash == self.input_hash
                && entry.expires_at_ms == self.lease_expires_at_ms
                && matches!(entry.state, ReplayState::InFlight)
        }) {
            entries.remove(&key);
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn response(
    status: StatusCode,
    body: Vec<u8>,
    is_result: bool,
    signature: Option<&[(&'static str, String)]>,
) -> warp::reply::Response {
    let mut response = warp::reply::Response::new(body.into());
    *response.status_mut() = status;
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static(if is_result {
            v4_wire::CANONICAL_TOOL_RESPONSE_CONTENT_TYPE
        } else {
            JSON_CONTENT_TYPE
        }),
    );
    if let Some(signature) = signature {
        for (name, value) in signature {
            if let Ok(value) = HeaderValue::from_str(value) {
                response.headers_mut().insert(*name, value);
            }
        }
    }
    response
}

fn json_response(status: StatusCode, value: serde_json::Value) -> warp::reply::Response {
    let body = serde_json::to_vec(&value)
        .unwrap_or_else(|_| br#"{"error":"serialization_error"}"#.to_vec());
    response(status, body, false, None)
}

fn auth_failed(error: impl std::fmt::Display) -> warp::reply::Response {
    json_response(
        StatusCode::UNAUTHORIZED,
        json!({
            "error": "auth_failed",
            "details": error.to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ed25519_dalek::SigningKey,
        http_body_util::BodyExt as _,
        nexus_sdk::signed_http::{
            v3::wire::{
                sign_request as sign_request_v3,
                verify_response as verify_response_v3,
                AllowedLeaderFileV1,
                AllowedLeaderKeyFileV1,
                AllowedLeadersFileV1,
                LeaderKeyResolver,
                ResponseHeadersRef as ResponseHeadersRefV3,
                SIGNATURE_VERSION_V3,
            },
            v4::{
                error::SignedHttpError,
                wire::{
                    authenticate_request as authenticate_request_v4,
                    sign_request,
                    verify_response,
                    EncodedRequestHeaders,
                    RequestHeadersRef,
                    ResponseHeadersRef,
                    CANONICAL_TOOL_RESPONSE_CONTENT_TYPE,
                    HEADER_SIGNATURE_VERSION,
                    HEADER_TOOL_SIGNATURE,
                    SIGNATURE_VERSION_V4,
                },
            },
        },
        std::sync::atomic::{AtomicUsize, Ordering},
    };

    fn allowed_leaders_file(leader_id: &str, kid: u64, key: &SigningKey) -> AllowedLeadersFileV1 {
        AllowedLeadersFileV1 {
            version: 1,
            leaders: vec![allowed_leader(leader_id, kid, key)],
        }
    }

    fn allowed_leader(leader_id: &str, kid: u64, key: &SigningKey) -> AllowedLeaderFileV1 {
        AllowedLeaderFileV1 {
            leader_id: leader_id.to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid,
                public_key: hex::encode(key.verifying_key().to_bytes()),
            }],
        }
    }

    fn signed_runtime(leader: &SigningKey, tool: &SigningKey) -> InvokeAuthRuntime {
        signed_runtime_with_tool_id(leader, tool, TEST_TOOL_ID)
    }

    fn signed_runtime_with_tool_id(
        leader: &SigningKey,
        tool: &SigningKey,
        registered_tool_id: [u8; 32],
    ) -> InvokeAuthRuntime {
        let allowed = AllowedLeaders::try_from(allowed_leaders_file("leader", 0, leader)).unwrap();
        InvokeAuthRuntime::Signed(Box::new(SignedInvokeRuntime {
            registered_tool_id,
            response_signing_key: Some(tool.clone()),
            allowed_leaders: Arc::new(RefreshingAllowedLeadersResolver::new(allowed)),
            replay_cache_ttl_ms: 10_000,
        }))
    }

    fn verification_runtime(leader: &SigningKey) -> InvokeAuthRuntime {
        let allowed = AllowedLeaders::try_from(allowed_leaders_file("leader", 0, leader)).unwrap();
        InvokeAuthRuntime::Signed(Box::new(SignedInvokeRuntime {
            registered_tool_id: TEST_TOOL_ID,
            response_signing_key: None,
            allowed_leaders: Arc::new(RefreshingAllowedLeadersResolver::new(allowed)),
            replay_cache_ttl_ms: 10_000,
        }))
    }

    fn request_headers(leader: &SigningKey, input_hash: [u8; 32], nonce: [u8; 32]) -> HeaderMap {
        let encoded = sign_request("leader", 0, TEST_TOOL_ID, nonce, input_hash, leader);
        headers_from_encoded(encoded)
    }

    fn request_headers_v3(leader: &SigningKey, input_hash: [u8; 32], nonce: [u8; 32]) -> HeaderMap {
        let encoded = sign_request_v3("leader", 0, input_hash, nonce, leader);
        headers_from_pairs(encoded.to_pairs())
    }

    fn headers_from_encoded(encoded: EncodedRequestHeaders) -> HeaderMap {
        headers_from_pairs(encoded.to_pairs())
    }

    fn headers_from_pairs(pairs: Vec<(&'static str, String)>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(name, HeaderValue::from_str(&value).unwrap());
        }
        headers
    }

    fn authenticate_v3_context(headers: &HeaderMap, runtime: &InvokeAuthRuntime) -> AuthContext {
        let InvokeAuthRuntime::Signed(runtime) = runtime else {
            panic!("signed test runtime expected");
        };
        let request_headers =
            v3_wire::RequestHeadersRef::from_getter(|name| header_str(headers, name));
        AuthContext::from_v3(
            v3_wire::authenticate_request(request_headers, runtime.allowed_leaders.as_ref())
                .unwrap(),
        )
    }

    fn authenticate_v4_context(headers: &HeaderMap, runtime: &InvokeAuthRuntime) -> AuthContext {
        let InvokeAuthRuntime::Signed(runtime) = runtime else {
            panic!("signed test runtime expected");
        };
        let request_headers =
            v4_wire::RequestHeadersRef::from_getter(|name| header_str(headers, name));
        AuthContext::from_v4(
            v4_wire::authenticate_request(request_headers, runtime.allowed_leaders.as_ref())
                .unwrap(),
        )
    }

    async fn assert_signed_result_response(
        response: warp::reply::Response,
        authenticated: &AuthContext,
        expected_body: &[u8],
        tool_public_key: [u8; 32],
    ) {
        assert_eq!(response.status(), StatusCode::OK);
        let expected_version = match authenticated.protocol_version {
            SignedHttpVersion::V3 => SIGNATURE_VERSION_V3,
            SignedHttpVersion::V4 => SIGNATURE_VERSION_V4,
        };
        assert_eq!(
            response.headers()[HEADER_SIGNATURE_VERSION],
            expected_version
        );
        let tool_signature = response.headers()[HEADER_TOOL_SIGNATURE]
            .to_str()
            .unwrap()
            .to_owned();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), expected_body);
        match authenticated.protocol_version {
            SignedHttpVersion::V3 => verify_response_v3(
                ResponseHeadersRefV3 {
                    signature_version: Some(SIGNATURE_VERSION_V3),
                    tool_signature: Some(&tool_signature),
                },
                &authenticated.leader_signature,
                &authenticated.nonce,
                &body,
                tool_public_key,
            )
            .unwrap(),
            SignedHttpVersion::V4 => verify_response(
                ResponseHeadersRef {
                    signature_version: Some(SIGNATURE_VERSION_V4),
                    tool_signature: Some(&tool_signature),
                },
                &authenticated.leader_signature,
                &authenticated.nonce,
                &body,
                tool_public_key,
            )
            .unwrap(),
        };
    }

    fn cached_response(body: u8) -> CachedResponse {
        CachedResponse {
            status: StatusCode::OK,
            body: vec![body],
            is_result: true,
        }
    }

    #[test]
    fn inline_allowed_leaders_stays_stable_and_rejects_wrong_identity() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let allowed = AllowedLeaders::try_from(allowed_leaders_file("leader-id", 4, &leader))
            .expect("inline allowlist");
        let resolver = RefreshingAllowedLeadersResolver::new(allowed);

        assert_eq!(
            resolver.leader_public_key("leader-id", 4),
            Some(leader.verifying_key().to_bytes())
        );
        assert_eq!(resolver.leader_public_key("0xwallet-address", 4), None);

        let encoded = sign_request(
            "0xwallet-address",
            4,
            TEST_TOOL_ID,
            [1; 32],
            [3; 32],
            &leader,
        );
        let leader_key_id = encoded.leader_key_id.to_string();
        let error = authenticate_request_v4(
            RequestHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V4),
                leader_id: Some(&encoded.leader_id),
                leader_key_id: Some(&leader_key_id),
                tool_id: Some(&encoded.tool_id),
                input_hash: Some(&encoded.input_hash),
                leader_signature: Some(&encoded.leader_signature),
                nonce: Some(&encoded.nonce),
            },
            &resolver,
        )
        .expect_err("wallet identity must not substitute for registered leader ID");
        assert!(matches!(error, SignedHttpError::UnknownLeaderKey { .. }));
    }

    #[test]
    fn invoke_auth_runtime_requires_configuration_for_the_selected_tool() {
        let defaults = ToolkitRuntimeConfig::from_json_str("{}").unwrap();
        assert!(matches!(
            InvokeAuthRuntime::from_toolkit_config_for_tool_id(&defaults, "xyz.demo@1").unwrap(),
            InvokeAuthRuntime::Unsigned
        ));

        let leader = SigningKey::from_bytes(&[7; 32]);
        let config = serde_json::json!({
            "signed_http": {
                "mode": "required",
                "allowed_leaders": allowed_leaders_file("leader", 0, &leader),
                "tools": {
                    "xyz.demo@1": {
                        "registered_tool_id": "0x2a",
                        "response_signing_key": hex::encode([9u8; 32]),
                    }
                }
            }
        });
        let required = ToolkitRuntimeConfig::from_json_str(&config.to_string()).unwrap();
        assert!(
            InvokeAuthRuntime::from_toolkit_config_for_tool_id(&required, "xyz.missing@1")
                .err()
                .expect("missing Tool configuration must fail")
                .to_string()
                .contains("has no configuration")
        );
        let runtime =
            InvokeAuthRuntime::from_toolkit_config_for_tool_id(&required, "xyz.demo@1").unwrap();
        let InvokeAuthRuntime::Signed(runtime) = runtime else {
            panic!("configured Tool must use signed auth")
        };
        assert_eq!(
            runtime.replay_cache_ttl_ms,
            crate::config::DEFAULT_REPLAY_CACHE_TTL_MS
        );

        let invoke_auth = InvokeAuth::new_sync(
            Config::from_config(Arc::new(required)),
            "xyz.demo@1".to_string(),
            Duration::from_secs(90),
        )
        .unwrap();
        assert_eq!(invoke_auth.replay.in_flight_lease_ms, 90_000);

        let mut custom_json = config;
        custom_json["signed_http"]["tools"]["xyz.demo@1"]["replay_cache_ttl_ms"] =
            serde_json::json!(42);
        let custom = ToolkitRuntimeConfig::from_json_str(&custom_json.to_string()).unwrap();
        let InvokeAuthRuntime::Signed(runtime) =
            InvokeAuthRuntime::from_toolkit_config_for_tool_id(&custom, "xyz.demo@1").unwrap()
        else {
            panic!("configured Tool must use signed auth")
        };
        assert_eq!(runtime.replay_cache_ttl_ms, 42);

        let mut zero = custom_json;
        zero["signed_http"]["tools"]["xyz.demo@1"]["replay_cache_ttl_ms"] = serde_json::json!(0);
        assert!(ToolkitRuntimeConfig::from_json_str(&zero.to_string())
            .err()
            .expect("zero TTL must fail")
            .to_string()
            .contains("must be greater than zero"));
    }

    #[test]
    fn file_backed_allowed_leaders_reloads_without_restart() {
        let first = SigningKey::from_bytes(&[7; 32]);
        let second = SigningKey::from_bytes(&[8; 32]);
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("allowed-leaders.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&allowed_leaders_file("leader", 0, &first)).unwrap(),
        )
        .unwrap();
        let resolver = RefreshingAllowedLeadersResolver::new(
            AllowedLeaders::from_path(&path).expect("file allowlist"),
        );
        assert_eq!(
            resolver.leader_public_key("leader", 0),
            Some(first.verifying_key().to_bytes())
        );

        std::fs::write(
            &path,
            serde_json::to_vec(&allowed_leaders_file("leader", 0, &second)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            resolver.leader_public_key("leader", 0),
            Some(second.verifying_key().to_bytes())
        );
    }

    #[test]
    fn replay_cache_rejects_duplicate_while_in_flight() {
        let cache = ReplayCache::new(10);
        let nonce = [9; 32];
        let _reservation = match cache.begin(nonce, [1; 32], 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        assert!(matches!(
            cache.begin(nonce, [1; 32], 6),
            ReplayDecision::InFlight
        ));
        assert!(matches!(
            cache.begin(nonce, [2; 32], 6),
            ReplayDecision::InFlight
        ));
    }

    #[test]
    fn replay_cache_expired_lease_can_be_taken_over_without_stale_drop() {
        let cache = ReplayCache::new(10);
        let nonce = [9; 32];
        let stale = match cache.begin(nonce, [1; 32], 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        let active = match cache.begin(nonce, [1; 32], 16) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("expired lease must be replaceable"),
        };
        drop(stale);
        assert!(matches!(
            cache.begin(nonce, [1; 32], 17),
            ReplayDecision::InFlight
        ));
        drop(active);
    }

    #[test]
    fn replay_cache_dropped_reservation_releases_nonce() {
        let cache = ReplayCache::new(10);
        let nonce = [9; 32];
        let reservation = match cache.begin(nonce, [1; 32], 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        drop(reservation);
        assert!(matches!(
            cache.begin(nonce, [1; 32], 6),
            ReplayDecision::Proceed(_)
        ));
    }

    #[test]
    fn replay_cache_completed_response_expires_after_ttl() {
        let cache = ReplayCache::new(10);
        let nonce = [9; 32];
        let reservation = match cache.begin(nonce, [1; 32], 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        reservation.complete(cached_response(9), 10, 5);
        assert!(matches!(
            cache.begin(nonce, [1; 32], 14),
            ReplayDecision::Return(_)
        ));
        assert!(matches!(
            cache.begin(nonce, [1; 32], 16),
            ReplayDecision::Proceed(_)
        ));
    }

    #[test]
    fn replay_cache_replaces_completed_entry_when_input_hash_changes() {
        let cache = ReplayCache::new(10);
        let nonce = [9; 32];
        let reservation = match cache.begin(nonce, [1; 32], 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        reservation.complete(cached_response(8), 6, 100);
        assert!(matches!(
            cache.begin(nonce, [2; 32], 7),
            ReplayDecision::Proceed(_)
        ));
    }

    #[tokio::test]
    async fn signed_invoke_missing_headers_is_rejected() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let replay = ReplayCache::new(1_000);
        let response = handle_invoke(
            &signed_runtime(&leader, &tool),
            &replay,
            HeaderMap::new(),
            Vec::new(),
            |_ctx, _body| async { (StatusCode::OK, vec![1], true) },
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(HEADER_TOOL_SIGNATURE).is_none());
    }

    #[tokio::test]
    async fn verification_only_runtime_authenticates_request_without_signing_result() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let input_hash = [3; 32];
        let nonce = [1; 32];
        let replay = ReplayCache::new(1_000);
        let calls = Arc::new(AtomicUsize::new(0));

        for _ in 0..2 {
            let calls = Arc::clone(&calls);
            let response = handle_invoke(
                &verification_runtime(&leader),
                &replay,
                request_headers(&leader, input_hash, nonce),
                Vec::new(),
                move |context, _body| async move {
                    let context = context.expect("signed mode authenticates the Leader");
                    assert_eq!(context.input_hash, input_hash);
                    calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![4, 5, 6], true)
                },
            )
            .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[HEADER_SIGNATURE_VERSION],
                SIGNATURE_VERSION_V4
            );
            assert!(response.headers().get(HEADER_TOOL_SIGNATURE).is_none());
            assert_eq!(
                response.into_body().collect().await.unwrap().to_bytes(),
                vec![4, 5, 6]
            );
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn signed_invoke_in_flight_nonce_does_not_run_a_duplicate_callback() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let replay = ReplayCache::new(90_000);
        let nonce = [4; 32];
        let _active = match replay.begin(nonce, [1; 32], now_ms()) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        let calls = Arc::new(AtomicUsize::new(0));

        for input_hash in [[1; 32], [2; 32]] {
            let callback_calls = Arc::clone(&calls);
            let response = handle_invoke(
                &runtime,
                &replay,
                request_headers(&leader, input_hash, nonce),
                Vec::new(),
                move |_ctx, _body| async move {
                    callback_calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![1], true)
                },
            )
            .await;
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let body = response.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                json!({
                    "error": "request_in_flight",
                    "details": "request with the same nonce is still processing",
                })
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unsigned_result_is_returned_as_exact_bcs() {
        let body = vec![1, 2, 3];
        let replay = ReplayCache::new(1_000);
        let response = handle_invoke(
            &InvokeAuthRuntime::Unsigned,
            &replay,
            HeaderMap::new(),
            Vec::new(),
            |_ctx, _body| async { (StatusCode::OK, body.clone(), true) },
        )
        .await;
        assert_eq!(
            response.headers()["content-type"],
            CANONICAL_TOOL_RESPONSE_CONTENT_TYPE
        );
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            body
        );
    }

    #[tokio::test]
    async fn missing_unknown_and_v2_versions_are_rejected_without_handler_access() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let calls = Arc::new(AtomicUsize::new(0));

        for version in [None, Some("5"), Some("2")] {
            let mut headers = request_headers(&leader, [3; 32], [1; 32]);
            if let Some(version) = version {
                headers.insert(HEADER_SIGNATURE_VERSION, HeaderValue::from_static(version));
            } else {
                headers.remove(HEADER_SIGNATURE_VERSION);
            }
            let handler_calls = Arc::clone(&calls);
            let response = handle_invoke(
                &runtime,
                &ReplayCache::new(1_000),
                headers,
                Vec::new(),
                move |_ctx, _body| async move {
                    handler_calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![1, 2, 3], true)
                },
            )
            .await;

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert!(response.headers().get(HEADER_TOOL_SIGNATURE).is_none());
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn invalid_v4_signature_is_not_retried_as_v3() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let input_hash = [3; 32];
        let nonce = [1; 32];
        let mut encoded = sign_request("leader", 0, TEST_TOOL_ID, nonce, input_hash, &leader);
        encoded.leader_signature =
            sign_request_v3("leader", 0, input_hash, nonce, &leader).leader_signature;
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let response = handle_invoke(
            &runtime,
            &ReplayCache::new(1_000),
            headers_from_encoded(encoded),
            Vec::new(),
            move |_ctx, _body| async move {
                handler_calls.fetch_add(1, Ordering::SeqCst);
                (StatusCode::OK, vec![1, 2, 3], true)
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn allowed_leader_check_applies_to_both_versions() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let unlisted_leader = SigningKey::from_bytes(&[8; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let input_hash = [3; 32];
        let nonce = [1; 32];
        let v3_headers = headers_from_pairs(
            sign_request_v3("leader", 0, input_hash, nonce, &unlisted_leader).to_pairs(),
        );
        let v4_headers = headers_from_encoded(sign_request(
            "leader",
            0,
            TEST_TOOL_ID,
            nonce,
            input_hash,
            &unlisted_leader,
        ));
        let calls = Arc::new(AtomicUsize::new(0));

        for headers in [v3_headers, v4_headers] {
            let handler_calls = Arc::clone(&calls);
            let response = handle_invoke(
                &runtime,
                &ReplayCache::new(1_000),
                headers,
                Vec::new(),
                move |_ctx, _body| async move {
                    handler_calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![1, 2, 3], true)
                },
            )
            .await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn wrong_tool_id_is_rejected_before_replay_lookup() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let replay = ReplayCache::new(1_000);
        let calls = Arc::new(AtomicUsize::new(0));

        let first_calls = Arc::clone(&calls);
        let accepted = handle_invoke(
            &runtime,
            &replay,
            request_headers(&leader, [3; 32], [1; 32]),
            Vec::new(),
            move |_ctx, _body| async move {
                first_calls.fetch_add(1, Ordering::SeqCst);
                (StatusCode::OK, vec![1, 2, 3], true)
            },
        )
        .await;
        assert_eq!(accepted.status(), StatusCode::OK);

        let mismatched_headers = headers_from_encoded(sign_request(
            "leader", 0, [99; 32], [1; 32], [3; 32], &leader,
        ));
        let rejected_calls = Arc::clone(&calls);
        let rejected = handle_invoke(
            &runtime,
            &replay,
            mismatched_headers,
            Vec::new(),
            move |_ctx, _body| async move {
                rejected_calls.fetch_add(1, Ordering::SeqCst);
                (StatusCode::OK, vec![4, 5, 6], true)
            },
        )
        .await;

        assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn explicit_v3_request_returns_v3_signed_response_without_request_tool_id() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime_with_tool_id(&leader, &tool, TEST_TOOL_ID);
        let replay = ReplayCache::new(1_000);
        let calls = Arc::new(AtomicUsize::new(0));
        let input_hash = [3; 32];
        let nonce = [1; 32];
        let headers = request_headers_v3(&leader, input_hash, nonce);
        let authenticated = authenticate_v3_context(&headers, &runtime);

        let rejected_calls = Arc::clone(&calls);
        let response = handle_invoke(
            &runtime,
            &replay,
            headers,
            Vec::new(),
            move |ctx, _body| async move {
                rejected_calls.fetch_add(1, Ordering::SeqCst);
                let ctx = ctx.unwrap();
                assert_eq!(ctx.protocol_version, SignedHttpVersion::V3);
                assert_eq!(ctx.tool_id, None);
                assert_eq!(ctx.input_hash, input_hash);
                (StatusCode::OK, vec![1, 2, 3], true)
            },
        )
        .await;

        assert_signed_result_response(
            response,
            &authenticated,
            &[1, 2, 3],
            tool.verifying_key().to_bytes(),
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn v4_tool_id_mismatch_is_rejected_before_cached_result_access() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime_with_tool_id(&leader, &tool, TEST_TOOL_ID);
        let replay = ReplayCache::new(1_000);
        let calls = Arc::new(AtomicUsize::new(0));
        let input_hash = [3; 32];
        let nonce = [1; 32];
        let v3_headers = request_headers_v3(&leader, input_hash, nonce);
        let authenticated_v3 = authenticate_v3_context(&v3_headers, &runtime);
        let first_calls = Arc::clone(&calls);
        let first = handle_invoke(
            &runtime,
            &replay,
            v3_headers,
            Vec::new(),
            move |ctx, _body| async move {
                first_calls.fetch_add(1, Ordering::SeqCst);
                assert_eq!(ctx.unwrap().protocol_version, SignedHttpVersion::V3);
                (StatusCode::OK, vec![1, 2, 3], true)
            },
        )
        .await;
        assert_signed_result_response(
            first,
            &authenticated_v3,
            &[1, 2, 3],
            tool.verifying_key().to_bytes(),
        )
        .await;

        let second_calls = Arc::clone(&calls);
        let second = handle_invoke(
            &runtime,
            &replay,
            headers_from_encoded(sign_request(
                "leader", 0, [2; 32], nonce, input_hash, &leader,
            )),
            Vec::new(),
            move |_ctx, _body| async move {
                second_calls.fetch_add(1, Ordering::SeqCst);
                (StatusCode::OK, vec![4, 5, 6], true)
            },
        )
        .await;

        assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
        assert!(second.headers().get(HEADER_SIGNATURE_VERSION).is_none());
        assert!(second.headers().get(HEADER_TOOL_SIGNATURE).is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn signed_result_uses_exact_body_and_tool_signature() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let input_hash = [3; 32];
        let nonce = [1; 32];
        let request = sign_request("leader", 0, TEST_TOOL_ID, nonce, input_hash, &leader);
        let leader_signature = authenticate_request_v4(
            RequestHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V4),
                leader_id: Some("leader"),
                leader_key_id: Some("0"),
                tool_id: Some(&request.tool_id),
                input_hash: Some(&request.input_hash),
                leader_signature: Some(&request.leader_signature),
                nonce: Some(&request.nonce),
            },
            match &signed_runtime(&leader, &tool) {
                InvokeAuthRuntime::Signed(runtime) => runtime.allowed_leaders.as_ref(),
                InvokeAuthRuntime::Unsigned => unreachable!(),
            },
        )
        .unwrap()
        .leader_signature;
        let result = vec![4, 5, 6];
        let replay = ReplayCache::new(1_000);
        let response = handle_invoke(
            &signed_runtime(&leader, &tool),
            &replay,
            request_headers(&leader, input_hash, nonce),
            Vec::new(),
            |ctx, _body| async move {
                assert_eq!(ctx.unwrap().input_hash, input_hash);
                (StatusCode::OK, result.clone(), true)
            },
        )
        .await;
        let tool_signature = response.headers()[HEADER_TOOL_SIGNATURE]
            .to_str()
            .unwrap()
            .to_owned();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V4),
                tool_signature: Some(&tool_signature),
            },
            &leader_signature,
            &nonce,
            &body,
            tool.verifying_key().to_bytes(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn exact_retry_uses_cache_and_changed_completed_input_reruns() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let replay = ReplayCache::new(1_000);
        let calls = Arc::new(AtomicUsize::new(0));

        for _ in 0..2 {
            let calls = Arc::clone(&calls);
            let response = handle_invoke(
                &runtime,
                &replay,
                request_headers(&leader, [1; 32], [1; 32]),
                Vec::new(),
                move |_ctx, _body| async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![8], true)
                },
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let changed_calls = Arc::clone(&calls);
        let response = handle_invoke(
            &runtime,
            &replay,
            request_headers(&leader, [2; 32], [1; 32]),
            Vec::new(),
            move |_ctx, _body| async move {
                changed_calls.fetch_add(1, Ordering::SeqCst);
                (StatusCode::OK, vec![9], true)
            },
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(HEADER_SIGNATURE_VERSION).is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn mixed_version_cache_hits_return_current_version_signatures() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let input_hash = [3; 32];
        let nonce = [1; 32];

        for first_version in [SignedHttpVersion::V3, SignedHttpVersion::V4] {
            let replay = ReplayCache::new(1_000);
            let calls = Arc::new(AtomicUsize::new(0));
            let v3_headers = request_headers_v3(&leader, input_hash, nonce);
            let v4_headers = request_headers(&leader, input_hash, nonce);
            let v3_context = authenticate_v3_context(&v3_headers, &runtime);
            let v4_context = authenticate_v4_context(&v4_headers, &runtime);
            let (first_headers, first_context, second_headers, second_context) =
                if first_version == SignedHttpVersion::V3 {
                    (v3_headers, v3_context, v4_headers, v4_context)
                } else {
                    (v4_headers, v4_context, v3_headers, v3_context)
                };

            let first_calls = Arc::clone(&calls);
            let first = handle_invoke(
                &runtime,
                &replay,
                first_headers,
                Vec::new(),
                move |_ctx, _body| async move {
                    first_calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![4, 5, 6], true)
                },
            )
            .await;
            assert_signed_result_response(
                first,
                &first_context,
                &[4, 5, 6],
                tool.verifying_key().to_bytes(),
            )
            .await;

            let second_calls = Arc::clone(&calls);
            let second = handle_invoke(
                &runtime,
                &replay,
                second_headers,
                Vec::new(),
                move |_ctx, _body| async move {
                    second_calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![7, 8, 9], true)
                },
            )
            .await;
            assert_signed_result_response(
                second,
                &second_context,
                &[4, 5, 6],
                tool.verifying_key().to_bytes(),
            )
            .await;
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn completed_tool_errors_are_cached_without_a_signature() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let replay = ReplayCache::new(1_000);
        let calls = Arc::new(AtomicUsize::new(0));

        for _ in 0..2 {
            let calls = Arc::clone(&calls);
            let response = handle_invoke(
                &runtime,
                &replay,
                request_headers(&leader, [1; 32], [5; 32]),
                Vec::new(),
                move |_ctx, _body| async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (
                        StatusCode::BAD_REQUEST,
                        br#"{"error":"bad input"}"#.to_vec(),
                        false,
                    )
                },
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert!(response.headers().get(HEADER_TOOL_SIGNATURE).is_none());
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn runtime_reload_preserves_cached_body_and_resigns_with_current_keys() {
        let leader_a = SigningKey::from_bytes(&[7; 32]);
        let leader_b = SigningKey::from_bytes(&[8; 32]);
        let tool_a = SigningKey::from_bytes(&[9; 32]);
        let tool_b = SigningKey::from_bytes(&[10; 32]);
        let resolver = Arc::new(RefreshingAllowedLeadersResolver::new(
            AllowedLeaders::try_from(AllowedLeadersFileV1 {
                version: 1,
                leaders: vec![
                    allowed_leader("leader-a", 0, &leader_a),
                    allowed_leader("leader-b", 1, &leader_b),
                ],
            })
            .unwrap(),
        ));
        let runtime_a = InvokeAuthRuntime::Signed(Box::new(SignedInvokeRuntime {
            registered_tool_id: TEST_TOOL_ID,
            response_signing_key: Some(tool_a.clone()),
            allowed_leaders: Arc::clone(&resolver),
            replay_cache_ttl_ms: 10_000,
        }));
        let runtime_b = InvokeAuthRuntime::Signed(Box::new(SignedInvokeRuntime {
            registered_tool_id: TEST_TOOL_ID,
            response_signing_key: Some(tool_b.clone()),
            allowed_leaders: Arc::clone(&resolver),
            replay_cache_ttl_ms: 20_000,
        }));
        let replay = ReplayCache::new(1_000);
        let nonce = [5; 32];
        let input_hash = [3; 32];
        let calls = Arc::new(AtomicUsize::new(0));

        let request_a = sign_request("leader-a", 0, TEST_TOOL_ID, nonce, input_hash, &leader_a);
        let authenticated_a = authenticate_request_v4(
            RequestHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V4),
                leader_id: Some(&request_a.leader_id),
                leader_key_id: Some("0"),
                tool_id: Some(&request_a.tool_id),
                input_hash: Some(&request_a.input_hash),
                leader_signature: Some(&request_a.leader_signature),
                nonce: Some(&request_a.nonce),
            },
            resolver.as_ref(),
        )
        .unwrap();
        let response_a = handle_invoke(
            &runtime_a,
            &replay,
            headers_from_encoded(request_a),
            Vec::new(),
            {
                let calls = Arc::clone(&calls);
                move |_ctx, _body| async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, vec![4, 5, 6], true)
                }
            },
        )
        .await;
        let signature_a = response_a.headers()[HEADER_TOOL_SIGNATURE]
            .to_str()
            .unwrap()
            .to_owned();
        verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V4),
                tool_signature: Some(&signature_a),
            },
            &authenticated_a.leader_signature,
            &nonce,
            &[4, 5, 6],
            tool_a.verifying_key().to_bytes(),
        )
        .unwrap();

        let request_b = sign_request("leader-b", 1, TEST_TOOL_ID, nonce, input_hash, &leader_b);
        let authenticated_b = authenticate_request_v4(
            RequestHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V4),
                leader_id: Some(&request_b.leader_id),
                leader_key_id: Some("1"),
                tool_id: Some(&request_b.tool_id),
                input_hash: Some(&request_b.input_hash),
                leader_signature: Some(&request_b.leader_signature),
                nonce: Some(&request_b.nonce),
            },
            resolver.as_ref(),
        )
        .unwrap();
        let response_b = handle_invoke(
            &runtime_b,
            &replay,
            headers_from_encoded(request_b),
            Vec::new(),
            |_ctx, _body| async { panic!("a completed replay entry must not rerun the Tool") },
        )
        .await;
        let signature_b = response_b.headers()[HEADER_TOOL_SIGNATURE]
            .to_str()
            .unwrap()
            .to_owned();
        verify_response(
            ResponseHeadersRef {
                signature_version: Some(SIGNATURE_VERSION_V4),
                tool_signature: Some(&signature_b),
            },
            &authenticated_b.leader_signature,
            &nonce,
            &[4, 5, 6],
            tool_b.verifying_key().to_bytes(),
        )
        .unwrap();

        assert_ne!(signature_a, signature_b);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
