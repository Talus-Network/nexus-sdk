//! Warp integration for signed HTTP (`/invoke`).
//!
//! `nexus-sdk` implements the signed HTTP protocol itself (claims, signing, verification, replay
//! storage). This module is the glue between that protocol and `warp`'s request/response types.
//!
//! Most tool developers should use [`crate::bootstrap!`] and not interact with this module.

use {
    crate::{config::Config, AuthContext, ToolkitRuntimeConfig},
    nexus_sdk::signed_http::v1::{
        engine::{
            ResponderDecisionV1,
            ResponderRejectionKindV1,
            SignatureHeadersRef,
            SignedHttpEngineV1,
            SignedHttpPolicyV1,
            SignedHttpResponderV1,
            SignedResponseV1,
        },
        error::SignedHttpError,
        wire::HttpRequestMeta,
    },
    serde_json::json,
    std::{
        future::Future,
        sync::{Arc, RwLock},
    },
    warp::http::{header::HeaderValue, HeaderMap, StatusCode},
};

/// Auth mode for `/invoke` handling.
///
/// In unsigned mode, the handler executes the tool and returns a plain JSON response.
/// In signed mode, the handler authenticates requests, enforces replay rules, and signs responses.
#[derive(Clone)]
pub enum InvokeAuthRuntime {
    /// Signed HTTP disabled (tool accepts unsigned requests).
    Unsigned,
    /// Signed HTTP enabled and required (tool rejects unsigned requests).
    Signed(Box<SignedHttpResponderV1>),
}

impl InvokeAuthRuntime {
    /// Build the per-tool auth runtime from [`ToolkitRuntimeConfig`].
    pub fn from_toolkit_config_for_tool_id(
        toolkit_cfg: &ToolkitRuntimeConfig,
        tool_id: &str,
    ) -> anyhow::Result<Self> {
        let Some(signed_http) = toolkit_cfg.signed_http() else {
            return Ok(Self::Unsigned);
        };

        let tool = signed_http.tools.get(tool_id).ok_or_else(|| {
            let src = toolkit_cfg
                .source_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<defaults>".to_string());
            anyhow::anyhow!(
                "signed_http is enabled but no signing key is configured for tool_id='{tool_id}' (config={src})"
            )
        })?;

        let engine = SignedHttpEngineV1::new(SignedHttpPolicyV1 {
            max_clock_skew_ms: signed_http.max_clock_skew_ms,
            max_validity_ms: signed_http.max_validity_ms,
        });

        Ok(Self::Signed(Box::new(
            engine.responder_with_in_memory_replay(
                tool_id.to_string(),
                tool.tool_kid,
                tool.tool_signing_key.clone(),
                signed_http.allowed_leaders.clone(),
            ),
        )))
    }
}

/// Internal auth runtime with hot-reload support.
///
/// Used by bootstrap! macro. Automatically updates auth runtime when config changes.
#[derive(Clone)]
pub(crate) struct InvokeAuth {
    state: Arc<RwLock<InvokeAuthState>>,
    tool_id: String,
    config: Arc<Config>,
}

/// Internal state for InvokeAuth, tracking both the runtime and which config version built it.
struct InvokeAuthState {
    auth: InvokeAuthRuntime,
    /// Pointer to the config that built this auth, used to detect config changes.
    config_ptr: usize,
}

impl InvokeAuth {
    /// Create a new auth runtime with hot-reload support (sync version).
    pub(crate) fn new_sync(config: Arc<Config>, tool_id: String) -> anyhow::Result<Self> {
        let current_config = config.current();
        let auth = InvokeAuthRuntime::from_toolkit_config_for_tool_id(&current_config, &tool_id)?;
        let config_ptr = Arc::as_ptr(&current_config) as usize;

        Ok(Self {
            state: Arc::new(RwLock::new(InvokeAuthState { auth, config_ptr })),
            tool_id,
            config,
        })
    }

    /// Get the current auth runtime, reloading from config if needed.
    pub(crate) async fn current(&self) -> InvokeAuthRuntime {
        let current_config = self.config.current();
        let current_ptr = Arc::as_ptr(&current_config) as usize;

        // Check if config has changed by comparing Arc pointers
        {
            let guard = self.state.read().unwrap();
            if guard.config_ptr == current_ptr {
                return guard.auth.clone();
            }
        }

        // Config changed, rebuild auth runtime
        let mut guard = self.state.write().unwrap();
        // Double-check after acquiring write lock
        if guard.config_ptr != current_ptr {
            if let Ok(new_auth) =
                InvokeAuthRuntime::from_toolkit_config_for_tool_id(&current_config, &self.tool_id)
            {
                guard.auth = new_auth;
                guard.config_ptr = current_ptr;
            }
        }
        guard.auth.clone()
    }
}

/// Handle a single `/invoke` request using the configured auth mode.
///
/// The `run` callback is responsible for executing the tool pipeline and returning an HTTP status
/// and raw JSON bytes. This module handles:
/// - signed HTTP authentication + replay rules (when enabled), and
/// - signed response production (including post-auth error responses).
pub async fn handle_invoke<F, Fut>(
    auth: InvokeAuthRuntime,
    http: HttpRequestMeta<'_>,
    headers: HeaderMap,
    body_bytes: Vec<u8>,
    run: F,
) -> warp::reply::Response
where
    F: FnOnce(Option<AuthContext>, Vec<u8>) -> Fut,
    Fut: Future<Output = (StatusCode, Vec<u8>)> + Send,
{
    match auth {
        InvokeAuthRuntime::Unsigned => {
            let (status, body) = run(None, body_bytes).await;
            json_bytes(status, body)
        }
        InvokeAuthRuntime::Signed(responder) => {
            let sig_headers = SignatureHeadersRef::from_getter(|name| header_str(&headers, name));

            let decision = match responder.authenticate_invoke(http, &body_bytes, sig_headers) {
                Ok(d) => d,
                Err(e) => return auth_failed(e),
            };

            match decision {
                ResponderDecisionV1::Return(resp) => signed_response(resp),
                ResponderDecisionV1::Reject(rej) => {
                    let (status, body) = match rej.kind {
                        ResponderRejectionKindV1::ReplayConflict => json_bytes_or_fallback(
                            StatusCode::UNAUTHORIZED,
                            json!({
                                "error": "replay_rejected",
                                "details": "nonce already used with different request",
                            }),
                        ),
                        ResponderRejectionKindV1::InFlight => json_bytes_or_fallback(
                            StatusCode::CONFLICT,
                            json!({
                                "error": "request_in_flight",
                                "details": "request with same nonce is still processing",
                            }),
                        ),
                    };

                    match rej.sign_response(status.as_u16(), &body) {
                        Ok(resp) => signed_response(resp),
                        Err(e) => json_value(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "response_signing_error",
                                "details": e.to_string(),
                            }),
                        ),
                    }
                }
                ResponderDecisionV1::Proceed(session) => {
                    let (status, body) = run(Some(session.auth_context()), body_bytes).await;
                    match session.finish(status.as_u16(), body) {
                        Ok(resp) => signed_response(resp),
                        Err(e) => json_value(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            json!({
                                "error": "response_signing_error",
                                "details": e.to_string(),
                            }),
                        ),
                    }
                }
            }
        }
    }
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
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

fn json_value(status: StatusCode, value: serde_json::Value) -> warp::reply::Response {
    let (status, body) = json_bytes_or_fallback(status, value);
    json_bytes(status, body)
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
    json_value(
        StatusCode::UNAUTHORIZED,
        json!({
            "error": "auth_failed",
            "details": err.to_string(),
        }),
    )
}

fn signed_response(resp: SignedResponseV1) -> warp::reply::Response {
    let mut response = warp::reply::Response::new(warp::hyper::Body::from(resp.body));
    *response.status_mut() =
        StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let headers = response.headers_mut();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    for (name, value) in resp.headers.to_pairs() {
        headers.insert(
            name,
            HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static("")),
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        ed25519_dalek::SigningKey,
        nexus_sdk::signed_http::v1::{
            engine::{SignedHttpEngineV1, SignedHttpInvokerV1, SignedHttpPolicyV1},
            wire::{
                AllowedLeaderFileV1,
                AllowedLeaderKeyFileV1,
                AllowedLeadersFileV1,
                AllowedLeadersV1,
                EncodedSignatureHeadersV1,
                HEADER_SIG,
                HEADER_SIG_INPUT,
                HEADER_SIG_VERSION,
                SIG_VERSION_V1,
            },
        },
        std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        warp::{http::header::HeaderValue, hyper::body::to_bytes},
    };

    fn make_allowed_leaders(leader_id: &str, leader_pk: [u8; 32]) -> AllowedLeadersV1 {
        AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
            version: 1,
            leaders: vec![AllowedLeaderFileV1 {
                leader_id: leader_id.to_string(),
                keys: vec![AllowedLeaderKeyFileV1 {
                    kid: 0,
                    public_key: hex::encode(leader_pk),
                }],
            }],
        })
        .unwrap()
    }

    fn request_headers(headers: &EncodedSignatureHeadersV1) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(HEADER_SIG_VERSION, HeaderValue::from_static(SIG_VERSION_V1));
        map.insert(
            HEADER_SIG_INPUT,
            HeaderValue::from_str(&headers.sig_input_b64).unwrap(),
        );
        map.insert(HEADER_SIG, HeaderValue::from_str(&headers.sig_b64).unwrap());
        map
    }

    fn build_invoker(
        engine: &SignedHttpEngineV1,
        leader_id: &str,
        leader_sk: &SigningKey,
    ) -> SignedHttpInvokerV1 {
        engine.invoker(leader_id.to_string(), 0, leader_sk.clone())
    }

    #[tokio::test]
    async fn unsigned_invoke_returns_plain_json() {
        let http = HttpRequestMeta {
            method: "POST",
            path: "/invoke",
            query: "",
        };

        let resp = handle_invoke(
            InvokeAuthRuntime::Unsigned,
            http,
            HeaderMap::new(),
            br#"{"hello":"world"}"#.to_vec(),
            |_ctx, _body| async { (StatusCode::OK, br#"{"ok":true}"#.to_vec()) },
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("content-type").unwrap(),
            "application/json"
        );
        let body = to_bytes(resp.into_body()).await.unwrap();
        assert_eq!(body.as_ref(), br#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn signed_invoke_missing_headers_is_rejected() {
        let leader_id = "0x1111";
        let tool_id = "demo::tool::1.0.0";
        let leader_sk = SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk = leader_sk.verifying_key().to_bytes();
        let tool_sk = SigningKey::from_bytes(&[9u8; 32]);

        let allowed = make_allowed_leaders(leader_id, leader_pk);
        let engine = SignedHttpEngineV1::new(SignedHttpPolicyV1 {
            max_clock_skew_ms: 0,
            max_validity_ms: 10_000,
        });
        let responder =
            engine.responder_with_in_memory_replay(tool_id.to_string(), 0, tool_sk, allowed);

        let http = HttpRequestMeta {
            method: "POST",
            path: "/invoke",
            query: "",
        };

        let resp = handle_invoke(
            InvokeAuthRuntime::Signed(Box::new(responder)),
            http,
            HeaderMap::new(),
            br#"{"hello":"world"}"#.to_vec(),
            |_ctx, _body| async { (StatusCode::OK, br#"{"ok":true}"#.to_vec()) },
        )
        .await;

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = to_bytes(resp.into_body()).await.unwrap();
        assert!(body
            .windows(b"auth_failed".len())
            .any(|w| w == b"auth_failed"));
    }

    #[tokio::test]
    async fn signed_invoke_success_and_replay_conflict() {
        let leader_id = "0x1111";
        let tool_id = "demo::tool::1.0.0";

        let leader_sk = SigningKey::from_bytes(&[7u8; 32]);
        let leader_pk = leader_sk.verifying_key().to_bytes();
        let tool_sk = SigningKey::from_bytes(&[9u8; 32]);

        let allowed = make_allowed_leaders(leader_id, leader_pk);
        let engine = SignedHttpEngineV1::new(SignedHttpPolicyV1 {
            max_clock_skew_ms: 0,
            max_validity_ms: 10_000,
        });
        let invoker = build_invoker(&engine, leader_id, &leader_sk);
        let responder =
            engine.responder_with_in_memory_replay(tool_id.to_string(), 0, tool_sk, allowed);

        let http = HttpRequestMeta {
            method: "POST",
            path: "/invoke",
            query: "",
        };

        let body_one = br#"{"hello":"world"}"#.to_vec();
        let outbound_one = invoker
            .begin_invoke_with_nonce(
                tool_id.to_string(),
                http.clone(),
                &body_one,
                "nonce".to_string(),
            )
            .unwrap();
        let headers_one = request_headers(outbound_one.request_headers());

        let run_calls = Arc::new(AtomicUsize::new(0));
        let run_calls_inner = Arc::clone(&run_calls);
        let resp_one = handle_invoke(
            InvokeAuthRuntime::Signed(Box::new(responder.clone())),
            http.clone(),
            headers_one,
            body_one.clone(),
            move |_ctx, _body| {
                let run_calls_inner = Arc::clone(&run_calls_inner);
                async move {
                    run_calls_inner.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, br#"{"ok":true}"#.to_vec())
                }
            },
        )
        .await;

        assert_eq!(resp_one.status(), StatusCode::OK);
        assert!(resp_one.headers().contains_key(HEADER_SIG_INPUT));

        let body_two = br#"{"hello":"different"}"#.to_vec();
        let outbound_two = invoker
            .begin_invoke_with_nonce(
                tool_id.to_string(),
                http.clone(),
                &body_two,
                "nonce".to_string(),
            )
            .unwrap();
        let headers_two = request_headers(outbound_two.request_headers());

        let resp_two = handle_invoke(
            InvokeAuthRuntime::Signed(Box::new(responder)),
            http,
            headers_two,
            body_two,
            |_ctx, _body| async { (StatusCode::OK, br#"{"ok":true}"#.to_vec()) },
        )
        .await;

        assert_eq!(resp_two.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(run_calls.load(Ordering::SeqCst), 1);
    }
}
