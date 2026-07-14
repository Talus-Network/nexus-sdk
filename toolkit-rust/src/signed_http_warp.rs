//! Warp integration for the minimal signed Tool transport.

use {
    crate::{config::Config, AuthContext, ToolkitRuntimeConfig},
    ed25519_dalek::SigningKey,
    nexus_sdk::signed_http::v2::{
        error::SignedHttpError,
        wire::{
            authenticate_request,
            sign_response,
            AllowedLeaders,
            EncodedResponseHeaders,
            LeaderKeyResolver,
            RequestHeadersRef,
        },
    },
    serde_json::json,
    std::{
        collections::HashMap,
        future::Future,
        sync::{Arc, Mutex, RwLock},
        time::{SystemTime, UNIX_EPOCH},
    },
    warp::http::{header::HeaderValue, HeaderMap, StatusCode},
};

const REPLAY_CACHE_TTL_MS: u64 = 300_000;
const IN_FLIGHT_LEASE_MS: u64 = 60_000;
const JSON_CONTENT_TYPE: &str = "application/json";
const TAGGED_OUTPUT_CONTENT_TYPE: &str = "application/vnd.nexus.tagged-output+bcs";

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

impl LeaderKeyResolver for RefreshingAllowedLeadersResolver {
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
    signing_key: SigningKey,
    allowed_leaders: Arc<RefreshingAllowedLeadersResolver>,
    replay: ReplayCache,
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
                "signed_http is enabled but no signing key is configured for tool_id='{tool_id}' (config={source})"
            )
        })?;

        Ok(Self::Signed(Box::new(SignedInvokeRuntime {
            signing_key: tool.tool_signing_key.clone(),
            allowed_leaders: Arc::new(RefreshingAllowedLeadersResolver::new(
                (*signed_http.allowed_leaders).clone(),
            )),
            replay: ReplayCache::new(REPLAY_CACHE_TTL_MS, IN_FLIGHT_LEASE_MS),
        })))
    }
}

#[derive(Clone)]
pub(crate) struct InvokeAuth {
    state: Arc<RwLock<InvokeAuthState>>,
    tool_id: String,
    config: Arc<Config>,
}

struct InvokeAuthState {
    auth: Arc<InvokeAuthRuntime>,
    config_ptr: usize,
}

impl InvokeAuth {
    pub(crate) fn new_sync(config: Arc<Config>, tool_id: String) -> anyhow::Result<Self> {
        let current_config = config.current();
        let auth = InvokeAuthRuntime::from_toolkit_config_for_tool_id(&current_config, &tool_id)?;
        let config_ptr = Arc::as_ptr(&current_config) as usize;
        Ok(Self {
            state: Arc::new(RwLock::new(InvokeAuthState {
                auth: Arc::new(auth),
                config_ptr,
            })),
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
}

/// Handle one `/invoke` request.
///
/// The callback's boolean marks whether its body is a canonical BCS `TaggedOutput`. Only those
/// result bodies are signed; local HTTP errors remain JSON and never become verifier evidence.
pub async fn handle_invoke<F, Fut>(
    auth: &InvokeAuthRuntime,
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
            let request_headers = RequestHeadersRef::from_getter(|name| header_str(&headers, name));
            let authenticated =
                match authenticate_request(request_headers, runtime.allowed_leaders.as_ref()) {
                    Ok(authenticated) => authenticated,
                    Err(error) => return auth_failed(error),
                };
            let identity = ReplayIdentity::from(&authenticated);
            match runtime
                .replay
                .begin(&authenticated.nonce, identity, now_ms())
            {
                ReplayDecision::Return(cached) => cached.into_response(),
                ReplayDecision::Conflict => json_response(
                    StatusCode::UNAUTHORIZED,
                    json!({
                        "error": "replay_rejected",
                        "details": "nonce already used with a different signed input hash",
                    }),
                ),
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
                    let signature = is_result.then(|| {
                        sign_response(&authenticated.leader_signature, &body, &runtime.signing_key)
                    });
                    let cached = CachedResponse {
                        status,
                        body,
                        is_result,
                        signature,
                    };
                    reservation.complete(cached.clone(), now_ms());
                    cached.into_response()
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReplayIdentity {
    leader_id: String,
    leader_key_id: u64,
    input_hash: [u8; 32],
    leader_signature: [u8; 64],
}

impl From<&AuthContext> for ReplayIdentity {
    fn from(value: &AuthContext) -> Self {
        Self {
            leader_id: value.leader_id.clone(),
            leader_key_id: value.leader_key_id,
            input_hash: value.input_hash,
            leader_signature: value.leader_signature,
        }
    }
}

#[derive(Clone)]
struct CachedResponse {
    status: StatusCode,
    body: Vec<u8>,
    is_result: bool,
    signature: Option<EncodedResponseHeaders>,
}

impl CachedResponse {
    fn into_response(self) -> warp::reply::Response {
        response(
            self.status,
            self.body,
            self.is_result,
            self.signature.as_ref(),
        )
    }
}

#[derive(Clone)]
struct ReplayCache {
    inner: Arc<Mutex<HashMap<String, ReplayEntry>>>,
    completed_ttl_ms: u64,
    in_flight_lease_ms: u64,
}

#[derive(Clone)]
struct ReplayEntry {
    identity: ReplayIdentity,
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
    Conflict,
    InFlight,
}

struct ReplayReservation {
    cache: ReplayCache,
    nonce: String,
    identity: ReplayIdentity,
    lease_expires_at_ms: u64,
    completed: bool,
}

impl ReplayCache {
    fn new(completed_ttl_ms: u64, in_flight_lease_ms: u64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            completed_ttl_ms,
            in_flight_lease_ms,
        }
    }

    fn begin(&self, nonce: &str, identity: ReplayIdentity, now_ms: u64) -> ReplayDecision {
        let mut entries = self.inner.lock().unwrap();
        entries.retain(|_, entry| entry.expires_at_ms >= now_ms);
        match entries.get(nonce) {
            Some(entry) if entry.identity != identity => ReplayDecision::Conflict,
            Some(ReplayEntry {
                state: ReplayState::Complete(response),
                ..
            }) => ReplayDecision::Return(response.clone()),
            Some(_) => ReplayDecision::InFlight,
            None => {
                let lease_expires_at_ms = now_ms.saturating_add(self.in_flight_lease_ms);
                entries.insert(
                    nonce.to_string(),
                    ReplayEntry {
                        identity: identity.clone(),
                        expires_at_ms: lease_expires_at_ms,
                        state: ReplayState::InFlight,
                    },
                );
                ReplayDecision::Proceed(ReplayReservation {
                    cache: self.clone(),
                    nonce: nonce.to_string(),
                    identity,
                    lease_expires_at_ms,
                    completed: false,
                })
            }
        }
    }
}

impl ReplayReservation {
    fn complete(mut self, response: CachedResponse, now_ms: u64) {
        let mut entries = self.cache.inner.lock().unwrap();
        if let Some(entry) = entries.get_mut(&self.nonce) {
            if entry.identity == self.identity
                && entry.expires_at_ms == self.lease_expires_at_ms
                && matches!(entry.state, ReplayState::InFlight)
            {
                entry.state = ReplayState::Complete(response);
                entry.expires_at_ms = now_ms.saturating_add(self.cache.completed_ttl_ms);
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
        if entries.get(&self.nonce).is_some_and(|entry| {
            entry.identity == self.identity
                && entry.expires_at_ms == self.lease_expires_at_ms
                && matches!(entry.state, ReplayState::InFlight)
        }) {
            entries.remove(&self.nonce);
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
    signature: Option<&EncodedResponseHeaders>,
) -> warp::reply::Response {
    let mut response = warp::reply::Response::new(warp::hyper::Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static(if is_result {
            TAGGED_OUTPUT_CONTENT_TYPE
        } else {
            JSON_CONTENT_TYPE
        }),
    );
    if let Some(signature) = signature {
        for (name, value) in signature.to_pairs() {
            if let Ok(value) = HeaderValue::from_str(&value) {
                response.headers_mut().insert(name, value);
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

fn auth_failed(error: SignedHttpError) -> warp::reply::Response {
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
        nexus_sdk::signed_http::v2::wire::{
            sign_request,
            verify_response,
            AllowedLeaderFileV1,
            AllowedLeaderKeyFileV1,
            AllowedLeadersFileV1,
            ResponseHeadersRef,
            HEADER_SIGNATURE_VERSION,
            HEADER_TOOL_SIGNATURE,
        },
        std::sync::atomic::{AtomicUsize, Ordering},
        warp::hyper::body::to_bytes,
    };

    fn allowed_leaders_file(leader_id: &str, kid: u64, key: &SigningKey) -> AllowedLeadersFileV1 {
        AllowedLeadersFileV1 {
            version: 1,
            leaders: vec![AllowedLeaderFileV1 {
                leader_id: leader_id.to_string(),
                keys: vec![AllowedLeaderKeyFileV1 {
                    kid,
                    public_key: hex::encode(key.verifying_key().to_bytes()),
                }],
            }],
        }
    }

    fn signed_runtime(leader: &SigningKey, tool: &SigningKey) -> InvokeAuthRuntime {
        let allowed = AllowedLeaders::try_from(allowed_leaders_file("leader", 0, leader)).unwrap();
        InvokeAuthRuntime::Signed(Box::new(SignedInvokeRuntime {
            signing_key: tool.clone(),
            allowed_leaders: Arc::new(RefreshingAllowedLeadersResolver::new(allowed)),
            replay: ReplayCache::new(10_000, 1_000),
        }))
    }

    fn request_headers(leader: &SigningKey, input_hash: [u8; 32], nonce: &str) -> HeaderMap {
        let encoded = sign_request("leader", 0, input_hash, nonce, leader);
        let mut headers = HeaderMap::new();
        for (name, value) in encoded.to_pairs() {
            headers.insert(name, HeaderValue::from_str(&value).unwrap());
        }
        headers
    }

    fn replay_identity(leader_id: &str, input_hash: [u8; 32]) -> ReplayIdentity {
        ReplayIdentity {
            leader_id: leader_id.to_string(),
            leader_key_id: 0,
            input_hash,
            leader_signature: [7; 64],
        }
    }

    fn cached_response(body: u8) -> CachedResponse {
        CachedResponse {
            status: StatusCode::OK,
            body: vec![body],
            is_result: true,
            signature: None,
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

        let encoded = sign_request("0xwallet-address", 4, [3; 32], "nonce", &leader);
        let leader_key_id = encoded.leader_key_id.to_string();
        let error = authenticate_request(
            RequestHeadersRef {
                signature_version: Some("2"),
                leader_id: Some(&encoded.leader_id),
                leader_key_id: Some(&leader_key_id),
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
        let cache = ReplayCache::new(100, 10);
        let identity = replay_identity("leader", [1; 32]);
        let _reservation = match cache.begin("nonce", identity.clone(), 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        assert!(matches!(
            cache.begin("nonce", identity, 6),
            ReplayDecision::InFlight
        ));
    }

    #[test]
    fn replay_cache_expired_lease_can_be_taken_over_without_stale_drop() {
        let cache = ReplayCache::new(100, 10);
        let identity = replay_identity("leader", [1; 32]);
        let stale = match cache.begin("nonce", identity.clone(), 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        let active = match cache.begin("nonce", identity.clone(), 16) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("expired lease must be replaceable"),
        };
        drop(stale);
        assert!(matches!(
            cache.begin("nonce", identity, 17),
            ReplayDecision::InFlight
        ));
        drop(active);
    }

    #[test]
    fn replay_cache_dropped_reservation_releases_nonce() {
        let cache = ReplayCache::new(100, 10);
        let identity = replay_identity("leader", [1; 32]);
        let reservation = match cache.begin("nonce", identity.clone(), 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        drop(reservation);
        assert!(matches!(
            cache.begin("nonce", identity, 6),
            ReplayDecision::Proceed(_)
        ));
    }

    #[test]
    fn replay_cache_completed_response_expires_after_ttl() {
        let cache = ReplayCache::new(5, 10);
        let identity = replay_identity("leader", [1; 32]);
        let reservation = match cache.begin("nonce", identity.clone(), 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        reservation.complete(cached_response(9), 10);
        assert!(matches!(
            cache.begin("nonce", identity.clone(), 14),
            ReplayDecision::Return(_)
        ));
        assert!(matches!(
            cache.begin("nonce", identity, 16),
            ReplayDecision::Proceed(_)
        ));
    }

    #[test]
    fn replay_cache_conflicts_across_leader_and_input_identity() {
        let cache = ReplayCache::new(100, 10);
        let identity = replay_identity("leader-a", [1; 32]);
        let _reservation = match cache.begin("nonce", identity, 5) {
            ReplayDecision::Proceed(reservation) => reservation,
            _ => panic!("first request must reserve the nonce"),
        };
        assert!(matches!(
            cache.begin("nonce", replay_identity("leader-b", [1; 32]), 6),
            ReplayDecision::Conflict
        ));
        assert!(matches!(
            cache.begin("nonce", replay_identity("leader-a", [2; 32]), 6),
            ReplayDecision::Conflict
        ));
    }

    #[tokio::test]
    async fn signed_invoke_missing_headers_is_rejected() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let response = handle_invoke(
            &signed_runtime(&leader, &tool),
            HeaderMap::new(),
            Vec::new(),
            |_ctx, _body| async { (StatusCode::OK, vec![1], true) },
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(HEADER_TOOL_SIGNATURE).is_none());
    }

    #[tokio::test]
    async fn unsigned_result_is_returned_as_exact_bcs() {
        let body = vec![1, 2, 3];
        let response = handle_invoke(
            &InvokeAuthRuntime::Unsigned,
            HeaderMap::new(),
            Vec::new(),
            |_ctx, _body| async { (StatusCode::OK, body.clone(), true) },
        )
        .await;
        assert_eq!(
            response.headers()["content-type"],
            TAGGED_OUTPUT_CONTENT_TYPE
        );
        assert_eq!(to_bytes(response.into_body()).await.unwrap(), body);
    }

    #[tokio::test]
    async fn signed_result_uses_exact_body_and_tool_signature() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let input_hash = [3; 32];
        let request = sign_request("leader", 0, input_hash, "nonce", &leader);
        let leader_signature = nexus_sdk::signed_http::v2::wire::authenticate_request(
            RequestHeadersRef {
                signature_version: Some("2"),
                leader_id: Some("leader"),
                leader_key_id: Some("0"),
                input_hash: Some(&request.input_hash),
                leader_signature: Some(&request.leader_signature),
                nonce: Some("nonce"),
            },
            match &signed_runtime(&leader, &tool) {
                InvokeAuthRuntime::Signed(runtime) => runtime.allowed_leaders.as_ref(),
                InvokeAuthRuntime::Unsigned => unreachable!(),
            },
        )
        .unwrap()
        .leader_signature;
        let result = vec![4, 5, 6];
        let response = handle_invoke(
            &signed_runtime(&leader, &tool),
            request_headers(&leader, input_hash, "nonce"),
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
        let body = to_bytes(response.into_body()).await.unwrap();
        verify_response(
            ResponseHeadersRef {
                signature_version: Some("2"),
                tool_signature: Some(&tool_signature),
            },
            &leader_signature,
            &body,
            tool.verifying_key().to_bytes(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn exact_retry_returns_cache_and_conflicting_nonce_is_rejected() {
        let leader = SigningKey::from_bytes(&[7; 32]);
        let tool = SigningKey::from_bytes(&[9; 32]);
        let runtime = signed_runtime(&leader, &tool);
        let calls = Arc::new(AtomicUsize::new(0));

        for _ in 0..2 {
            let calls = Arc::clone(&calls);
            let response = handle_invoke(
                &runtime,
                request_headers(&leader, [1; 32], "nonce"),
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

        let response = handle_invoke(
            &runtime,
            request_headers(&leader, [2; 32], "nonce"),
            Vec::new(),
            |_ctx, _body| async { (StatusCode::OK, vec![9], true) },
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(HEADER_SIGNATURE_VERSION).is_none());
    }
}
