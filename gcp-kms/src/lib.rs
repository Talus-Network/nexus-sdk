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
//! Requests and responses are protected with CRC32C checks, and every
//! signature is verified locally before it is returned.

use {
    base64::Engine as _,
    futures::future::{BoxFuture, FutureExt as _},
    nexus_sdk::{
        nexus::{error::NexusError, transaction_signer::TransactionSigner},
        sui,
    },
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
/// builds that do not use this crate, so other TLS clients in the process are unaffected.
fn install_default_crypto_provider() {
    // `Err` only means another provider is already installed, which is fine.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// The real KMS client, built once and shared by all signing calls.
struct GoogleKms {
    client: google_cloud_kms_v1::client::KeyManagementService,
}

impl GoogleKms {
    /// Builds the client with Application Default Credentials, or `credentials`.
    async fn connect(
        credentials: Option<google_cloud_auth::credentials::Credentials>,
    ) -> Result<Self, NexusError> {
        install_default_crypto_provider();
        let builder = google_cloud_kms_v1::client::KeyManagementService::builder();
        let builder = match credentials {
            Some(credentials) => builder.with_credentials(credentials),
            None => builder,
        };
        let client = builder.build().await.map_err(|error| {
            NexusError::Configuration(format!(
                "could not create the Cloud KMS client (Application Default Credentials): {error}"
            ))
        })?;
        Ok(Self { client })
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
                .map_err(|error| kms_error("GetPublicKey failed", error))?;
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
                .map_err(|error| kms_error("AsymmetricSign failed", error))?;
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
/// The public key is fetched once in [`Self::new`] and the address derived from
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
        let api = GoogleKms::connect(None).await?;
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
        ed25519_dalek::Signer as _,
        futures::future::{join_all, ready},
        p256::ecdsa::signature::hazmat::PrehashSigner as _,
        std::sync::atomic::{AtomicUsize, Ordering},
    };

    const VECTORS: &str = include_str!("../tests/fixtures/kms_sui_keytool_vectors.json");
    const KEY_VERSION: &str = "projects/p/locations/l/keyRings/r/cryptoKeys/k/cryptoKeyVersions/1";
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
        assert!(GoogleKms::connect(Some(credentials)).await.is_ok());
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
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
