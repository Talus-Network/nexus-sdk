//! Signed HTTP protocol v1.
//!
//! This module defines the on-wire format and verification logic for Nexus'
//! application-layer Ed25519 signatures used in Leader <=> Tool HTTP calls.
//!
//! # Module layout
//! - [`error`]: shared error types.
//! - [`wire`]: the v1 wire format (claims structs, header encoding/decoding, and low-level sign/verify).
//! - [`engine`]: a higher-level invoker/responder API (sessions + replay store) built on top of [`wire`].
//!
//! Most consumers should prefer the invoker/responder API ([`engine::SignedHttpEngineV1`],
//! [`engine::SignedHttpInvokerV1`], [`engine::SignedHttpResponderV1`]) so that HTTP signing, replay
//! rules, and response binding are encapsulated.
//!
//! # Wire format (headers)
//! Every signed request/response carries three headers:
//! - [`wire::HEADER_SIG_VERSION`]: protocol version string (`"1"` for this module).
//! - [`wire::HEADER_SIG_INPUT`]: base64url (no padding) of the raw JSON claims bytes.
//! - [`wire::HEADER_SIG`]: base64url (no padding) of the 64-byte Ed25519 signature.
//!
//! `base64url` is the URL-safe base64 variant (RFC 4648) that uses `-` and `_` instead of `+`
//! and `/`. We use the no-padding form because it is compact and safe to embed in HTTP headers.
//!
//! ## HTTP refresher: headers vs body
//! An HTTP request/response is composed of:
//! - a start-line (`POST /invoke ...` or `HTTP/1.1 200 OK`),
//! - headers (small key/value metadata fields), and
//! - an optional body (the main payload bytes).
//!
//! In this protocol, the tool input/output stays in the HTTP body. The signature metadata lives
//! in headers so tools do not need to wrap or change their JSON schemas.
//!
//! This format is compact, easy to forward through HTTP infrastructure, and widely supported
//! (it is the same encoding used by JWTs).
//!
//! ## Why headers (not an "envelope" body)?
//! Nexus intentionally does not wrap the application payload in a signed envelope object.
//! Instead, the payload remains the body, and the signed envelope (claims + signature) is carried
//! in headers:
//! - preserves existing tool request/response schemas (no wrapper object)
//! - works for any content-type (JSON, protobuf, etc) without schema changes
//! - aligns with common HTTP authentication patterns (e.g. `Authorization`, AWS SigV4, RFC 9421
//!   HTTP Message Signatures)
//!
//! Operational note: your gateway/proxy must forward the `X-Nexus-*` headers. If it strips them,
//! signed HTTP will fail closed (the tool will reject the request).
//!
//! The HTTP body is not replaced by an envelope. It stays as the tool input/output
//! payload (usually JSON). Instead, the body is bound into the signature via a
//! SHA-256 hash stored inside the claims.
//!
//! # What is actually signed?
//! The signature is computed over:
//! - a protocol-specific domain separator (request vs response), and
//! - the exact `sig_input` bytes (the JSON encoding of the claims).
//!
//! Concretely:
//! - Request domain: `b"nexus.leader_tool.request.v1."`
//! - Response domain: `b"nexus.leader_tool.response.v1."`
//!
//! This prevents cross-protocol signature replay (a valid request signature cannot be
//! reused as a response signature).
//!
//! # Claims
//! The signed claims are JSON-serialized structs:
//! - [`wire::InvokeRequestClaimsV1`] (Leader -> Tool)
//! - [`wire::InvokeResponseClaimsV1`] (Tool -> Leader)
//!
//! We call these fields "claims" because they are signed assertions made by the sender about
//! the HTTP message (identity, intent, body hash, freshness, nonce), rather than the application
//! payload itself (which remains in the HTTP body).
//!
//! Request claims bind:
//! - identity (`leader_id`, `leader_kid`, `tool_id`)
//! - intent (`method`, `path`, `query`)
//! - body integrity (`body_sha256`)
//! - freshness (`iat_ms`, `exp_ms`)
//! - uniqueness (`nonce`)
//!
//! Response claims bind:
//! - identity (`tool_id`, `tool_kid`)
//! - request binding (`nonce`, `req_sig_input_sha256`)
//! - body integrity (`body_sha256`)
//! - freshness (`iat_ms`, `exp_ms`)
//!
//! # Replay resistance and retries
//! This module verifies signature correctness and freshness windows, but it does not
//! persist nonce state. Callers MUST implement nonce replay tracking appropriate to
//! their deployment.
//!
//! A recommended model (implemented in `nexus-toolkit`) is:
//! - key nonce state by `(tool_id, nonce)`
//! - accept an identical retry when the request identity matches (`method`, `path`, `query`, `body_sha256`)
//! - reject a conflicting replay (same nonce, different request identity)
//!
//! # Example: sign and verify a request + response
//! ```
//! use {
//!     ed25519_dalek::SigningKey,
//!     nexus_sdk::signed_http::v1::wire::*,
//! };
//!
//! // === Setup identities and keys ===
//! let leader_id = "0x1111";
//! let tool_id = "demo::tool::1.0.0";
//!
//! let leader_sk = SigningKey::from_bytes(&[7u8; 32]);
//! fn to_hex(bytes: &[u8]) -> String {
//!     bytes.iter().map(|b| format!("{:02x}", b)).collect()
//! }
//! let leader_pk_hex = to_hex(&leader_sk.verifying_key().to_bytes());
//! let tool_sk = SigningKey::from_bytes(&[9u8; 32]);
//! let tool_pk = tool_sk.verifying_key().to_bytes();
//!
//! // Tool-side allowlist: leader_id + key id -> leader public key.
//! let allowed = AllowedLeadersV1::try_from(AllowedLeadersFileV1 {
//!     version: 1,
//!     leaders: vec![AllowedLeaderFileV1 {
//!         leader_id: leader_id.to_string(),
//!         keys: vec![AllowedLeaderKeyFileV1 {
//!             kid: 0, // key id
//!             public_key: leader_pk_hex,
//!         }],
//!     }],
//! })
//! .unwrap();
//!
//! // === Leader signs the request ===
//! let req_body = br#"{"hello":"world"}"#;
//! let req_claims = InvokeRequestClaimsV1 {
//!     leader_id: leader_id.to_string(),
//!     leader_kid: 0,
//!     tool_id: tool_id.to_string(),
//!     iat_ms: 1000,
//!     exp_ms: 2000,
//!     nonce: "abc".to_string(),
//!     method: "POST".to_string(),
//!     path: "/invoke".to_string(),
//!     query: "".to_string(),
//!     body_sha256: sha256_hex(req_body),
//! };
//!
//! let (req_sig_input, req_sig) = sign_invoke_request_v1(&req_claims, &leader_sk).unwrap();
//! let req_headers = encode_signature_headers_v1(&req_sig_input, &req_sig);
//!
//! // === Tool verifies the request ===
//! let decoded = decode_signature_headers_v1(
//!     Some(SIG_VERSION_V1),
//!     Some(&req_headers.sig_input_b64),
//!     Some(&req_headers.sig_b64),
//! )
//! .unwrap();
//!
//! let opts = VerifyOptions {
//!     now_ms: 1500,
//!     max_clock_skew_ms: 0,
//!     max_validity_ms: 10_000,
//! };
//!
//! let verified_req = verify_invoke_request_v1(
//!     decoded,
//!     HttpRequestMeta {
//!         method: "POST",
//!         path: "/invoke",
//!         query: "",
//!     },
//!     req_body,
//!     tool_id,
//!     &allowed,
//!     &opts,
//! )
//! .unwrap();
//!
//! // === Tool signs a response bound to the request ===
//! let resp_body = br#"{"ok":true}"#;
//! let resp_claims = InvokeResponseClaimsV1 {
//!     tool_id: tool_id.to_string(),
//!     tool_kid: 0,
//!     owner_leader_id: verified_req.claims.leader_id.clone(),
//!     iat_ms: 1500,
//!     exp_ms: 2500,
//!     nonce: verified_req.claims.nonce.clone(),
//!     req_sig_input_sha256: sha256_hex(&verified_req.sig_input),
//!     status: 200,
//!     body_sha256: sha256_hex(resp_body),
//! };
//! let (resp_sig_input, resp_sig) = sign_invoke_response_v1(&resp_claims, &tool_sk).unwrap();
//! let resp_headers = encode_signature_headers_v1(&resp_sig_input, &resp_sig);
//!
//! // === Leader verifies the response (provenance + binding) ===
//! let resp_decoded = decode_signature_headers_v1(
//!     Some(SIG_VERSION_V1),
//!     Some(&resp_headers.sig_input_b64),
//!     Some(&resp_headers.sig_b64),
//! )
//! .unwrap();
//! let verified_resp = verify_invoke_response_v1(
//!     resp_decoded,
//!     resp_body,
//!     tool_id,
//!     verified_req.sig_input_sha256,
//!     tool_pk,
//!     &opts,
//! )
//! .unwrap();
//! assert_eq!(verified_resp.claims.nonce, "abc");
//! ```

pub mod engine;
pub mod error;
pub mod wire;

#[cfg(test)]
mod tests;
