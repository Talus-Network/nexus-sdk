//! [`TransactionSigner`] backed by a Google Cloud KMS asymmetric key version.
//!
//! The private key never leaves KMS and every signature is gated by IAM.
//! Supported key algorithms are the ones Sui can verify and KMS can produce:
//!
//! - `EC_SIGN_ED25519` (Cloud KMS software protection): PureEdDSA over the
//!   32-byte Sui signing digest, sent to KMS as `data`.
//! - `EC_SIGN_P256_SHA256`: Sui's secp256r1 scheme hashes the signing digest
//!   with SHA-256 internally, so KMS signs `digest = sha256(signing_digest)`.
//!   The DER signature is converted to `r || s` and normalized to low-S, which
//!   Sui requires.
//!
//! Credentials come from Application Default Credentials (for example a
//! Workload Identity Federation `external_account` file referenced by
//! `GOOGLE_APPLICATION_CREDENTIALS`), handled by the Google client library.
//! Where the process cannot obtain credentials itself, [`GcpKmsSigner::via_proxy`]
//! sends the same calls, without credentials, to a proxy that authenticates them.
//! Requests and responses are protected with CRC32C checks, and every
//! signature is verified locally before it is returned.

use {
    crate::{
        nexus::{error::NexusError, transaction_signer::TransactionSigner},
        sui,
    },
    base64::Engine as _,
    futures::future::{BoxFuture, FutureExt as _},
    sha2::{Digest as _, Sha256},
    std::sync::Arc,
};

/// DER prefix of an Ed25519 `SubjectPublicKeyInfo`; the 32 key bytes follow.
const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// Key algorithms of a KMS key version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum KmsAlgorithm {
    Ed25519,
    P256Sha256,
    Unsupported(String),
}

/// The payload KMS is asked to sign.
pub(crate) enum SignInput {
    /// Raw message (PureEdDSA).
    Data(Vec<u8>),
    /// SHA-256 digest of the message (ECDSA).
    Sha256Digest([u8; 32]),
}

impl SignInput {
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            SignInput::Data(data) => data,
            SignInput::Sha256Digest(digest) => digest,
        }
    }
}

pub(crate) struct PublicKeyResp {
    pub pem: String,
    pub algorithm: KmsAlgorithm,
    pub pem_crc32c: Option<i64>,
}

pub(crate) struct SignResp {
    pub name: String,
    pub signature: Vec<u8>,
    pub signature_crc32c: Option<i64>,
    /// KMS verified the CRC32C of the data or digest we sent.
    pub verified_input_crc32c: bool,
}

/// The slice of the KMS API the signer uses; the seam for tests.
pub(crate) trait KmsApi: Send + Sync {
    fn get_public_key<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<PublicKeyResp, NexusError>>;

    fn asymmetric_sign<'a>(
        &'a self,
        name: &'a str,
        input: SignInput,
        input_crc32c: i64,
    ) -> BoxFuture<'a, Result<SignResp, NexusError>>;
}

fn kms_error(context: &str, error: impl std::fmt::Display) -> NexusError {
    NexusError::Wallet(anyhow::anyhow!("Cloud KMS {context}: {error}"))
}

/// Upper bound on a single KMS request; gax retries within its overall limit.
const KMS_ATTEMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// CRC32C (Castagnoli) as KMS uses it for request and response integrity.
fn kms_crc32c(bytes: &[u8]) -> i64 {
    i64::from(crc32c::crc32c(bytes))
}

/// Installs rustls' ring provider as the process default unless one is set.
///
/// The Google crates are built without a provider of their own, so the KMS
/// client needs a process default. ring is what rustls selects on its own in
/// builds without `gcp-kms`, so other TLS clients in the process are unaffected.
fn install_default_crypto_provider() {
    // `Err` only means another provider is already installed, which is fine.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// The real KMS client, built once and shared by all signing calls.
///
/// No `Debug`: the client's `Debug` prints its endpoint, which for a proxy
/// carries a secret path token.
struct GoogleKms {
    client: google_cloud_kms_v1::client::KeyManagementService,
    /// Path of the proxy URL, a secret token that must never appear in an
    /// error; the client's own errors quote the request URL. `None` when the
    /// calls go straight to Cloud KMS.
    secret_proxy_path: Option<String>,
}

/// Replaces every occurrence of `secret_path` in `text` with a placeholder.
/// A canonical proxy path is sent exactly as configured, so a plain
/// replacement is complete. An empty path is nothing to hide.
fn redact_secret_path(secret_path: Option<&str>, text: String) -> String {
    match secret_path {
        Some(path) if !path.is_empty() => text.replace(path, "<redacted>"),
        _ => text,
    }
}

/// Where the KMS client sends its calls and how it authenticates them.
enum KmsTarget {
    /// Cloud KMS itself, authenticated with Application Default Credentials,
    /// or with `credentials_instead_of_adc` when set (tests only).
    DirectToCloudKms {
        credentials_instead_of_adc: Option<google_cloud_auth::credentials::Credentials>,
    },
    /// An authenticating proxy at this validated base URL. The client sends
    /// no credentials; the proxy adds them.
    AuthenticatingProxy { proxy_url: String },
}

impl GoogleKms {
    /// Builds the client for `target`.
    async fn connect(target: KmsTarget) -> Result<Self, NexusError> {
        install_default_crypto_provider();
        let builder = google_cloud_kms_v1::client::KeyManagementService::builder();
        let (builder, secret_proxy_path, description) = match target {
            KmsTarget::DirectToCloudKms {
                credentials_instead_of_adc,
            } => {
                let builder = match credentials_instead_of_adc {
                    Some(credentials) => builder.with_credentials(credentials),
                    None => builder,
                };
                (builder, None, "Application Default Credentials")
            }
            KmsTarget::AuthenticatingProxy { proxy_url } => {
                let path = proxy_url_path(&proxy_url).to_owned();
                let anonymous = google_cloud_auth::credentials::anonymous::Builder::new().build();
                (
                    builder.with_endpoint(proxy_url).with_credentials(anonymous),
                    Some(path),
                    "KMS proxy",
                )
            }
        };
        let client = builder.build().await.map_err(|error| {
            NexusError::Configuration(format!(
                "could not create the Cloud KMS client ({description}): {}",
                redact_secret_path(secret_proxy_path.as_deref(), error.to_string())
            ))
        })?;
        Ok(Self {
            client,
            secret_proxy_path,
        })
    }

    fn error(&self, context: &str, error: impl std::fmt::Display) -> NexusError {
        kms_error(
            context,
            redact_secret_path(self.secret_proxy_path.as_deref(), error.to_string()),
        )
    }
}

impl KmsApi for GoogleKms {
    fn get_public_key<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<PublicKeyResp, NexusError>> {
        use {
            google_cloud_gax::options::RequestOptionsBuilder as _,
            google_cloud_kms_v1::model::crypto_key_version::CryptoKeyVersionAlgorithm as Algorithm,
        };
        async move {
            let response = self
                .client
                .get_public_key()
                .set_name(name)
                .with_idempotency(true)
                .with_attempt_timeout(KMS_ATTEMPT_TIMEOUT)
                .send()
                .await
                .map_err(|error| self.error("GetPublicKey failed", error))?;
            let algorithm = if response.algorithm == Algorithm::EcSignEd25519 {
                KmsAlgorithm::Ed25519
            } else if response.algorithm == Algorithm::EcSignP256Sha256 {
                KmsAlgorithm::P256Sha256
            } else {
                KmsAlgorithm::Unsupported(
                    response
                        .algorithm
                        .name()
                        .map(str::to_owned)
                        .unwrap_or_else(|| "unknown".to_owned()),
                )
            };
            Ok(PublicKeyResp {
                pem: response.pem,
                algorithm,
                pem_crc32c: response.pem_crc32c,
            })
        }
        .boxed()
    }

    fn asymmetric_sign<'a>(
        &'a self,
        name: &'a str,
        input: SignInput,
        input_crc32c: i64,
    ) -> BoxFuture<'a, Result<SignResp, NexusError>> {
        use google_cloud_gax::options::RequestOptionsBuilder as _;
        async move {
            // Signing has no side effects, so transport failures and 429/503
            // after the request was sent are safe to retry.
            let request = self
                .client
                .asymmetric_sign()
                .set_name(name)
                .with_idempotency(true)
                .with_attempt_timeout(KMS_ATTEMPT_TIMEOUT);
            let request = match input {
                SignInput::Data(data) => request.set_data(data).set_data_crc32c(input_crc32c),
                SignInput::Sha256Digest(digest) => request
                    .set_digest(
                        google_cloud_kms_v1::model::Digest::new().set_sha256(digest.to_vec()),
                    )
                    .set_digest_crc32c(input_crc32c),
            };
            let response = request
                .send()
                .await
                .map_err(|error| self.error("AsymmetricSign failed", error))?;
            Ok(SignResp {
                name: response.name,
                signature: response.signature.to_vec(),
                signature_crc32c: response.signature_crc32c,
                verified_input_crc32c: response.verified_data_crc32c
                    || response.verified_digest_crc32c,
            })
        }
        .boxed()
    }
}

/// Signs Sui transactions with a Cloud KMS asymmetric key version.
///
/// The public key is fetched once in [`Self::new`] or [`Self::via_proxy`] and the address derived from
/// it; the KMS client is built once and shared by concurrent signing calls.
pub struct GcpKmsSigner {
    key_version: String,
    api: Arc<dyn KmsApi>,
    algorithm: KmsAlgorithm,
    public_key: PublicKey,
    address: sui::types::Address,
}

#[derive(Clone)]
enum PublicKey {
    Ed25519(sui::types::Ed25519PublicKey),
    Secp256r1(sui::types::Secp256r1PublicKey),
}

impl std::fmt::Debug for GcpKmsSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcpKmsSigner")
            .field("key_version", &self.key_version)
            .field("address", &self.address)
            .finish()
    }
}

impl GcpKmsSigner {
    /// Connects with Application Default Credentials and loads the public key of
    /// `key_version`, a pinned key version resource name:
    /// `projects/<p>/locations/<l>/keyRings/<r>/cryptoKeys/<k>/cryptoKeyVersions/<n>`.
    ///
    /// Installs rustls' ring provider as the process default when none is
    /// installed yet; the Google client libraries are built without one.
    pub async fn new(key_version: impl Into<String>) -> Result<Self, NexusError> {
        let key_version = key_version.into();
        validate_key_version(&key_version)?;
        let api = GoogleKms::connect(KmsTarget::DirectToCloudKms {
            credentials_instead_of_adc: None,
        })
        .await?;
        Self::with_api(key_version, Arc::new(api)).await
    }

    /// Like [`Self::new`], but sends the Cloud KMS REST calls to an
    /// authenticating proxy at `proxy_url` instead of `cloudkms.googleapis.com`,
    /// and sends no credentials itself.
    ///
    /// The proxy must forward the `GetPublicKey` (`GET /v1/<key_version>/publicKey`)
    /// and `AsymmetricSign` (`POST /v1/<key_version>:asymmetricSign`) calls to
    /// Cloud KMS unchanged and add the credentials. Everything else behaves as
    /// in [`Self::new`]: the public key is fetched and CRC32C-checked once, and
    /// every signature is verified locally against that pinned key, so a proxy
    /// cannot make the signer return a signature from a different key.
    ///
    /// `proxy_url` is a canonical base URL, and anything else is rejected with
    /// [`NexusError::Configuration`]: a lowercase `http` or `https` scheme; a
    /// host that is a DNS name (`[A-Za-z0-9.-]`) or a dotted IPv4 address; an
    /// optional port 1-65535; no userinfo, query, fragment or whitespace; and a
    /// path that is empty or made of `/segment`s with segments of
    /// `[A-Za-z0-9_~-]+`. The path may be a secret token
    /// (`http://kms-proxy:8790/<token>`); the client appends `/v1/...` to it.
    ///
    /// This SDK keeps the URL out of its own errors and `Debug` output. The
    /// Google client's opt-in tracing (`GOOGLE_CLOUD_RUST_LOGGING=true`)
    /// records request URLs, so keep it off where the URL carries a token.
    pub async fn via_proxy(
        key_version: impl Into<String>,
        proxy_url: impl Into<String>,
    ) -> Result<Self, NexusError> {
        let key_version = key_version.into();
        validate_key_version(&key_version)?;
        let proxy_url = proxy_url.into();
        validate_proxy_url(&proxy_url)?;
        let api = GoogleKms::connect(KmsTarget::AuthenticatingProxy { proxy_url }).await?;
        Self::with_api(key_version, Arc::new(api)).await
    }

    pub(crate) async fn with_api(
        key_version: impl Into<String>,
        api: Arc<dyn KmsApi>,
    ) -> Result<Self, NexusError> {
        let key_version = key_version.into();
        validate_key_version(&key_version)?;
        let response = api.get_public_key(&key_version).await?;
        if let KmsAlgorithm::Unsupported(name) = &response.algorithm {
            return Err(NexusError::Configuration(format!(
                "unsupported KMS key algorithm {name}; supported: EC_SIGN_ED25519, EC_SIGN_P256_SHA256"
            )));
        }
        if response.pem_crc32c != Some(kms_crc32c(response.pem.as_bytes())) {
            return Err(kms_error(
                "GetPublicKey response",
                "missing or failed its CRC32C integrity check",
            ));
        }
        let der = pem_to_der(&response.pem)?;
        let public_key = match response.algorithm {
            KmsAlgorithm::Ed25519 => {
                let key = der
                    .strip_prefix(&ED25519_SPKI_PREFIX)
                    .and_then(|key| <[u8; 32]>::try_from(key).ok())
                    .ok_or_else(|| {
                        NexusError::Configuration(
                            "KMS public key is not an Ed25519 SubjectPublicKeyInfo".into(),
                        )
                    })?;
                PublicKey::Ed25519(sui::types::Ed25519PublicKey::new(key))
            }
            KmsAlgorithm::P256Sha256 => {
                use p256::pkcs8::DecodePublicKey as _;
                let key = p256::PublicKey::from_public_key_der(&der).map_err(|_| {
                    NexusError::Configuration("KMS public key is not a P-256 key".into())
                })?;
                let compressed = <[u8; 33]>::try_from(
                    p256::elliptic_curve::sec1::ToEncodedPoint::to_encoded_point(&key, true)
                        .as_bytes(),
                )
                .map_err(|_| NexusError::Configuration("unexpected P-256 key length".into()))?;
                PublicKey::Secp256r1(sui::types::Secp256r1PublicKey::new(compressed))
            }
            KmsAlgorithm::Unsupported(_) => unreachable!("rejected above"),
        };
        let address = match &public_key {
            PublicKey::Ed25519(key) => key.derive_address(),
            PublicKey::Secp256r1(key) => key.derive_address(),
        };
        Ok(Self {
            key_version,
            api,
            algorithm: response.algorithm,
            public_key,
            address,
        })
    }

    /// The KMS key version resource name this signer uses.
    pub fn key_version(&self) -> &str {
        &self.key_version
    }

    async fn sign_transaction(
        &self,
        tx: &sui::types::Transaction,
    ) -> Result<sui::types::UserSignature, NexusError> {
        let digest = tx.signing_digest();
        let input = match self.algorithm {
            KmsAlgorithm::Ed25519 => SignInput::Data(digest.to_vec()),
            _ => SignInput::Sha256Digest(Sha256::digest(digest).into()),
        };
        let input_crc32c = kms_crc32c(input.bytes());
        let response = self
            .api
            .asymmetric_sign(&self.key_version, input, input_crc32c)
            .await?;
        if response.name != self.key_version {
            return Err(kms_error(
                "AsymmetricSign response",
                "signed with a different key version than requested",
            ));
        }
        if !response.verified_input_crc32c {
            return Err(kms_error(
                "AsymmetricSign response",
                "did not verify the request CRC32C (request corrupted in transit)",
            ));
        }
        if response.signature_crc32c != Some(kms_crc32c(&response.signature)) {
            return Err(kms_error(
                "AsymmetricSign response",
                "signature is missing its CRC32C or failed the integrity check",
            ));
        }
        let signature = match &self.public_key {
            PublicKey::Ed25519(public_key) => {
                let bytes = <[u8; 64]>::try_from(response.signature.as_slice()).map_err(|_| {
                    kms_error("AsymmetricSign response", "malformed Ed25519 signature")
                })?;
                sui::types::SimpleSignature::Ed25519 {
                    signature: sui::types::Ed25519Signature::new(bytes),
                    public_key: *public_key,
                }
            }
            PublicKey::Secp256r1(public_key) => {
                let parsed = p256::ecdsa::Signature::from_der(&response.signature)
                    .map_err(|_| kms_error("AsymmetricSign response", "malformed DER signature"))?;
                // Sui rejects high-S signatures.
                let parsed = parsed.normalize_s().unwrap_or(parsed);
                let bytes: [u8; 64] = parsed.to_bytes().into();
                sui::types::SimpleSignature::Secp256r1 {
                    signature: sui::types::Secp256r1Signature::new(bytes),
                    public_key: *public_key,
                }
            }
        };
        let signature = sui::types::UserSignature::Simple(signature);
        // Never return a signature Sui would reject (wrong key, wrong digest, bit flips).
        use sui::crypto::SuiVerifier as _;
        sui::crypto::UserSignatureVerifier::new()
            .verify_transaction(tx, &signature)
            .map_err(|_| {
                kms_error(
                    "AsymmetricSign response",
                    "signature does not verify against the key's public key",
                )
            })?;
        Ok(signature)
    }
}

impl TransactionSigner for GcpKmsSigner {
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

/// The path part of an already validated proxy URL (empty when there is none).
fn proxy_url_path(proxy_url: &str) -> &str {
    let after_scheme = proxy_url
        .split_once("://")
        .map_or(proxy_url, |(_, rest)| rest);
    after_scheme
        .find('/')
        .map_or("", |index| &after_scheme[index..])
}

fn proxy_url_error(rule: &str) -> NexusError {
    NexusError::Configuration(format!("KMS proxy URL {rule}"))
}

/// Accepts only the canonical form `http(s)://host[:port][/segment...]`, so
/// that what is configured is exactly what the HTTP client sends. Error texts
/// name the rule broken and never echo the URL.
fn validate_proxy_url(proxy_url: &str) -> Result<(), NexusError> {
    let Some((scheme, rest)) = proxy_url.split_once("://") else {
        return Err(proxy_url_error("must be http or https"));
    };
    if scheme != "http" && scheme != "https" {
        return Err(proxy_url_error("must be http or https"));
    }
    if proxy_url.contains(char::is_whitespace) {
        return Err(proxy_url_error("must not contain whitespace"));
    }
    if proxy_url.contains(['?', '#']) {
        return Err(proxy_url_error("must not have a query or fragment"));
    }
    let (authority, path) = rest.find('/').map_or((rest, ""), |i| rest.split_at(i));
    if authority.contains('@') {
        return Err(proxy_url_error("must not contain credentials"));
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    if host.is_empty() {
        return Err(proxy_url_error("must have a host"));
    }
    // The URL parser reads a host whose last label starts with a digit as
    // IPv4, so it must be a strict dotted quad; anything else is a DNS name.
    let last_label = host.rsplit('.').next().unwrap_or_default();
    let host_ok = if last_label.starts_with(|c: char| c.is_ascii_digit()) {
        host.parse::<std::net::Ipv4Addr>().is_ok()
    } else {
        host.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    };
    if !host_ok {
        return Err(proxy_url_error(
            "host must be a DNS name or an IPv4 address",
        ));
    }
    if let Some(port) = port {
        let digits_only =
            !port.is_empty() && !port.starts_with('0') && port.chars().all(|c| c.is_ascii_digit());
        if !digits_only || !matches!(port.parse::<u16>(), Ok(1..)) {
            return Err(proxy_url_error("port must be 1-65535"));
        }
    }
    // The path is empty or `/segment(/segment)*`; it never ends in `/`.
    if !path.is_empty() {
        let canonical = path[1..].split('/').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '~' | '-'))
        });
        if !path.starts_with('/') || !canonical {
            return Err(proxy_url_error(
                "path must be segments of letters, digits, '_', '~' or '-'",
            ));
        }
    }
    Ok(())
}

/// Accepts only a pinned key version, never a key (which would follow rotation).
fn validate_key_version(name: &str) -> Result<(), NexusError> {
    let parts: Vec<&str> = name.split('/').collect();
    let segment_ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    let valid = parts.len() == 10
        && [
            "projects",
            "locations",
            "keyRings",
            "cryptoKeys",
            "cryptoKeyVersions",
        ]
        .iter()
        .enumerate()
        .all(|(i, label)| parts[2 * i] == *label && segment_ok(parts[2 * i + 1]))
        && parts[9].chars().all(|c| c.is_ascii_digit());
    if valid {
        Ok(())
    } else {
        Err(NexusError::Configuration(
            "invalid KMS key version: expected projects/<p>/locations/<l>/keyRings/<r>/cryptoKeys/<k>/cryptoKeyVersions/<n> (a pinned version, not a key)".into(),
        ))
    }
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, NexusError> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|_| NexusError::Configuration("KMS returned an unparsable public key PEM".into()))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::sui,
        ed25519_dalek::Signer as _,
        futures::future::{join_all, ready},
        p256::ecdsa::signature::hazmat::PrehashSigner as _,
        std::sync::atomic::{AtomicUsize, Ordering},
    };

    const VECTORS: &str = include_str!("../../../tests/fixtures/kms_sui_keytool_vectors.json");
    const KEY_VERSION: &str = "projects/p/locations/l/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1";
    /// `CryptoKeyVersionAlgorithm::EC_SIGN_ED25519` as the REST client's int enum encoding.
    const EC_SIGN_ED25519_INT: i32 = 40;
    const P256_ORDER_HALF: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31,
        0x92, 0xa8,
    ];

    fn vector(kind: &str) -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(VECTORS).unwrap()[kind].clone()
    }

    fn vector_str(kind: &str, field: &str) -> String {
        vector(kind)[field].as_str().unwrap().to_owned()
    }

    fn fixture_tx() -> sui::types::Transaction {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(vector_str("ed25519", "kat_tx_b64"))
            .unwrap();
        bcs::from_bytes(&bytes).unwrap()
    }

    fn spki_pem(der: &[u8]) -> String {
        use base64::Engine as _;
        let body = base64::engine::general_purpose::STANDARD.encode(der);
        format!("-----BEGIN PUBLIC KEY-----\n{body}\n-----END PUBLIC KEY-----\n")
    }

    fn crc(bytes: &[u8]) -> i64 {
        kms_crc32c(bytes)
    }

    enum Key {
        Ed25519(ed25519_dalek::SigningKey),
        P256(p256::ecdsa::SigningKey),
    }

    impl Key {
        fn pem(&self) -> String {
            match self {
                Key::Ed25519(key) => {
                    let mut der = vec![
                        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
                    ];
                    der.extend_from_slice(key.verifying_key().as_bytes());
                    spki_pem(&der)
                }
                Key::P256(key) => {
                    use p256::pkcs8::EncodePublicKey as _;
                    key.verifying_key()
                        .to_public_key_pem(p256::pkcs8::LineEnding::LF)
                        .unwrap()
                }
            }
        }

        fn algorithm(&self) -> KmsAlgorithm {
            match self {
                Key::Ed25519(_) => KmsAlgorithm::Ed25519,
                Key::P256(_) => KmsAlgorithm::P256Sha256,
            }
        }

        fn sign(&self, input: &SignInput, high_s: bool) -> Vec<u8> {
            match (self, input) {
                (Key::Ed25519(key), SignInput::Data(data)) => key.sign(data).to_bytes().to_vec(),
                (Key::P256(key), SignInput::Sha256Digest(digest)) => {
                    let signature: p256::ecdsa::Signature = key.sign_prehash(digest).unwrap();
                    let signature = signature.normalize_s().unwrap_or(signature);
                    let (r, s) = signature.split_scalars();
                    let signature = if high_s {
                        p256::ecdsa::Signature::from_scalars(r, -*s).unwrap()
                    } else {
                        signature
                    };
                    signature.to_der().as_bytes().to_vec()
                }
                _ => panic!("fake KMS asked for an unsupported input"),
            }
        }
    }

    #[derive(Default)]
    struct Switches {
        pem_crc: bool,
        signature_crc: bool,
        unverified_input: bool,
        high_s: bool,
        wrong_algorithm: bool,
    }

    struct FakeKms {
        key: Key,
        other_key: Option<Key>,
        switches: Switches,
        public_key_calls: AtomicUsize,
        sign_calls: AtomicUsize,
    }

    impl FakeKms {
        fn new(key: Key) -> Self {
            Self {
                key,
                other_key: None,
                switches: Switches::default(),
                public_key_calls: AtomicUsize::new(0),
                sign_calls: AtomicUsize::new(0),
            }
        }

        fn ed25519() -> Self {
            let seed = hex::decode(vector_str("ed25519", "private_key_hex")).unwrap();
            Self::new(Key::Ed25519(ed25519_dalek::SigningKey::from_bytes(
                &seed.try_into().unwrap(),
            )))
        }

        fn p256() -> Self {
            let seed = hex::decode(vector_str("secp256r1", "private_key_hex")).unwrap();
            Self::new(Key::P256(
                p256::ecdsa::SigningKey::from_slice(&seed).unwrap(),
            ))
        }

        async fn signer(self) -> Result<GcpKmsSigner, NexusError> {
            GcpKmsSigner::with_api(KEY_VERSION, Arc::new(self)).await
        }
    }

    impl KmsApi for FakeKms {
        fn get_public_key<'a>(
            &'a self,
            name: &'a str,
        ) -> BoxFuture<'a, Result<PublicKeyResp, NexusError>> {
            assert_eq!(name, KEY_VERSION);
            self.public_key_calls.fetch_add(1, Ordering::SeqCst);
            let pem = self.key.pem();
            let mut pem_crc32c = crc(pem.as_bytes());
            if self.switches.pem_crc {
                pem_crc32c += 1;
            }
            let algorithm = if self.switches.wrong_algorithm {
                KmsAlgorithm::Unsupported("RSA_SIGN_PKCS1_2048_SHA256".into())
            } else {
                self.key.algorithm()
            };
            ready(Ok(PublicKeyResp {
                pem,
                algorithm,
                pem_crc32c: Some(pem_crc32c),
            }))
            .boxed()
        }

        fn asymmetric_sign<'a>(
            &'a self,
            name: &'a str,
            input: SignInput,
            input_crc32c: i64,
        ) -> BoxFuture<'a, Result<SignResp, NexusError>> {
            assert_eq!(name, KEY_VERSION);
            assert_eq!(input_crc32c, crc(input.bytes()), "request crc must match");
            self.sign_calls.fetch_add(1, Ordering::SeqCst);
            let signer = self.other_key.as_ref().unwrap_or(&self.key);
            let signature = signer.sign(&input, self.switches.high_s);
            let mut signature_crc32c = crc(&signature);
            if self.switches.signature_crc {
                signature_crc32c += 1;
            }
            ready(Ok(SignResp {
                name: name.to_owned(),
                signature,
                signature_crc32c: Some(signature_crc32c),
                verified_input_crc32c: !self.switches.unverified_input,
            }))
            .boxed()
        }
    }

    #[tokio::test]
    async fn ed25519_matches_keytool_vector() {
        let signer = FakeKms::ed25519().signer().await.unwrap();
        assert_eq!(
            signer.address().to_string(),
            vector_str("ed25519", "address")
        );
        let tx = fixture_tx();
        let signature = TransactionSigner::sign(&signer, &tx).await.unwrap();
        assert_eq!(
            signature.to_base64(),
            vector_str("ed25519", "kat_keytool_signature_b64")
        );
    }

    #[tokio::test]
    async fn p256_address_matches_keytool_vector() {
        let signer = FakeKms::p256().signer().await.unwrap();
        assert_eq!(
            signer.address().to_string(),
            vector_str("secp256r1", "address")
        );
    }

    fn assert_low_s_and_verifies(
        signature: &sui::types::UserSignature,
        tx: &sui::types::Transaction,
    ) {
        use sui::crypto::SuiVerifier as _;
        sui::crypto::UserSignatureVerifier::new()
            .verify_transaction(tx, signature)
            .unwrap();
        let sui::types::UserSignature::Simple(sui::types::SimpleSignature::Secp256r1 {
            signature,
            ..
        }) = signature
        else {
            panic!("expected a Secp256r1 signature");
        };
        assert!(
            AsRef::<[u8]>::as_ref(signature)[32..] <= P256_ORDER_HALF[..],
            "s must be low"
        );
    }

    #[tokio::test]
    async fn p256_signature_verifies_and_is_low_s() {
        let signer = FakeKms::p256().signer().await.unwrap();
        let tx = fixture_tx();
        let signature = TransactionSigner::sign(&signer, &tx).await.unwrap();
        assert_low_s_and_verifies(&signature, &tx);
    }

    #[tokio::test]
    async fn p256_high_s_is_normalized() {
        let mut kms = FakeKms::p256();
        kms.switches.high_s = true;
        // Sanity: the fake really returns a high-S signature.
        let digest = [7u8; 32];
        let der = kms.key.sign(&SignInput::Sha256Digest(digest), true);
        let raw = p256::ecdsa::Signature::from_der(&der).unwrap().to_bytes();
        assert!(raw[32..] > P256_ORDER_HALF[..]);

        let signer = kms.signer().await.unwrap();
        let tx = fixture_tx();
        let signature = TransactionSigner::sign(&signer, &tx).await.unwrap();
        assert_low_s_and_verifies(&signature, &tx);
    }

    #[tokio::test]
    async fn rejects_unverified_data_crc() {
        let mut kms = FakeKms::ed25519();
        kms.switches.unverified_input = true;
        let signer = kms.signer().await.unwrap();
        assert!(TransactionSigner::sign(&signer, &fixture_tx())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn rejects_signature_crc_mismatch() {
        let mut kms = FakeKms::ed25519();
        kms.switches.signature_crc = true;
        let signer = kms.signer().await.unwrap();
        assert!(TransactionSigner::sign(&signer, &fixture_tx())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn rejects_pem_crc_mismatch() {
        let mut kms = FakeKms::ed25519();
        kms.switches.pem_crc = true;
        assert!(kms.signer().await.is_err());
    }

    #[tokio::test]
    async fn rejects_unsupported_algorithm() {
        let mut kms = FakeKms::ed25519();
        kms.switches.wrong_algorithm = true;
        assert!(kms.signer().await.is_err());
    }

    #[tokio::test]
    async fn rejects_signature_from_other_key() {
        let mut kms = FakeKms::ed25519();
        kms.other_key = Some(Key::Ed25519(ed25519_dalek::SigningKey::from_bytes(
            &[5; 32],
        )));
        let signer = kms.signer().await.unwrap();
        assert!(TransactionSigner::sign(&signer, &fixture_tx())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn rejects_malformed_key_version() {
        let result =
            GcpKmsSigner::with_api("projects/p/cryptoKeys/k", Arc::new(FakeKms::ed25519())).await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn concurrent_signs_share_one_client() {
        let kms = Arc::new(FakeKms::ed25519());
        let signer = GcpKmsSigner::with_api(KEY_VERSION, kms.clone())
            .await
            .unwrap();
        let tx = fixture_tx();
        let results = join_all((0..32).map(|_| TransactionSigner::sign(&signer, &tx))).await;
        assert!(results.iter().all(Result::is_ok));
        assert_eq!(kms.public_key_calls.load(Ordering::SeqCst), 1);
        assert_eq!(kms.sign_calls.load(Ordering::SeqCst), 32);
    }

    #[test]
    fn crc32c_known_answer() {
        // The CRC-32C (Castagnoli) check value.
        assert_eq!(kms_crc32c(b"123456789"), 0xe306_9283);
    }

    /// The Google crates ship without a rustls provider, so building the
    /// client (which builds its HTTPS stack) panics unless `connect` installs
    /// one. Anonymous credentials keep this offline: nothing is sent.
    #[tokio::test]
    async fn connect_builds_https_client_without_bundled_provider() {
        let credentials = google_cloud_auth::credentials::anonymous::Builder::new().build();
        assert!(GoogleKms::connect(KmsTarget::DirectToCloudKms {
            credentials_instead_of_adc: Some(credentials),
        })
        .await
        .is_ok());
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }

    /// Sign body the Google REST client sends for `AsymmetricSign` with data.
    fn sign_request_matcher(digest: &[u8]) -> mockito::Matcher {
        use base64::Engine as _;
        // The client repeats `name` in the body as well as in the path.
        mockito::Matcher::Json(serde_json::json!({
            "name": KEY_VERSION,
            "data": base64::engine::general_purpose::STANDARD.encode(digest),
            "dataCrc32c": crc(digest).to_string(),
        }))
    }

    #[tokio::test]
    async fn via_proxy_signs_through_proxy_path_without_credentials() {
        use base64::Engine as _;
        let kms = FakeKms::ed25519();
        let tx = fixture_tx();
        let digest = tx.signing_digest();
        let pem = kms.key.pem();
        let signature = kms.key.sign(&SignInput::Data(digest.to_vec()), false);
        let b64 = base64::engine::general_purpose::STANDARD;

        let mut server = mockito::Server::new_async().await;
        let query = mockito::Matcher::Regex(r"(?i)alt=json.*enum-encoding(=|%3D)int".into());
        let public_key = server
            .mock("GET", format!("/tok3n/v1/{KEY_VERSION}/publicKey").as_str())
            .match_query(query.clone())
            .match_header("authorization", mockito::Matcher::Missing)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "pem": pem,
                    "pemCrc32c": crc(pem.as_bytes()).to_string(),
                    "algorithm": EC_SIGN_ED25519_INT,
                    "name": KEY_VERSION,
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;
        let sign = server
            .mock(
                "POST",
                format!("/tok3n/v1/{KEY_VERSION}:asymmetricSign").as_str(),
            )
            .match_query(query)
            .match_header("authorization", mockito::Matcher::Missing)
            .match_body(sign_request_matcher(&digest))
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "name": KEY_VERSION,
                    "signature": b64.encode(&signature),
                    "signatureCrc32c": crc(&signature).to_string(),
                    "verifiedDataCrc32c": true,
                })
                .to_string(),
            )
            .expect(1)
            .create_async()
            .await;

        let signer = GcpKmsSigner::via_proxy(KEY_VERSION, format!("{}/tok3n", server.url()))
            .await
            .unwrap();
        let user_signature = TransactionSigner::sign(&signer, &tx).await.unwrap();
        assert_eq!(
            user_signature.to_base64(),
            vector_str("ed25519", "kat_keytool_signature_b64")
        );
        public_key.assert_async().await;
        sign.assert_async().await;
    }

    #[tokio::test]
    async fn via_proxy_rejects_bad_urls() {
        for bad in [
            "ftp://kms-proxy:8790/s3cret-tok3n",
            "HTTP://kms-proxy:8790/s3cret-tok3n",
            "http:///s3cret-tok3n",
            "http://:8790/s3cret-tok3n",
            "kms-proxy:8790/s3cret-tok3n",
            "http://kms-proxy:8790/s3cret-tok3n?x=1",
            "http://kms-proxy:8790/s3cret-tok3n#frag",
            "http://kms-proxy:99999/s3cret-tok3n",
            "http://kms-proxy:0/s3cret-tok3n",
            "http://kms-proxy:/s3cret-tok3n",
            "http://kms-proxy:8x/s3cret-tok3n",
            "http://user:pw@kms-proxy:8790/s3cret-tok3n",
            "http://[::1]:8790/s3cret-tok3n",
            "http://kms_proxy:8790/s3cret-tok3n",
            "http://kms-proxy:8790/a/../s3cret-tok3n",
            "http://kms-proxy:8790/./s3cret-tok3n",
            "http://kms-proxy:8790/s3cret-tok3n/",
            "http://kms-proxy/",
            "http://1.2.3.999:8790/s3cret-tok3n",
            "http://1.2.3.4.5:8790/s3cret-tok3n",
            "http://256.0.0.1:8790/s3cret-tok3n",
            "http://999:8790/s3cret-tok3n",
            "http://0x7f.1:8790/s3cret-tok3n",
            "http://a..b:8790/s3cret-tok3n",
            "http://-a:8790/s3cret-tok3n",
            "http://a-:8790/s3cret-tok3n",
            "http://.:8790/s3cret-tok3n",
            "http://kms-proxy.:8790/s3cret-tok3n",
            "http://kms-proxy:08790/s3cret-tok3n",
            "http://kms-proxy:8790//s3cret-tok3n",
            "http://kms-proxy:8790/{s3cret-tok3n}",
            "http://kms-proxy:8790/s3cret-tok\u{f6}n",
            "http://kms-proxy:8790/s3cret-to%6Bn",
            "http://kms-proxy:8790/s3cret tok3n",
            "",
        ] {
            let error = GcpKmsSigner::via_proxy(KEY_VERSION, bad).await.unwrap_err();
            assert!(matches!(error, NexusError::Configuration(_)), "{bad}");
            let text = format!("{error} {error:?}");
            assert!(!text.contains("s3cret"), "{bad}: {text}");
            assert!(!text.contains("kms-proxy"), "{bad}: {text}");
            assert!(!text.contains("pw"), "{bad}: {text}");
        }
    }

    #[test]
    fn via_proxy_accepts_canonical_urls() {
        for good in [
            "http://kms-proxy:8790/s3cret-tok3n",
            "http://127.0.0.1:8790/tok3n",
            "https://kms-proxy.internal/a/b_c~d",
            "http://kms-proxy",
            "http://kms-proxy:65535",
        ] {
            assert!(validate_proxy_url(good).is_ok(), "{good}");
        }
    }

    /// A proxy answering with a non-retryable error that echoes the request
    /// path (as a 404 from a router might) must not leak the token path.
    #[tokio::test]
    async fn via_proxy_errors_hide_url() {
        let mut server = mockito::Server::new_async().await;
        let _echo = server
            .mock("GET", mockito::Matcher::Any)
            .match_query(mockito::Matcher::Any)
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body_from_request(|request| {
                serde_json::json!({"error": {
                    "code": 400,
                    "status": "INVALID_ARGUMENT",
                    "message": format!("no route for {}", request.path()),
                }})
                .to_string()
                .into_bytes()
            })
            .create_async()
            .await;
        let error = GcpKmsSigner::via_proxy(KEY_VERSION, format!("{}/s3cret-tok3n", server.url()))
            .await
            .unwrap_err();
        let text = format!("{error} {error:?}");
        assert!(text.contains("no route for"), "{text}");
        assert!(text.contains("<redacted>"), "{text}");
        assert!(!text.contains("s3cret-tok3n"), "{text}");
    }

    #[tokio::test]
    async fn via_proxy_debug_hides_url() {
        let mut server = mockito::Server::new_async().await;
        let kms = FakeKms::ed25519();
        let pem = kms.key.pem();
        let _public_key = server
            .mock("GET", mockito::Matcher::Any)
            .match_query(mockito::Matcher::Any)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "pem": pem,
                    "pemCrc32c": crc(pem.as_bytes()).to_string(),
                    "algorithm": EC_SIGN_ED25519_INT,
                })
                .to_string(),
            )
            .create_async()
            .await;
        let signer = GcpKmsSigner::via_proxy(KEY_VERSION, format!("{}/s3cret-tok3n", server.url()))
            .await
            .unwrap();
        let rendered = format!("{signer:?}");
        assert!(!rendered.contains("s3cret-tok3n"));
        assert!(rendered.contains(KEY_VERSION));
    }

    #[test]
    fn redact_secret_path_replaces_every_occurrence_and_skips_empty() {
        let text = redact_secret_path(
            Some("/s3cret-tok3n"),
            "for url (http://h:1/s3cret-tok3n/v1/x) at /s3cret-tok3n".into(),
        );
        assert!(!text.contains("s3cret-tok3n"));
        assert_eq!(redact_secret_path(Some(""), "a/b".into()), "a/b");
        assert_eq!(redact_secret_path(None, "a/b".into()), "a/b");
    }

    #[test]
    fn debug_shows_only_key_version_and_address() {
        let rendered = format!(
            "{:?}",
            futures::executor::block_on(FakeKms::ed25519().signer()).unwrap()
        );
        assert!(rendered.contains(KEY_VERSION));
        assert!(rendered.contains(&vector_str("ed25519", "address")));
    }
}
