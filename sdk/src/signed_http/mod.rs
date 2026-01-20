//! Signed HTTP requests/responses.
//!
//! This module implements a simple, deployment agnostic signing format designed
//! for Nexus Leader <=> Tool communication:
//! - Payload stays as the regular HTTP body (e.g., tool input/output JSON).
//! - A small JSON "claims" blob is serialized to bytes and signed with Ed25519.
//! - The signed bytes are shipped in `X-Nexus-Sig-Input` (base64url) alongside
//!   the signature in `X-Nexus-Sig` (base64url).
//!
//! This avoids brittle HTTP canonicalization and keeps tool schemas unchanged.
//!
//! "Claims" here means signed assertions about the HTTP message (who is calling, what is being
//! called, which body bytes are being sent, and within what time/nonce window), not the tool's
//! application payload itself.
//!
//! # Why an "envelope" header design?
//! Many HTTP signing schemes require canonicalizing the request target, headers, and body.
//! In practice this becomes fragile when requests traverse heterogeneous infrastructure
//! (reverse proxies, load balancers, gateways) that may normalize or rewrite parts of the
//! request/response.
//!
//! Instead, Nexus signs a small, explicit JSON claims blob that contains:
//! - the HTTP method + path + query string,
//! - a SHA-256 hash of the raw body bytes,
//! - a time window (`iat_ms`/`exp_ms`) and `nonce`,
//! - the `LeaderId`/`ToolId` identifiers and key ids.
//!
//! The claims bytes are the only bytes covered by the Ed25519 signature. This ensures:
//! - tool input/output schemas remain unchanged,
//! - no bespoke body canonicalization is required,
//! - the signature is stable and auditable (store claims bytes + signature).
//!
//! # Versions
//! - [`v1`] is the currently implemented protocol version.

pub mod v1;

pub mod keys;

pub use {keys::*, v1::*};
