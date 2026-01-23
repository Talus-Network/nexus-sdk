//! High-level invoker/responder helpers for signed HTTP v1.
//!
//! This module builds on [`super::wire`] and provides an ergonomic session API:
//! - invokers sign requests and verify signed responses, and
//! - responders authenticate requests, apply replay rules, and sign responses.

use {
    super::{
        error::SignedHttpError,
        wire::{
            decode_signature_headers_v1,
            encode_signature_headers_v1,
            message_to_verify,
            now_ms,
            parse_hex_32,
            sha256,
            sha256_hex,
            sign_invoke_request_v1,
            sign_invoke_response_v1,
            validate_time_window,
            verify_invoke_response_v1,
            AllowedLeadersV1,
            DecodedSignatureV1,
            EncodedSignatureHeadersV1,
            HttpRequestMeta,
            InvokeRequestClaimsV1,
            InvokeResponseClaimsV1,
            VerifyOptions,
            DOMAIN_REQUEST_V1,
            HEADER_SIG,
            HEADER_SIG_INPUT,
            HEADER_SIG_VERSION,
        },
    },
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _},
    ed25519_dalek::{Signature, SigningKey, VerifyingKey},
    rand::RngCore as _,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
};

/// Borrowed signature headers for a signed HTTP request/response.
///
/// This type exists so callers can integrate with any HTTP library without the SDK depending on a
/// specific header map type. You can construct it using [`SignatureHeadersRef::from_getter`] by
/// providing a closure that looks up a header value by name.
#[derive(Clone, Copy, Debug)]
pub struct SignatureHeadersRef<'a> {
    pub sig_v: Option<&'a str>,
    pub sig_input_b64: Option<&'a str>,
    pub sig_b64: Option<&'a str>,
}

impl<'a> SignatureHeadersRef<'a> {
    /// Build from raw header values.
    pub fn new(
        sig_v: Option<&'a str>,
        sig_input_b64: Option<&'a str>,
        sig_b64: Option<&'a str>,
    ) -> Self {
        Self {
            sig_v,
            sig_input_b64,
            sig_b64,
        }
    }

    /// Build using a closure that returns a header value by name.
    ///
    /// This avoids leaking protocol header names into the rest of your codebase.
    pub fn from_getter(mut get: impl FnMut(&'static str) -> Option<&'a str>) -> Self {
        Self {
            sig_v: get(HEADER_SIG_VERSION),
            sig_input_b64: get(HEADER_SIG_INPUT),
            sig_b64: get(HEADER_SIG),
        }
    }
}

/// Responder-side replay behavior decision for an authenticated `(invoker_id, nonce)` pair.
#[derive(Clone, Debug)]
pub enum ReplayDecisionV1 {
    /// First time seen; allow execution and mark as `InFlight`.
    Proceed,
    /// Identical retry; return the cached signed response bytes.
    Return(SignedResponseV1),
    /// Conflicting replay (same nonce, different request hash).
    Reject,
    /// Concurrent retry while the original request is still executing.
    InFlight,
}

/// Replay storage interface used by the responder to distinguish retries from replays.
///
/// The SDK provides [`InMemoryReplayStore`] as the default implementation.
pub trait ReplayStore: Send + Sync {
    /// Called after request authentication to decide how to handle a nonce.
    fn begin_or_replay(
        &self,
        nonce_key: &str,
        request_hash: [u8; 32],
        expires_at_ms: u64,
        now_ms: u64,
    ) -> ReplayDecisionV1;

    /// Mark a nonce as completed and store the signed response for future retries.
    fn complete(
        &self,
        nonce_key: &str,
        request_hash: [u8; 32],
        expires_at_ms: u64,
        response: SignedResponseV1,
    );

    /// Remove an `InFlight` reservation for a nonce.
    fn remove(&self, nonce_key: &str);
}

#[derive(Clone)]
struct InFlightGuardV1 {
    store: Arc<dyn ReplayStore>,
    nonce_key: String,
    armed: bool,
}

impl InFlightGuardV1 {
    fn new(store: Arc<dyn ReplayStore>, nonce_key: String) -> Self {
        Self {
            store,
            nonce_key,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for InFlightGuardV1 {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.store.remove(&self.nonce_key);
    }
}

#[derive(Clone)]
struct InMemoryReplayStore {
    inner: Arc<Mutex<HashMap<String, ReplayEntryV1>>>,
}

#[derive(Clone)]
struct ReplayEntryV1 {
    request_hash: [u8; 32],
    expires_at_ms: u64,
    state: ReplayStateV1,
}

#[derive(Clone)]
enum ReplayStateV1 {
    InFlight,
    Complete(SignedResponseV1),
}

impl InMemoryReplayStore {
    /// Create a new empty in-memory replay store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn purge_expired(cache: &mut HashMap<String, ReplayEntryV1>, now_ms: u64) {
        cache.retain(|_, entry| entry.expires_at_ms >= now_ms);
    }
}

impl Default for InMemoryReplayStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayStore for InMemoryReplayStore {
    fn begin_or_replay(
        &self,
        nonce_key: &str,
        request_hash: [u8; 32],
        expires_at_ms: u64,
        now_ms: u64,
    ) -> ReplayDecisionV1 {
        let mut cache = self.inner.lock().unwrap();
        Self::purge_expired(&mut cache, now_ms);

        match cache.get(nonce_key) {
            None => {
                cache.insert(
                    nonce_key.to_string(),
                    ReplayEntryV1 {
                        request_hash,
                        expires_at_ms,
                        state: ReplayStateV1::InFlight,
                    },
                );
                ReplayDecisionV1::Proceed
            }
            Some(entry) if entry.request_hash != request_hash => ReplayDecisionV1::Reject,
            Some(entry) => match &entry.state {
                ReplayStateV1::InFlight => ReplayDecisionV1::InFlight,
                ReplayStateV1::Complete(resp) => ReplayDecisionV1::Return(resp.clone()),
            },
        }
    }

    fn complete(
        &self,
        nonce_key: &str,
        request_hash: [u8; 32],
        expires_at_ms: u64,
        response: SignedResponseV1,
    ) {
        let mut cache = self.inner.lock().unwrap();
        cache.insert(
            nonce_key.to_string(),
            ReplayEntryV1 {
                request_hash,
                expires_at_ms,
                state: ReplayStateV1::Complete(response),
            },
        );
    }

    fn remove(&self, nonce_key: &str) {
        let mut cache = self.inner.lock().unwrap();
        cache.remove(nonce_key);
    }
}

/// Verification policy used by the high-level invoker/responder API.
///
/// This is the same set of knobs exposed in [`VerifyOptions`], but without a `now_ms` value.
#[derive(Clone, Copy, Debug)]
pub struct SignedHttpPolicyV1 {
    pub max_clock_skew_ms: u64,
    pub max_validity_ms: u64,
}

impl Default for SignedHttpPolicyV1 {
    fn default() -> Self {
        Self {
            max_clock_skew_ms: 30_000,
            max_validity_ms: 60_000,
        }
    }
}

/// Time source used by [`SignedHttpEngineV1`].
pub trait ClockV1: Send + Sync {
    fn now_ms(&self) -> u64;
}

/// Wall-clock time source based on [`now_ms`].
#[derive(Clone, Default)]
pub struct SystemClockV1;

impl ClockV1 for SystemClockV1 {
    fn now_ms(&self) -> u64 {
        now_ms()
    }
}

/// High-level entry point for signed HTTP v1.
///
/// A process typically creates a single engine and then uses it in one of two directions:
/// - Invoker: sign outbound requests and verify the responder's signed responses.
/// - Responder: authenticate inbound requests, enforce replay rules, and sign responses.
#[derive(Clone)]
pub struct SignedHttpEngineV1 {
    clock: Arc<dyn ClockV1>,
    policy: SignedHttpPolicyV1,
}

impl SignedHttpEngineV1 {
    /// Create a new engine using the system clock.
    pub fn new(policy: SignedHttpPolicyV1) -> Self {
        Self::with_clock(policy, SystemClockV1)
    }

    /// Create a new engine with a custom clock (useful for tests).
    pub fn with_clock(policy: SignedHttpPolicyV1, clock: impl ClockV1 + 'static) -> Self {
        Self {
            clock: Arc::new(clock),
            policy,
        }
    }

    pub fn policy(&self) -> SignedHttpPolicyV1 {
        self.policy
    }

    fn verify_options(&self) -> VerifyOptions {
        VerifyOptions {
            now_ms: self.clock.now_ms(),
            max_clock_skew_ms: self.policy.max_clock_skew_ms,
            max_validity_ms: self.policy.max_validity_ms,
        }
    }

    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    /// Create an invoker helper bound to an invoker identity and signing key.
    pub fn invoker(
        &self,
        invoker_id: impl Into<String>,
        invoker_kid: u64,
        invoker_signing_key: SigningKey,
    ) -> SignedHttpInvokerV1 {
        SignedHttpInvokerV1 {
            engine: self.clone(),
            invoker_id: invoker_id.into(),
            invoker_kid,
            invoker_signing_key,
        }
    }

    /// Create a responder helper with an in-memory replay store.
    pub fn responder_with_in_memory_replay(
        &self,
        responder_id: impl Into<String>,
        responder_kid: u64,
        responder_signing_key: SigningKey,
        invoker_keys: impl InvokerKeyResolver + 'static,
    ) -> SignedHttpResponderV1 {
        self.responder(
            responder_id,
            responder_kid,
            responder_signing_key,
            invoker_keys,
            Arc::new(InMemoryReplayStore::new()),
        )
    }

    /// Create a responder helper with a custom replay store.
    pub fn responder(
        &self,
        responder_id: impl Into<String>,
        responder_kid: u64,
        responder_signing_key: SigningKey,
        invoker_keys: impl InvokerKeyResolver + 'static,
        replay_store: Arc<dyn ReplayStore>,
    ) -> SignedHttpResponderV1 {
        SignedHttpResponderV1 {
            engine: self.clone(),
            responder_id: responder_id.into(),
            responder_kid,
            responder_signing_key,
            invoker_keys: Arc::new(invoker_keys),
            replay_store,
        }
    }
}

/// Resolve invoker public keys when authenticating requests.
pub trait InvokerKeyResolver: Send + Sync {
    fn invoker_public_key(&self, invoker_id: &str, invoker_kid: u64) -> Option<[u8; 32]>;
}

/// Resolve responder public keys when verifying responses.
pub trait ResponderKeyResolver: Send + Sync {
    fn responder_public_key(&self, responder_id: &str, responder_kid: u64) -> Option<[u8; 32]>;
}

impl<T: InvokerKeyResolver + ?Sized> InvokerKeyResolver for Arc<T> {
    fn invoker_public_key(&self, invoker_id: &str, invoker_kid: u64) -> Option<[u8; 32]> {
        (**self).invoker_public_key(invoker_id, invoker_kid)
    }
}

impl<T: ResponderKeyResolver + ?Sized> ResponderKeyResolver for Arc<T> {
    fn responder_public_key(&self, responder_id: &str, responder_kid: u64) -> Option<[u8; 32]> {
        (**self).responder_public_key(responder_id, responder_kid)
    }
}

impl InvokerKeyResolver for AllowedLeadersV1 {
    fn invoker_public_key(&self, invoker_id: &str, invoker_kid: u64) -> Option<[u8; 32]> {
        self.leader_public_key_bytes(invoker_id, invoker_kid)
    }
}

/// Simple key resolver for a single responder key.
#[derive(Clone, Debug)]
pub struct StaticResponderKeyV1 {
    pub responder_id: String,
    pub responder_kid: u64,
    pub public_key: [u8; 32],
}

impl ResponderKeyResolver for StaticResponderKeyV1 {
    fn responder_public_key(&self, responder_id: &str, responder_kid: u64) -> Option<[u8; 32]> {
        if responder_id != self.responder_id || responder_kid != self.responder_kid {
            return None;
        }
        Some(self.public_key)
    }
}

/// Authenticated request context derived from a verified inbound request.
///
/// This is designed for tool/business-level authorization hooks (`allowlists`, `rate limits`, etc)
/// without requiring any external reads at runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthContextV1 {
    pub invoker_id: String,
    pub invoker_kid: u64,
    pub responder_id: String,
    pub iat_ms: u64,
    pub exp_ms: u64,
    pub nonce: String,
    pub method: String,
    pub path: String,
    pub query: String,
    pub invoker_public_key: [u8; 32],
    pub request_sig_input_sha256: [u8; 32],
}

/// A signed HTTP response (status + body + signature headers).
#[derive(Clone, Debug)]
pub struct SignedResponseV1 {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: EncodedSignatureHeadersV1,
}

/// Invoker helper for signing requests and verifying responses.
#[derive(Clone)]
pub struct SignedHttpInvokerV1 {
    engine: SignedHttpEngineV1,
    invoker_id: String,
    invoker_kid: u64,
    invoker_signing_key: SigningKey,
}

impl SignedHttpInvokerV1 {
    /// Begin a signed invocation request to `responder_id`.
    ///
    /// The returned session is stable: calling [`OutboundSessionV1::request_headers`] multiple
    /// times returns identical headers, which enables safe network retries.
    pub fn begin_invoke(
        &self,
        responder_id: impl Into<String>,
        http: HttpRequestMeta<'_>,
        body: &[u8],
    ) -> Result<OutboundSessionV1, SignedHttpError> {
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let nonce = URL_SAFE_NO_PAD.encode(bytes);

        self.begin_invoke_with_nonce(responder_id, http, body, nonce)
    }

    /// Begin a signed invocation request using an explicit nonce.
    ///
    /// This is useful for tests and for advanced retry models where the invoker wants to
    /// deterministically select the nonce.
    pub fn begin_invoke_with_nonce(
        &self,
        responder_id: impl Into<String>,
        http: HttpRequestMeta<'_>,
        body: &[u8],
        nonce: String,
    ) -> Result<OutboundSessionV1, SignedHttpError> {
        let responder_id = responder_id.into();

        let iat_ms = self.engine.now_ms();
        let exp_ms = iat_ms.saturating_add(self.engine.policy.max_validity_ms);

        let claims = InvokeRequestClaimsV1 {
            leader_id: self.invoker_id.clone(),
            leader_kid: self.invoker_kid,
            tool_id: responder_id.clone(),
            iat_ms,
            exp_ms,
            nonce: nonce.clone(),
            method: http.method.to_string(),
            path: http.path.to_string(),
            query: http.query.to_string(),
            body_sha256: sha256_hex(body),
        };

        let (sig_input, sig) = sign_invoke_request_v1(&claims, &self.invoker_signing_key)?;
        let sig_input_sha256 = sha256(&sig_input);
        let headers = encode_signature_headers_v1(&sig_input, &sig);

        Ok(OutboundSessionV1 {
            engine: self.engine.clone(),
            expected_responder_id: responder_id,
            nonce,
            sig_input,
            sig_input_sha256,
            headers,
        })
    }
}

/// Per-invocation invoker session (request signing + response verification).
#[derive(Clone)]
pub struct OutboundSessionV1 {
    engine: SignedHttpEngineV1,
    expected_responder_id: String,
    nonce: String,
    sig_input: Vec<u8>,
    sig_input_sha256: [u8; 32],
    headers: EncodedSignatureHeadersV1,
}

impl OutboundSessionV1 {
    /// Return signature headers for the request.
    pub fn request_headers(&self) -> &EncodedSignatureHeadersV1 {
        &self.headers
    }

    /// Return the nonce used for this invocation.
    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    /// Return the raw signed-request `sig_input` bytes (the JSON claims bytes).
    ///
    /// This is useful for auditing/debugging (e.g., persisting a signed transcript alongside
    /// `request_sig_input_sha256`).
    pub fn request_sig_input_bytes(&self) -> &[u8] {
        &self.sig_input
    }

    /// Return `sha256(request_sig_input_bytes)`, used for response binding.
    pub fn request_sig_input_sha256(&self) -> [u8; 32] {
        self.sig_input_sha256
    }

    /// Verify the responder's signed response and ensure it is bound to this request.
    pub fn verify_response(
        &self,
        status: u16,
        headers: SignatureHeadersRef<'_>,
        body: &[u8],
        responder_keys: &dyn ResponderKeyResolver,
    ) -> Result<VerifiedOutboundResponseV1, SignedHttpError> {
        let decoded =
            decode_signature_headers_v1(headers.sig_v, headers.sig_input_b64, headers.sig_b64)?;

        let claims: InvokeResponseClaimsV1 = serde_json::from_slice(&decoded.sig_input)
            .map_err(SignedHttpError::InvalidSignedInputJson)?;

        if claims.status != status {
            return Err(SignedHttpError::StatusMismatch {
                claimed: claims.status,
                actual: status,
            });
        }

        let tool_public_key = responder_keys
            .responder_public_key(&claims.tool_id, claims.tool_kid)
            .ok_or_else(|| SignedHttpError::UnknownToolKey {
                tool_id: claims.tool_id.clone(),
                tool_kid: claims.tool_kid,
            })?;

        let verified = verify_invoke_response_v1(
            decoded,
            body,
            &self.expected_responder_id,
            self.sig_input_sha256,
            tool_public_key,
            &self.engine.verify_options(),
        )?;

        Ok(VerifiedOutboundResponseV1 {
            responder_id: verified.claims.tool_id.clone(),
            responder_kid: verified.claims.tool_kid,
            nonce: verified.claims.nonce.clone(),
            status: verified.claims.status,
            responder_public_key: verified.tool_public_key,
            response_sig_input_sha256: verified.sig_input_sha256,
        })
    }
}

/// Result of verifying a responder's signed response for an outbound session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedOutboundResponseV1 {
    pub responder_id: String,
    pub responder_kid: u64,
    pub nonce: String,
    pub status: u16,
    pub responder_public_key: [u8; 32],
    pub response_sig_input_sha256: [u8; 32],
}

/// Responder helper for authenticating requests and producing signed responses.
#[derive(Clone)]
pub struct SignedHttpResponderV1 {
    engine: SignedHttpEngineV1,
    responder_id: String,
    responder_kid: u64,
    responder_signing_key: SigningKey,
    invoker_keys: Arc<dyn InvokerKeyResolver>,
    replay_store: Arc<dyn ReplayStore>,
}

/// Result of authenticating an inbound request on the responder side.
pub enum ResponderDecisionV1 {
    /// Execute the request and then call [`InboundSessionV1::finish`] to sign + cache the response.
    Proceed(InboundSessionV1),
    /// Return the cached signed response (idempotent retry).
    Return(SignedResponseV1),
    /// Reject replay/in-flight conditions after authenticating the request.
    Reject(ResponderRejectionV1),
}

/// Replay-related rejection kinds (after request authentication).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponderRejectionKindV1 {
    ReplayConflict,
    InFlight,
}

/// A verified inbound request without an `InFlight` reservation.
///
/// This is returned for replay related rejections so callers can still produce a signed error
/// response that is bound to the authenticated request.
#[derive(Clone)]
pub struct AuthenticatedRequestV1 {
    responder: SignedHttpResponderV1,
    verified: VerifiedInboundRequestV1,
}

impl AuthenticatedRequestV1 {
    pub fn auth_context(&self) -> AuthContextV1 {
        AuthContextV1 {
            invoker_id: self.verified.invoker_id.clone(),
            invoker_kid: self.verified.invoker_kid,
            responder_id: self.verified.responder_id.clone(),
            iat_ms: self.verified.iat_ms,
            exp_ms: self.verified.exp_ms,
            nonce: self.verified.nonce.clone(),
            method: self.verified.method.clone(),
            path: self.verified.path.clone(),
            query: self.verified.query.clone(),
            invoker_public_key: self.verified.invoker_public_key,
            request_sig_input_sha256: self.verified.sig_input_sha256,
        }
    }

    pub fn sign_response(
        &self,
        status: u16,
        body: &[u8],
    ) -> Result<SignedResponseV1, SignedHttpError> {
        self.responder
            .sign_response_for(&self.verified, status, body)
    }
}

/// Replay related rejection after request authentication.
#[derive(Clone)]
pub struct ResponderRejectionV1 {
    pub kind: ResponderRejectionKindV1,
    pub request: AuthenticatedRequestV1,
}

impl ResponderRejectionV1 {
    pub fn auth_context(&self) -> AuthContextV1 {
        self.request.auth_context()
    }

    pub fn sign_response(
        &self,
        status: u16,
        body: &[u8],
    ) -> Result<SignedResponseV1, SignedHttpError> {
        self.request.sign_response(status, body)
    }
}

/// Verified inbound request envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedInboundRequestV1 {
    pub invoker_id: String,
    pub invoker_kid: u64,
    pub responder_id: String,
    pub iat_ms: u64,
    pub exp_ms: u64,
    pub nonce: String,
    pub method: String,
    pub path: String,
    pub query: String,
    pub invoker_public_key: [u8; 32],
    pub sig_input: Vec<u8>,
    pub sig_input_sha256: [u8; 32],
}

/// Per-invocation responder session (authentication + replay reservation).
#[derive(Clone)]
pub struct InboundSessionV1 {
    request: AuthenticatedRequestV1,
    nonce_key: String,
    request_hash: [u8; 32],
    expires_at_ms: u64,
    guard: InFlightGuardV1,
}

impl InboundSessionV1 {
    pub fn auth_context(&self) -> AuthContextV1 {
        self.request.auth_context()
    }

    /// Sign and cache a response, then return it for transport.
    pub fn finish(
        mut self,
        status: u16,
        body: Vec<u8>,
    ) -> Result<SignedResponseV1, SignedHttpError> {
        let resp = self.request.sign_response(status, &body)?;
        self.request.responder.replay_store.complete(
            &self.nonce_key,
            self.request_hash,
            self.expires_at_ms,
            resp.clone(),
        );
        self.guard.disarm();
        Ok(SignedResponseV1 {
            status: resp.status,
            body,
            headers: resp.headers,
        })
    }
}

impl SignedHttpResponderV1 {
    /// Authenticate a signed invocation request and apply replay rules.
    pub fn authenticate_invoke(
        &self,
        http: HttpRequestMeta<'_>,
        body: &[u8],
        headers: SignatureHeadersRef<'_>,
    ) -> Result<ResponderDecisionV1, SignedHttpError> {
        let decoded =
            decode_signature_headers_v1(headers.sig_v, headers.sig_input_b64, headers.sig_b64)?;

        let verified = self.verify_inbound_request(decoded, http, body)?;
        let nonce_key = format!("{}:{}", verified.invoker_id, verified.nonce);
        let expires_at_ms = verified.exp_ms;
        let request_hash = verified.sig_input_sha256;

        let now_ms = self.engine.now_ms();
        match self
            .replay_store
            .begin_or_replay(&nonce_key, request_hash, expires_at_ms, now_ms)
        {
            ReplayDecisionV1::Proceed => {
                let request = AuthenticatedRequestV1 {
                    responder: self.clone(),
                    verified: verified.clone(),
                };
                Ok(ResponderDecisionV1::Proceed(InboundSessionV1 {
                    request,
                    nonce_key: nonce_key.clone(),
                    request_hash,
                    expires_at_ms,
                    guard: InFlightGuardV1::new(self.replay_store.clone(), nonce_key),
                }))
            }
            ReplayDecisionV1::Return(resp) => Ok(ResponderDecisionV1::Return(resp)),
            ReplayDecisionV1::Reject => Ok(ResponderDecisionV1::Reject(ResponderRejectionV1 {
                kind: ResponderRejectionKindV1::ReplayConflict,
                request: AuthenticatedRequestV1 {
                    responder: self.clone(),
                    verified,
                },
            })),
            ReplayDecisionV1::InFlight => Ok(ResponderDecisionV1::Reject(ResponderRejectionV1 {
                kind: ResponderRejectionKindV1::InFlight,
                request: AuthenticatedRequestV1 {
                    responder: self.clone(),
                    verified,
                },
            })),
        }
    }

    fn verify_inbound_request(
        &self,
        decoded: DecodedSignatureV1,
        http: HttpRequestMeta<'_>,
        body: &[u8],
    ) -> Result<VerifiedInboundRequestV1, SignedHttpError> {
        let claims: InvokeRequestClaimsV1 = serde_json::from_slice(&decoded.sig_input)
            .map_err(SignedHttpError::InvalidSignedInputJson)?;

        if claims.tool_id != self.responder_id {
            return Err(SignedHttpError::ToolIdMismatch {
                claimed: claims.tool_id,
                expected: self.responder_id.clone(),
            });
        }

        if claims.method != http.method {
            return Err(SignedHttpError::MethodMismatch {
                claimed: claims.method,
                actual: http.method.to_string(),
            });
        }

        if claims.path != http.path {
            return Err(SignedHttpError::PathMismatch {
                claimed: claims.path,
                actual: http.path.to_string(),
            });
        }

        if claims.query != http.query {
            return Err(SignedHttpError::QueryMismatch {
                claimed: claims.query,
                actual: http.query.to_string(),
            });
        }

        let body_sha256 = sha256(body);
        let claimed_body_sha256 = parse_hex_32(&claims.body_sha256)
            .map_err(|_| SignedHttpError::InvalidBodySha256Hex(claims.body_sha256.clone()))?;
        if body_sha256 != claimed_body_sha256 {
            return Err(SignedHttpError::BodyHashMismatch);
        }

        validate_time_window(claims.iat_ms, claims.exp_ms, &self.engine.verify_options())?;

        let invoker_public_key = self
            .invoker_keys
            .invoker_public_key(&claims.leader_id, claims.leader_kid)
            .ok_or_else(|| SignedHttpError::UnknownLeaderKey {
                leader_id: claims.leader_id.clone(),
                leader_kid: claims.leader_kid,
            })?;

        let verifying_key = VerifyingKey::from_bytes(&invoker_public_key).map_err(|_| {
            SignedHttpError::InvalidLeaderPublicKey {
                leader_id: claims.leader_id.clone(),
                leader_kid: claims.leader_kid,
            }
        })?;

        let msg = message_to_verify(DOMAIN_REQUEST_V1, &decoded.sig_input);
        let sig = Signature::from_bytes(&decoded.signature);
        verifying_key
            .verify_strict(&msg, &sig)
            .map_err(|_| SignedHttpError::InvalidSignature)?;

        let sig_input_sha256 = sha256(&decoded.sig_input);

        Ok(VerifiedInboundRequestV1 {
            invoker_id: claims.leader_id,
            invoker_kid: claims.leader_kid,
            responder_id: claims.tool_id,
            iat_ms: claims.iat_ms,
            exp_ms: claims.exp_ms,
            nonce: claims.nonce,
            method: claims.method,
            path: claims.path,
            query: claims.query,
            invoker_public_key,
            sig_input: decoded.sig_input,
            sig_input_sha256,
        })
    }

    fn sign_response_for(
        &self,
        verified: &VerifiedInboundRequestV1,
        status: u16,
        body: &[u8],
    ) -> Result<SignedResponseV1, SignedHttpError> {
        let iat_ms = self.engine.now_ms();
        let exp_ms = iat_ms.saturating_add(self.engine.policy.max_validity_ms);

        let claims = InvokeResponseClaimsV1 {
            tool_id: self.responder_id.clone(),
            tool_kid: self.responder_kid,
            iat_ms,
            exp_ms,
            nonce: verified.nonce.clone(),
            req_sig_input_sha256: hex::encode(verified.sig_input_sha256),
            status,
            body_sha256: sha256_hex(body),
        };

        let (sig_input, sig) = sign_invoke_response_v1(&claims, &self.responder_signing_key)?;
        let headers = encode_signature_headers_v1(&sig_input, &sig);

        Ok(SignedResponseV1 {
            status,
            body: body.to_vec(),
            headers,
        })
    }
}
