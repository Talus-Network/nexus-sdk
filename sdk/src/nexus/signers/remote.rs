//! [`TransactionSigner`] backed by any HTTP signing service.
//!
//! The SDK sends the 32-byte Sui signing digest to an endpoint and gets a
//! signature back; it knows nothing about where the key lives (a KMS, an HSM,
//! an MPC cluster, ...). Any service that implements the wire protocol below
//! can be used.
//!
//! # Wire protocol v1
//!
//! All bodies are UTF-8 JSON (`Content-Type: application/json`). Every `<base64>`
//! value is the standard alphabet (RFC 4648 section 4) **with** padding. The base
//! URL is `http(s)://host[:port]` with no path. Authentication is optional:
//! `Authorization: Bearer <token>`; the service decides whether it requires it.
//!
//! 1. `GET /v1/public-key`
//!    - `200`: `{"scheme": "ed25519" | "secp256r1", "public_key": "<base64>"}`
//!    - `public_key` is the raw Sui public key: 32 bytes for `ed25519`, the
//!      33-byte compressed SEC1 point for `secp256r1`.
//! 2. `POST /v1/sign` with the body `{"digest": "<base64 of the 32-byte Sui signing digest>"}`
//!    - The service must sign any 32 bytes: the connect challenge is random, not
//!      a transaction digest.
//!    - `200`: `{"signature": "<base64>"}`, a raw 64-byte signature:
//!      - `ed25519`: PureEdDSA over the 32 digest bytes;
//!      - `secp256r1`: `r || s`, ECDSA P-256 with SHA-256 over the 32 digest
//!        bytes (what Sui verifies). Low-S is preferred; a high-S signature is
//!        normalized by the SDK.
//! 3. Any non-2xx status is a failure (`401`/`403` not authorized, `400` bad
//!    request). The SDK reports the status code and at most a short sanitized
//!    prefix of the body. It retries only connect errors, timeouts and `5xx`:
//!    3 attempts in total, each limited to 10 seconds.
//! 4. Redirects are not followed (a 3xx is a failure), responses are read up to
//!    16 KiB, and environment proxies (`HTTP_PROXY` and friends) are not used:
//!    the SDK always talks to the service directly.
//!
//! # Trust
//!
//! The service is not trusted to pick the key: the caller pins the expected
//! Sui address in [`RemoteSignerConfig::expected_address`]. [`RemoteSigner::connect`]
//! checks that the service's public key derives that address and that the
//! service can sign a fresh random challenge with it, and every transaction
//! signature is verified locally against that key before it is returned.
//!
//! The bearer token is sent to [`RemoteSignerConfig::url`]. Over `http` it
//! travels in cleartext, so use `https` (with
//! [`RemoteSignerConfig::extra_root_ca_pem`] for a private CA) anywhere the
//! network is not trusted. The token never appears in `Debug` output, errors
//! or logs, and the header is marked sensitive. Error messages quote at most a
//! short prefix of a failing response body with control characters removed and
//! the token replaced by `<redacted>`; a service that echoes only part of the
//! token cannot be scrubbed.
//!
//! The default trusted roots are the bundled Mozilla roots; a private or
//! corporate CA must be supplied as [`RemoteSignerConfig::extra_root_ca_pem`].

use {
    crate::{
        nexus::{error::NexusError, transaction_signer::TransactionSigner},
        sui,
    },
    base64::Engine as _,
    futures::future::{BoxFuture, FutureExt as _},
    rand::RngCore as _,
    std::time::Duration,
};

/// Per-attempt timeout of a request to the signing service.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);
/// Total attempts (first try included) for connect errors, timeouts and 5xx.
const MAX_ATTEMPTS: u32 = 3;
/// Backoff before the second attempt; doubles for the third.
const BACKOFF: Duration = Duration::from_millis(100);
/// Longest response body read from the service.
const MAX_BODY_BYTES: usize = 16 * 1024;
/// Longest prefix of an error body that is reported.
const MAX_ERROR_BODY_CHARS: usize = 120;

/// Signature scheme of the key held by the signing service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteSignatureScheme {
    /// Ed25519 (`"ed25519"` on the wire).
    Ed25519,
    /// ECDSA over NIST P-256 with SHA-256 (`"secp256r1"` on the wire).
    Secp256r1,
}

/// Where and how to reach a signing service, and which key it must hold.
///
/// See the [module documentation](self) for the wire protocol. `Debug` prints
/// the URL host and the address only; the token is shown as `<redacted>`.
#[derive(Clone)]
pub struct RemoteSignerConfig {
    /// Base URL, `http(s)://host[:port]`, without userinfo, path, query or fragment.
    pub url: String,
    /// Sent as `Authorization: Bearer <token>` to `url` (in cleartext over
    /// `http`). Never logged.
    pub bearer_token: Option<String>,
    /// The Sui address the service's key must derive; pinned by the caller.
    pub expected_address: sui::types::Address,
    /// PEM certificate(s) trusted in addition to the default roots (the bundled
    /// Mozilla roots), for a service under a private or corporate CA. Must
    /// contain at least one certificate.
    pub extra_root_ca_pem: Option<Vec<u8>>,
}

impl std::fmt::Debug for RemoteSignerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSignerConfig")
            .field("url_host", &host_of(&self.url))
            .field(
                "bearer_token",
                &self.bearer_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expected_address", &self.expected_address)
            .field("extra_root_ca", &self.extra_root_ca_pem.is_some())
            .finish()
    }
}

fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| "<invalid>".into())
}

/// The public key of the service, with the local signature checks.
#[derive(Clone)]
enum Key {
    Ed25519(ed25519_dalek::VerifyingKey),
    Secp256r1(p256::ecdsa::VerifyingKey),
}

impl Key {
    fn scheme(&self) -> RemoteSignatureScheme {
        match self {
            Key::Ed25519(_) => RemoteSignatureScheme::Ed25519,
            Key::Secp256r1(_) => RemoteSignatureScheme::Secp256r1,
        }
    }

    fn sui_public_key(&self) -> SuiKey {
        match self {
            Key::Ed25519(key) => SuiKey::Ed25519(sui::types::Ed25519PublicKey::new(key.to_bytes())),
            Key::Secp256r1(key) => {
                let point = key.to_encoded_point(true);
                let bytes = <[u8; 33]>::try_from(point.as_bytes())
                    .expect("a compressed P-256 point is 33 bytes");
                SuiKey::Secp256r1(sui::types::Secp256r1PublicKey::new(bytes))
            }
        }
    }

    /// Checks the length, normalizes secp256r1 to low-S and verifies `signature`
    /// over `digest`. Returns the signature to use, or `None` if it is not valid.
    fn checked_signature(&self, digest: &[u8; 32], signature: &[u8]) -> Option<[u8; 64]> {
        use ed25519_dalek::Verifier as _;
        let bytes = <[u8; 64]>::try_from(signature).ok()?;
        match self {
            Key::Ed25519(key) => key
                .verify_strict(digest, &ed25519_dalek::Signature::from_bytes(&bytes))
                .ok()
                .map(|_| bytes),
            Key::Secp256r1(key) => {
                let parsed = p256::ecdsa::Signature::from_slice(&bytes).ok()?;
                // Sui rejects high-S signatures.
                let parsed = parsed.normalize_s().unwrap_or(parsed);
                key.verify(digest, &parsed).ok()?;
                Some(parsed.to_bytes().into())
            }
        }
    }
}

enum SuiKey {
    Ed25519(sui::types::Ed25519PublicKey),
    Secp256r1(sui::types::Secp256r1PublicKey),
}

impl SuiKey {
    fn address(&self) -> sui::types::Address {
        match self {
            SuiKey::Ed25519(key) => key.derive_address(),
            SuiKey::Secp256r1(key) => key.derive_address(),
        }
    }

    fn user_signature(&self, signature: [u8; 64]) -> sui::types::UserSignature {
        sui::types::UserSignature::Simple(match self {
            SuiKey::Ed25519(public_key) => sui::types::SimpleSignature::Ed25519 {
                signature: sui::types::Ed25519Signature::new(signature),
                public_key: *public_key,
            },
            SuiKey::Secp256r1(public_key) => sui::types::SimpleSignature::Secp256r1 {
                signature: sui::types::Secp256r1Signature::new(signature),
                public_key: *public_key,
            },
        })
    }
}

/// A [`TransactionSigner`] that signs through an HTTP signing service.
///
/// Create it with [`RemoteSigner::connect`]. `Debug` prints the URL host, the
/// address and the scheme only.
pub struct RemoteSigner {
    transport: Transport,
    key: Key,
    public_key: SuiKey,
    address: sui::types::Address,
}

impl std::fmt::Debug for RemoteSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSigner")
            .field(
                "url_host",
                &self.transport.base_url.host_str().unwrap_or("<none>"),
            )
            .field("address", &self.address)
            .field("scheme", &self.key.scheme())
            .finish()
    }
}

fn remote_error(step: &str, error: impl std::fmt::Display) -> NexusError {
    NexusError::RemoteSigner(format!("{step}: {error}"))
}

fn validate_url(url: &str) -> Result<reqwest::Url, NexusError> {
    let invalid = |problem: &str| {
        NexusError::Configuration(format!("remote signer URL is invalid: {problem}"))
    };
    let parsed =
        reqwest::Url::parse(url).map_err(|error| invalid(&format!("not a valid URL ({error})")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(invalid("the scheme must be http or https"));
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err(invalid("a host is required"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(invalid("userinfo (user:password@) is not allowed"));
    }
    if parsed.path() != "/" {
        return Err(invalid("a path is not allowed, use http(s)://host[:port]"));
    }
    if parsed.query().is_some() {
        return Err(invalid("a query is not allowed"));
    }
    if parsed.fragment().is_some() {
        return Err(invalid("a fragment is not allowed"));
    }
    Ok(parsed)
}

/// The `Authorization` header for `token`, marked sensitive so it is never
/// printed by reqwest or http.
fn bearer_header(token: &str) -> Result<reqwest::header::HeaderValue, NexusError> {
    if token.trim().is_empty() {
        return Err(NexusError::Configuration(
            "remote signer bearer token is empty; use None for no token".into(),
        ));
    }
    let mut value =
        reqwest::header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
            NexusError::Configuration(
                "remote signer bearer token is not a valid HTTP header value".into(),
            )
        })?;
    value.set_sensitive(true);
    Ok(value)
}

fn build_client(config: &RemoteSignerConfig) -> Result<reqwest::Client, NexusError> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        // The token must go to `url` and nowhere else, whatever the environment says.
        .no_proxy()
        .timeout(ATTEMPT_TIMEOUT);
    if let Some(token) = &config.bearer_token {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, bearer_header(token)?);
        builder = builder.default_headers(headers);
    }
    if let Some(pem) = &config.extra_root_ca_pem {
        let bad = |problem: String| {
            NexusError::Configuration(format!("remote signer extra root CA: {problem}"))
        };
        let certificates = reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|error| bad(format!("not valid PEM ({})", error_chain(error))))?;
        if certificates.is_empty() {
            return Err(bad("no PEM certificate found".into()));
        }
        for certificate in certificates {
            builder = builder.add_root_certificate(certificate);
        }
    }
    builder.build().map_err(|error| {
        NexusError::Configuration(format!("remote signer HTTP client: {}", error_chain(error)))
    })
}

/// `error` and its sources, `a: b: c`. reqwest's own `Display` is only "error
/// sending request"; the cause (refused, DNS, TLS) is in the sources. The URL is
/// removed and no request headers are part of these errors.
fn error_chain(error: reqwest::Error) -> String {
    let error = error.without_url();
    let mut text = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// At most [`MAX_ERROR_BODY_CHARS`] printable characters of `body`. Control
/// characters are removed first, then the token (should the service echo it) is
/// replaced, so a token split by a control character is still caught. A partial
/// echo of the token cannot be scrubbed.
fn sanitized_prefix(body: &[u8], token: Option<&str>) -> String {
    let mut text: String = String::from_utf8_lossy(body)
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        text = text.replace(token, "<redacted>");
    }
    text.chars().take(MAX_ERROR_BODY_CHARS).collect()
}

fn decode_b64(step: &str, field: &str, value: &serde_json::Value) -> Result<Vec<u8>, NexusError> {
    let text = value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| remote_error(step, format!("response has no string field `{field}`")))?;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| remote_error(step, format!("response field `{field}` is not base64")))
}

impl RemoteSigner {
    /// Connects to the signing service and proves it holds the expected key.
    ///
    /// 1. `GET /v1/public-key`: the scheme and the exact key length are checked,
    ///    the Sui address is derived and compared with
    ///    [`RemoteSignerConfig::expected_address`]. A mismatch fails here, before
    ///    any signing request is made.
    /// 2. `POST /v1/sign` with 32 fresh random bytes: the signature must verify
    ///    locally against the key, which proves the service can sign as that
    ///    address.
    ///
    /// Every later signature is verified locally too. The bearer token, if any,
    /// is sent to `config.url`, in cleartext over `http`: use `https`. See the
    /// [module documentation](self) for the wire protocol.
    pub async fn connect(config: RemoteSignerConfig) -> Result<Self, NexusError> {
        let transport = Transport {
            base_url: validate_url(&config.url)?,
            client: build_client(&config)?,
            token: config.bearer_token,
        };

        let step = "public-key request";
        let response = transport
            .request(step, reqwest::Method::GET, "v1/public-key", None)
            .await?;
        let key = parse_public_key(step, &response)?;
        let public_key = key.sui_public_key();
        let address = public_key.address();
        if address != config.expected_address {
            return Err(NexusError::Configuration(format!(
                "remote signer holds a key for address {address}, expected {}",
                config.expected_address
            )));
        }

        // The challenge: the service must be able to sign as that address.
        let mut challenge = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut challenge);
        sign_digest(&transport, &key, &challenge).await?;

        Ok(Self {
            transport,
            key,
            public_key,
            address,
        })
    }

    /// The signature scheme of the service's key.
    pub fn scheme(&self) -> RemoteSignatureScheme {
        self.key.scheme()
    }

    async fn sign_transaction(
        &self,
        tx: &sui::types::Transaction,
    ) -> Result<sui::types::UserSignature, NexusError> {
        let signature = sign_digest(&self.transport, &self.key, &tx.signing_digest()).await?;
        let signature = self.public_key.user_signature(signature);
        // Never return a signature Sui would reject.
        use sui::crypto::SuiVerifier as _;
        sui::crypto::UserSignatureVerifier::new()
            .verify_transaction(tx, &signature)
            .map_err(|_| {
                remote_error(
                    "sign request",
                    "the remote signature does not verify against the expected key",
                )
            })?;
        Ok(signature)
    }
}

/// The HTTP side: where the service is, and how to talk to it.
struct Transport {
    client: reqwest::Client,
    base_url: reqwest::Url,
    /// Only used to scrub error bodies; never printed.
    token: Option<String>,
}

/// Sends `digest` to `/v1/sign` and returns the locally verified 64-byte signature.
async fn sign_digest(
    transport: &Transport,
    key: &Key,
    digest: &[u8; 32],
) -> Result<[u8; 64], NexusError> {
    let step = "sign request";
    let body = serde_json::json!({
        "digest": base64::engine::general_purpose::STANDARD.encode(digest),
    });
    let response = transport
        .request(step, reqwest::Method::POST, "v1/sign", Some(body))
        .await?;
    let value = parse_json(step, &response)?;
    let signature = decode_b64(step, "signature", &value)?;
    key.checked_signature(digest, &signature).ok_or_else(|| {
        remote_error(
            step,
            "the remote signature does not verify against the expected key",
        )
    })
}

impl Transport {
    /// One logical request: retries connect errors, timeouts and 5xx.
    async fn request(
        &self,
        step: &str,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Vec<u8>, NexusError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|error| remote_error(step, error))?;
        let mut last_error = String::new();
        for attempt in 1..=MAX_ATTEMPTS {
            if attempt > 1 {
                tokio::time::sleep(BACKOFF * (1 << (attempt - 2))).await;
            }
            let mut request = self.client.request(method.clone(), url.clone());
            if let Some(body) = &body {
                request = request.json(body);
            }
            let mut response = match request.send().await {
                Ok(response) => response,
                Err(error) if error.is_connect() || error.is_timeout() => {
                    last_error = error_chain(error);
                    continue;
                }
                Err(error) => {
                    return Err(remote_error(step, error_chain(error)));
                }
            };
            let status = response.status();
            let (bytes, read_error) = match read_body(&mut response).await {
                Ok(read) => (read, None),
                Err(error) => (Vec::new(), Some(error)),
            };
            // Retry only what may be transient: a 5xx, or a timeout on a 2xx.
            // A 4xx is final even if its body cannot be read.
            if read_error.as_ref().is_some_and(|error| {
                status.is_server_error() || (status.is_success() && error.is_timeout())
            }) {
                let error = read_error.expect("checked above");
                last_error = format!("status {status}: {}", error_chain(error));
                continue;
            }
            if status.is_server_error() {
                last_error = format!(
                    "status {status}: {}",
                    sanitized_prefix(&bytes, self.token.as_deref())
                );
                continue;
            }
            if !status.is_success() {
                return Err(remote_error(
                    step,
                    format!(
                        "status {status}: {}",
                        sanitized_prefix(&bytes, self.token.as_deref())
                    ),
                ));
            }
            if let Some(error) = read_error {
                return Err(remote_error(step, error_chain(error)));
            }
            return Ok(bytes);
        }
        Err(remote_error(
            step,
            format!("failed after {MAX_ATTEMPTS} attempts, last error: {last_error}"),
        ))
    }
}

/// Reads at most [`MAX_BODY_BYTES`] of the body and silently drops the rest; a
/// longer body then usually fails to parse, instead of using unbounded memory.
async fn read_body(response: &mut reqwest::Response) -> Result<Vec<u8>, reqwest::Error> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_BODY_BYTES {
            bytes.truncate(MAX_BODY_BYTES);
            break;
        }
    }
    Ok(bytes)
}

fn parse_json(step: &str, bytes: &[u8]) -> Result<serde_json::Value, NexusError> {
    serde_json::from_slice(bytes).map_err(|_| remote_error(step, "response is not valid JSON"))
}

fn parse_public_key(step: &str, bytes: &[u8]) -> Result<Key, NexusError> {
    let value = parse_json(step, bytes)?;
    let scheme = value
        .get("scheme")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| remote_error(step, "response has no string field `scheme`"))?;
    let key = decode_b64(step, "public_key", &value)?;
    let bad = |what: &str| NexusError::Configuration(format!("remote signer public key: {what}"));
    match scheme {
        "ed25519" => {
            let bytes = <[u8; 32]>::try_from(key.as_slice())
                .map_err(|_| bad("an ed25519 key must be 32 bytes"))?;
            ed25519_dalek::VerifyingKey::from_bytes(&bytes)
                .map(Key::Ed25519)
                .map_err(|_| bad("not a valid ed25519 key"))
        }
        "secp256r1" => {
            if key.len() != 33 {
                return Err(bad("a secp256r1 key must be 33 bytes (compressed SEC1)"));
            }
            p256::ecdsa::VerifyingKey::from_sec1_bytes(&key)
                .map(Key::Secp256r1)
                .map_err(|_| bad("not a valid compressed secp256r1 key"))
        }
        // The value is not echoed: it is service-controlled text.
        _ => Err(bad("unsupported scheme; supported: ed25519, secp256r1")),
    }
}

impl TransactionSigner for RemoteSigner {
    fn address(&self) -> sui::types::Address {
        self.address
    }

    fn sign<'a>(
        &'a self,
        tx: &'a sui::types::Transaction,
    ) -> BoxFuture<'a, Result<sui::types::UserSignature, NexusError>> {
        self.sign_transaction(tx).boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        mockito::{Matcher, Server, ServerGuard},
        p256::ecdsa::signature::Signer as _,
        std::sync::Arc,
    };

    const TOKEN: &str = "s3cr3t-token-value";

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn fixture_tx(sender: sui::types::Address) -> sui::types::Transaction {
        crate::nexus::address_balance::finish_transaction(
            sui::types::ProgrammableTransaction {
                inputs: vec![],
                commands: vec![],
            },
            sender,
            1_000_000,
            crate::nexus::address_balance::SubmissionContext {
                reference_gas_price: 1,
                epoch: 1,
                chain: sui::types::Digest::new([9; 32]),
            },
            1,
        )
    }

    #[derive(Clone)]
    enum TestKey {
        Ed25519(ed25519_dalek::SigningKey),
        P256(p256::ecdsa::SigningKey),
    }

    impl TestKey {
        fn ed25519(seed: u8) -> Self {
            Self::Ed25519(ed25519_dalek::SigningKey::from_bytes(&[seed; 32]))
        }

        fn p256(seed: u8) -> Self {
            Self::P256(p256::ecdsa::SigningKey::from_bytes(&[seed; 32].into()).unwrap())
        }

        fn scheme_name(&self) -> &'static str {
            match self {
                Self::Ed25519(_) => "ed25519",
                Self::P256(_) => "secp256r1",
            }
        }

        fn public_key(&self) -> Vec<u8> {
            match self {
                Self::Ed25519(key) => key.verifying_key().to_bytes().to_vec(),
                Self::P256(key) => key
                    .verifying_key()
                    .to_encoded_point(true)
                    .as_bytes()
                    .to_vec(),
            }
        }

        fn address(&self) -> sui::types::Address {
            match self {
                Self::Ed25519(key) => {
                    sui::types::Ed25519PublicKey::new(key.verifying_key().to_bytes())
                        .derive_address()
                }
                Self::P256(_) => sui::types::Secp256r1PublicKey::new(
                    <[u8; 33]>::try_from(self.public_key().as_slice()).unwrap(),
                )
                .derive_address(),
            }
        }

        fn sign(&self, digest: &[u8], high_s: bool) -> Vec<u8> {
            match self {
                Self::Ed25519(key) => ed25519_dalek::Signer::sign(key, digest).to_bytes().to_vec(),
                Self::P256(key) => {
                    let signature: p256::ecdsa::Signature = key.sign(digest);
                    let signature = signature.normalize_s().unwrap_or(signature);
                    if high_s {
                        let (r, s) = signature.split_scalars();
                        p256::ecdsa::Signature::from_scalars(*r, -*s)
                            .unwrap()
                            .to_bytes()
                            .to_vec()
                    } else {
                        signature.to_bytes().to_vec()
                    }
                }
            }
        }
    }

    fn pubkey_body(key: &TestKey) -> String {
        serde_json::json!({"scheme": key.scheme_name(), "public_key": b64(&key.public_key())})
            .to_string()
    }

    fn digest_of_request(request: &mockito::Request) -> Vec<u8> {
        let body: serde_json::Value = serde_json::from_slice(request.body().unwrap()).unwrap();
        base64::engine::general_purpose::STANDARD
            .decode(body["digest"].as_str().unwrap())
            .unwrap()
    }

    fn sign_body(
        key: TestKey,
        high_s: bool,
    ) -> impl Fn(&mockito::Request) -> Vec<u8> + Send + Sync {
        move |request| {
            let signature = key.sign(&digest_of_request(request), high_s);
            serde_json::json!({ "signature": b64(&signature) })
                .to_string()
                .into_bytes()
        }
    }

    async fn mock_public_key(server: &mut ServerGuard, key: &TestKey) -> mockito::Mock {
        server
            .mock("GET", "/v1/public-key")
            .with_header("content-type", "application/json")
            .with_body(pubkey_body(key))
            .create_async()
            .await
    }

    fn config(server: &ServerGuard, key: &TestKey, token: Option<&str>) -> RemoteSignerConfig {
        RemoteSignerConfig {
            url: server.url(),
            bearer_token: token.map(str::to_owned),
            expected_address: key.address(),
            extra_root_ca_pem: None,
        }
    }

    const P256_ORDER_HALF: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31,
        0x92, 0xa8,
    ];

    #[tokio::test]
    async fn connect_and_sign_ed25519() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        let public = server
            .mock("GET", "/v1/public-key")
            .match_header("authorization", format!("Bearer {TOKEN}").as_str())
            .with_body(pubkey_body(&key))
            .expect(1)
            .create_async()
            .await;
        let sign = server
            .mock("POST", "/v1/sign")
            .match_header("authorization", format!("Bearer {TOKEN}").as_str())
            .match_header("content-type", "application/json")
            .with_body_from_request(sign_body(key.clone(), false))
            .expect(2) // the challenge, then the transaction
            .create_async()
            .await;

        let signer = RemoteSigner::connect(config(&server, &key, Some(TOKEN)))
            .await
            .unwrap();
        assert_eq!(signer.scheme(), RemoteSignatureScheme::Ed25519);
        assert_eq!(TransactionSigner::address(&signer), key.address());

        let tx = fixture_tx(key.address());
        let signature = TransactionSigner::sign(&signer, &tx).await.unwrap();
        use sui::crypto::SuiVerifier as _;
        sui::crypto::UserSignatureVerifier::new()
            .verify_transaction(&tx, &signature)
            .unwrap();
        public.assert_async().await;
        sign.assert_async().await;
    }

    #[tokio::test]
    async fn connect_and_sign_secp256r1() {
        let key = TestKey::p256(5);
        let mut server = Server::new_async().await;
        mock_public_key(&mut server, &key).await;
        // The service answers with a high-S signature every time.
        server
            .mock("POST", "/v1/sign")
            .with_body_from_request(sign_body(key.clone(), true))
            .create_async()
            .await;
        // Sanity: the fake really produces high-S.
        assert!(key.sign(&[1; 32], true)[32..] > P256_ORDER_HALF[..]);

        let signer = RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap();
        assert_eq!(signer.scheme(), RemoteSignatureScheme::Secp256r1);
        let tx = fixture_tx(key.address());
        let signature = TransactionSigner::sign(&signer, &tx).await.unwrap();
        use sui::crypto::SuiVerifier as _;
        sui::crypto::UserSignatureVerifier::new()
            .verify_transaction(&tx, &signature)
            .unwrap();
        let sui::types::UserSignature::Simple(sui::types::SimpleSignature::Secp256r1 {
            signature,
            ..
        }) = signature
        else {
            panic!("expected a Secp256r1 signature");
        };
        assert!(AsRef::<[u8]>::as_ref(&signature)[32..] <= P256_ORDER_HALF[..]);
    }

    #[tokio::test]
    async fn connect_without_token_sends_no_authorization() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        let public = server
            .mock("GET", "/v1/public-key")
            .match_header("authorization", Matcher::Missing)
            .with_body(pubkey_body(&key))
            .expect(1)
            .create_async()
            .await;
        let sign = server
            .mock("POST", "/v1/sign")
            .match_header("authorization", Matcher::Missing)
            .with_body_from_request(sign_body(key.clone(), false))
            .expect(1)
            .create_async()
            .await;
        RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap();
        public.assert_async().await;
        sign.assert_async().await;
    }

    #[tokio::test]
    async fn connect_rejects_wrong_address() {
        let key = TestKey::ed25519(7);
        let other = TestKey::ed25519(8);
        let mut server = Server::new_async().await;
        mock_public_key(&mut server, &key).await;
        let sign = server
            .mock("POST", "/v1/sign")
            .expect(0)
            .create_async()
            .await;
        let mut cfg = config(&server, &key, None);
        cfg.expected_address = other.address();
        let error = RemoteSigner::connect(cfg).await.unwrap_err();
        let NexusError::Configuration(message) = &error else {
            panic!("expected Configuration, got {error:?}");
        };
        assert!(message.contains(&key.address().to_string()));
        assert!(message.contains(&other.address().to_string()));
        sign.assert_async().await;
    }

    #[tokio::test]
    async fn connect_rejects_bad_challenge_signature() {
        let key = TestKey::ed25519(7);
        let impostor = TestKey::ed25519(9);
        let mut server = Server::new_async().await;
        mock_public_key(&mut server, &key).await;
        server
            .mock("POST", "/v1/sign")
            .with_body_from_request(sign_body(impostor, false))
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not verify against the expected key"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn sign_rejects_signature_that_does_not_verify() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        mock_public_key(&mut server, &key).await;
        // Good for the challenge, wrong afterwards.
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let good = key.clone();
        let bad = TestKey::ed25519(9);
        let counter = calls.clone();
        server
            .mock("POST", "/v1/sign")
            .with_body_from_request(move |request| {
                let which = if counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    &good
                } else {
                    &bad
                };
                let signature = which.sign(&digest_of_request(request), false);
                serde_json::json!({ "signature": b64(&signature) })
                    .to_string()
                    .into_bytes()
            })
            .create_async()
            .await;
        let signer = RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap();
        let error = TransactionSigner::sign(&signer, &fixture_tx(key.address()))
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not verify against the expected key"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn rejects_bad_public_key_scheme_or_length() {
        let key = TestKey::ed25519(7);
        let p256 = TestKey::p256(5);
        let cases = [
            serde_json::json!({"scheme": "rsa", "public_key": b64(&key.public_key())}),
            serde_json::json!({"scheme": "ed25519", "public_key": b64(&key.public_key()[..31])}),
            serde_json::json!({"scheme": "ed25519", "public_key": b64(&[key.public_key(), vec![0]].concat())}),
            // A secp256r1 scheme with an ed25519-length key, and a truncated one.
            serde_json::json!({"scheme": "secp256r1", "public_key": b64(&key.public_key())}),
            serde_json::json!({"scheme": "secp256r1", "public_key": b64(&p256.public_key()[..32])}),
            serde_json::json!({"scheme": "ed25519"}),
            serde_json::json!({"scheme": "ed25519", "public_key": "!!not base64!!"}),
        ];
        for case in cases {
            let mut server = Server::new_async().await;
            server
                .mock("GET", "/v1/public-key")
                .with_body(case.to_string())
                .create_async()
                .await;
            let sign = server
                .mock("POST", "/v1/sign")
                .expect(0)
                .create_async()
                .await;
            let result = RemoteSigner::connect(config(&server, &key, None)).await;
            assert!(result.is_err(), "accepted {case}");
            sign.assert_async().await;
        }
    }

    #[tokio::test]
    async fn rejects_bad_secp256r1_keys_by_the_right_check() {
        let key = TestKey::p256(5);
        let TestKey::P256(signing) = &key else {
            unreachable!()
        };
        let uncompressed = signing.verifying_key().to_encoded_point(false);
        assert_eq!(uncompressed.as_bytes().len(), 65);
        // 33 bytes, but x is not a field element, so it is no point at all.
        let mut off_curve = vec![0x02];
        off_curve.extend_from_slice(&[0xff; 32]);
        for (public_key, expected) in [
            (uncompressed.as_bytes().to_vec(), "must be 33 bytes"),
            (off_curve, "not a valid compressed secp256r1 key"),
        ] {
            let mut server = Server::new_async().await;
            server
                .mock("GET", "/v1/public-key")
                .with_body(
                    serde_json::json!({"scheme": "secp256r1", "public_key": b64(&public_key)})
                        .to_string(),
                )
                .create_async()
                .await;
            let sign = server
                .mock("POST", "/v1/sign")
                .expect(0)
                .create_async()
                .await;
            let error = RemoteSigner::connect(config(&server, &key, None))
                .await
                .unwrap_err();
            assert!(error.to_string().contains(expected), "{expected}: {error}");
            sign.assert_async().await;
        }
    }

    #[tokio::test]
    async fn non_2xx_reports_status_without_echoing_body_headers() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        server
            .mock("GET", "/v1/public-key")
            .with_status(403)
            .with_body(format!(
                "denied\nAuthorization: Bearer {TOKEN}\n{}",
                "x".repeat(500)
            ))
            .expect(1)
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, Some(TOKEN)))
            .await
            .unwrap_err();
        let rendered = format!("{error} {error:?}");
        assert!(rendered.contains("403"), "{rendered}");
        assert!(rendered.contains("public-key request"), "{rendered}");
        assert!(!rendered.contains(TOKEN), "{rendered}");
        assert!(!rendered.contains(&"x".repeat(200)), "{rendered}");
    }

    #[tokio::test]
    async fn retries_5xx_then_succeeds() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        let failing = server
            .mock("GET", "/v1/public-key")
            .with_status(503)
            .expect(2)
            .create_async()
            .await;
        let ok = server
            .mock("GET", "/v1/public-key")
            .with_body(pubkey_body(&key))
            .expect(1)
            .create_async()
            .await;
        server
            .mock("POST", "/v1/sign")
            .with_body_from_request(sign_body(key.clone(), false))
            .create_async()
            .await;
        RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap();
        failing.assert_async().await;
        ok.assert_async().await;
    }

    #[tokio::test]
    async fn gives_up_after_three_5xx_attempts() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        let failing = server
            .mock("GET", "/v1/public-key")
            .with_status(500)
            .expect(3)
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("3 attempts"), "{error}");
        failing.assert_async().await;
    }

    #[tokio::test]
    async fn does_not_retry_4xx() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        let denied = server
            .mock("GET", "/v1/public-key")
            .with_status(401)
            .expect(1)
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("401"), "{error}");
        denied.assert_async().await;
    }

    #[tokio::test]
    async fn rejects_bad_urls() {
        let key = TestKey::ed25519(7);
        for url in [
            "",
            "not a url",
            "ftp://host",
            "https://",
            "https://user:pw@host",
            "https://user@host",
            "https://host/path",
            "https://host/v1",
            "https://host?x=1",
            "https://host/#frag",
            "https://host#frag",
            "file:///etc/passwd",
        ] {
            let error = RemoteSigner::connect(RemoteSignerConfig {
                url: url.into(),
                bearer_token: Some(TOKEN.into()),
                expected_address: key.address(),
                extra_root_ca_pem: None,
            })
            .await
            .unwrap_err();
            assert!(
                matches!(error, NexusError::Configuration(_)),
                "{url}: {error:?}"
            );
            let rendered = error.to_string();
            assert!(!rendered.contains(TOKEN), "{url}: {rendered}");
            assert!(!rendered.contains("pw"), "{url}: {rendered}");
        }
    }

    #[test]
    fn accepts_plain_and_port_urls() {
        for url in [
            "http://host",
            "https://host:8790",
            "https://host:8790/",
            "http://127.0.0.1:1",
        ] {
            validate_url(url).unwrap_or_else(|error| panic!("{url}: {error}"));
        }
    }

    #[tokio::test]
    async fn debug_redacts_token() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        mock_public_key(&mut server, &key).await;
        server
            .mock("POST", "/v1/sign")
            .with_body_from_request(sign_body(key.clone(), false))
            .create_async()
            .await;
        let cfg = config(&server, &key, Some(TOKEN));
        let rendered = format!("{cfg:?}");
        assert!(!rendered.contains(TOKEN));
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("127.0.0.1"));
        let signer = RemoteSigner::connect(cfg).await.unwrap();
        let rendered = format!("{signer:?}");
        assert!(!rendered.contains(TOKEN));
        assert!(rendered.contains("127.0.0.1"));
        assert!(rendered.contains(&key.address().to_string()));
        assert!(rendered.contains("Ed25519"));
    }

    #[tokio::test]
    async fn extra_root_ca_without_a_certificate_is_rejected() {
        let key = TestKey::ed25519(7);
        for pem in [
            b"".to_vec(),
            b"not pem at all".to_vec(),
            vec![0x30, 0x82, 0x01, 0x00], // DER, not PEM
            b"-----BEGIN CERTIFICATE-----\nnot a cert\n".to_vec(),
            b"-----BEGIN CERTIFICATE-----\n!!!!\n-----END CERTIFICATE-----\n".to_vec(),
        ] {
            let error = RemoteSigner::connect(RemoteSignerConfig {
                url: "https://127.0.0.1:1".into(),
                bearer_token: None,
                expected_address: key.address(),
                extra_root_ca_pem: Some(pem.clone()),
            })
            .await
            .unwrap_err();
            assert!(
                matches!(error, NexusError::Configuration(_)),
                "{pem:?}: {error:?}"
            );
            assert!(error.to_string().contains("extra root CA"), "{error}");
        }
    }

    #[tokio::test]
    async fn alternate_format_prints_each_cause_once() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        server
            .mock("GET", "/v1/public-key")
            .with_status(403)
            .with_body("uniquemarker")
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap_err();
        assert_eq!(format!("{error:#}").matches("uniquemarker").count(), 1);
    }

    #[tokio::test]
    async fn refused_connection_reports_the_cause() {
        let key = TestKey::ed25519(7);
        let error = RemoteSigner::connect(RemoteSignerConfig {
            url: "http://127.0.0.1:1".into(),
            bearer_token: Some(TOKEN.into()),
            expected_address: key.address(),
            extra_root_ca_pem: None,
        })
        .await
        .unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("3 attempts"), "{rendered}");
        assert!(rendered.to_lowercase().contains("refused"), "{rendered}");
        assert!(!rendered.contains(TOKEN), "{rendered}");
    }

    #[tokio::test]
    async fn tls_failure_is_distinguishable_from_refused() {
        // https against a plain-http server: the handshake fails, nothing is refused.
        let key = TestKey::ed25519(7);
        let server = Server::new_async().await;
        let url = server.url().replace("http://", "https://");
        let error = RemoteSigner::connect(RemoteSignerConfig {
            url,
            bearer_token: None,
            expected_address: key.address(),
            extra_root_ca_pem: None,
        })
        .await
        .unwrap_err();
        let rendered = error.to_string();
        assert!(!rendered.to_lowercase().contains("refused"), "{rendered}");
        assert!(rendered.len() > "remote signer public-key request: failed after 3 attempts, last error: error sending request".len(), "{rendered}");
    }

    #[tokio::test]
    async fn scheme_value_is_not_echoed() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        server
            .mock("GET", "/v1/public-key")
            .with_body(
                serde_json::json!({"scheme": format!("Bearer {TOKEN}"), "public_key": b64(&key.public_key())})
                    .to_string(),
            )
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, Some(TOKEN)))
            .await
            .unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("unsupported scheme"), "{rendered}");
        assert!(!rendered.contains(TOKEN), "{rendered}");
    }

    #[tokio::test]
    async fn token_split_by_a_control_character_is_still_scrubbed() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        let (head, tail) = TOKEN.split_at(6);
        server
            .mock("GET", "/v1/public-key")
            .with_status(403)
            .with_body(format!("bad\u{7}\n\tcredential {head}\u{7}{tail}!"))
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, Some(TOKEN)))
            .await
            .unwrap_err();
        let rendered = error.to_string();
        assert!(rendered.contains("<redacted>"), "{rendered}");
        assert!(!rendered.contains(TOKEN), "{rendered}");
        assert!(!rendered.contains(head), "{rendered}");
        assert!(!rendered.chars().any(char::is_control), "{rendered}");
    }

    #[test]
    fn bearer_header_is_sensitive() {
        assert!(bearer_header(TOKEN).unwrap().is_sensitive());
        assert_eq!(
            bearer_header(TOKEN).unwrap(),
            format!("Bearer {TOKEN}").as_str()
        );
    }

    #[tokio::test]
    async fn empty_or_blank_token_is_rejected() {
        let key = TestKey::ed25519(7);
        for token in ["", " ", "\t "] {
            let error = RemoteSigner::connect(RemoteSignerConfig {
                url: "http://127.0.0.1:1".into(),
                bearer_token: Some(token.into()),
                expected_address: key.address(),
                extra_root_ca_pem: None,
            })
            .await
            .unwrap_err();
            assert!(
                matches!(error, NexusError::Configuration(_)),
                "{token:?}: {error:?}"
            );
            assert!(error.to_string().contains("empty"), "{error}");
        }
    }

    #[tokio::test]
    async fn redirects_are_not_followed() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        server
            .mock("GET", "/v1/public-key")
            .with_status(302)
            .with_header("location", "/elsewhere")
            .expect(1)
            .create_async()
            .await;
        let elsewhere = server
            .mock("GET", "/elsewhere")
            .expect(0)
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, Some(TOKEN)))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("302"), "{error}");
        elsewhere.assert_async().await;
    }

    #[tokio::test]
    async fn challenge_is_fresh_and_random() {
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        mock_public_key(&mut server, &key).await;
        let seen = Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
        let recorder = seen.clone();
        let signing = key.clone();
        server
            .mock("POST", "/v1/sign")
            .expect(2)
            .with_body_from_request(move |request| {
                let digest = digest_of_request(request);
                recorder.lock().unwrap().push(digest.clone());
                serde_json::json!({ "signature": b64(&signing.sign(&digest, false)) })
                    .to_string()
                    .into_bytes()
            })
            .create_async()
            .await;
        for _ in 0..2 {
            RemoteSigner::connect(config(&server, &key, None))
                .await
                .unwrap();
        }
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].len(), 32);
        assert_ne!(seen[0], seen[1]);
        assert!(seen.iter().all(|digest| digest.iter().any(|b| *b != 0)));
    }

    #[tokio::test]
    async fn client_error_with_unreadable_body_is_not_retried() {
        // A 4xx must be final even when its body cannot be read.
        let key = TestKey::ed25519(7);
        let mut server = Server::new_async().await;
        let denied = server
            .mock("GET", "/v1/public-key")
            .with_status(403)
            .with_chunked_body(|_| Err(std::io::Error::other("body cut")))
            .expect(1)
            .create_async()
            .await;
        let error = RemoteSigner::connect(config(&server, &key, None))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("403"), "{error}");
        denied.assert_async().await;
    }
}
