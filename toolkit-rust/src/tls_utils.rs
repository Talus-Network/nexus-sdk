use {
    anyhow::{Context, Result},
    futures_util::TryStreamExt,
    pkcs8::EncodePrivateKey,
    rustls::{
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        crypto::{ring, ring::sign::any_supported_type, verify_tls13_signature_with_raw_key},
        pki_types::{CertificateDer, PrivateKeyDer, ServerName, SubjectPublicKeyInfoDer, UnixTime},
        server::{AlwaysResolvesServerRawPublicKeys, ServerConfig as RustlsServerConfig},
        version::TLS13,
        DigitallySignedStruct,
        Error as RustlsError,
        SignatureScheme,
    },
    sha2::{Digest, Sha256},
    std::{fs, path::Path, sync::Arc},
    subtle::ConstantTimeEq,
    tokio::net::TcpListener,
    tokio_stream::wrappers::TcpListenerStream,
    warp::Filter,
};

// Server side

/// Build a TLS 1.3‑only server configuration that presents a single Ed25519 or
/// P‑256 raw public key (look RFC 7250). The key must be stored as PKCS#8.
pub fn server_cfg<P: AsRef<Path>>(key_path: P) -> Result<RustlsServerConfig> {
    // 1  read private key
    let key_bytes = fs::read(&key_path)
        .with_context(|| format!("reading key file {} failed", key_path.as_ref().display()))?;
    let key_der = PrivateKeyDer::Pkcs8(key_bytes.into());

    // 2 instantiate signer
    let signer =
        any_supported_type(&key_der).context("unsupported key type - need Ed25519 or P-256")?;

    // 3  extract SPKI
    let spki: CertificateDer<'static> = signer.public_key().unwrap().as_ref().to_vec().into();

    // 4 resolver returning a single RPK
    let resolver = AlwaysResolvesServerRawPublicKeys::new(Arc::new(rustls::sign::CertifiedKey {
        cert: vec![spki],
        key: signer,
        ocsp: None,
    }));

    // 5 – assemble config via builder
    let mut cfg = RustlsServerConfig::builder_with_protocol_versions(&[&TLS13])
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(resolver));

    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(cfg)
}

/// SHA‑256 digest of SPKI.
#[inline]
pub fn spki_sha256(spki: &CertificateDer<'_>) -> [u8; 32] {
    Sha256::digest(spki.as_ref()).into()
}

/// Generate a new Ed25519 private key, save as PKCS#8, return hex‑encoded SPKI hash.
pub fn generate_key_and_hash<P: AsRef<Path>>(out: P) -> Result<String> {
    use {ed25519_dalek::SigningKey, rand::rngs::OsRng};

    let mut rng = OsRng;
    let sk = SigningKey::generate(&mut rng);
    let pkcs8 = sk.to_pkcs8_der()?;
    fs::write(&out, pkcs8.as_bytes())?;

    let der = PrivateKeyDer::Pkcs8(pkcs8.as_bytes().to_vec().into());
    let signer = any_supported_type(&der)?;
    let spki: CertificateDer<'static> = signer.public_key().unwrap().as_ref().to_vec().into();
    Ok(hex::encode(spki_sha256(&spki)))
}

/// Compute the SHA-256 hash of the SPKI from an existing key file.
pub fn compute_key_hash<P: AsRef<Path>>(key_path: P) -> Result<String> {
    let key_bytes = fs::read(&key_path)?;
    let der = PrivateKeyDer::Pkcs8(key_bytes.into());
    let signer = any_supported_type(&der)?;
    let spki: CertificateDer<'static> = signer.public_key().unwrap().as_ref().to_vec().into();
    Ok(hex::encode(spki_sha256(&spki)))
}

/// Run a Warp server on the given address using the supplied TLS acceptor.
pub async fn spawn_tls_server<F>(
    routes: F,
    addr: std::net::SocketAddr,
    acceptor: tokio_rustls::TlsAcceptor,
) -> Result<()>
where
    F: Filter + Clone + Send + Sync + 'static,
    F::Extract: warp::reply::Reply + Send + 'static,
{
    let listener = TcpListener::bind(addr).await?;
    let incoming_tls = TcpListenerStream::new(listener).and_then(move |tcp| {
        let acc = acceptor.clone();
        async move {
            acc.accept(tcp).await.map_err(|e| {
                eprintln!("TLS handshake failed: {e}");
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })
        }
    });

    warp::serve(routes).run_incoming(incoming_tls).await;
    Ok(())
}

// Client side

/// Holds the allowed hashes for pinning.
#[derive(Debug)]
pub struct Pinned(pub Vec<[u8; 32]>);

impl ServerCertVerifier for Pinned {
    fn requires_raw_public_keys(&self) -> bool {
        true
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ED25519,
            SignatureScheme::ECDSA_NISTP256_SHA256,
        ]
    }

    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, RustlsError> {
        let digest = spki_sha256(end_entity);
        for pin in &self.0 {
            if pin.ct_eq(&digest).into() {
                return Ok(ServerCertVerified::assertion());
            }
        }
        Err(RustlsError::General("public-key pin mismatch".into()))
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, RustlsError> {
        Err(RustlsError::General("TLS 1.2 not supported".into()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        let spki = SubjectPublicKeyInfoDer::from(cert.as_ref());
        let algs = &ring::default_provider().signature_verification_algorithms;
        verify_tls13_signature_with_raw_key(message, &spki, dss, algs)
    }
}

/// Construct a `reqwest` client that enforces SPKI pinning.
pub fn reqwest_with_pin<'a, I>(pins_hex: I) -> Result<reqwest::Client>
where
    I: IntoIterator<Item = &'a str>,
{
    let pins = pins_hex
        .into_iter()
        .map(|hex| {
            let bytes = hex::decode(hex)?;
            let arr: [u8; 32] = bytes
                .as_slice()
                .try_into()
                .context("pin must be 32 bytes")?;
            Ok::<_, anyhow::Error>(arr)
        })
        .collect::<Result<Vec<_>>>()?;

    let mut cfg = rustls::ClientConfig::builder_with_protocol_versions(&[&TLS13])
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned(pins)))
        .with_no_client_auth();

    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(reqwest::Client::builder()
        .use_preconfigured_tls(cfg)
        .build()
        .context("building reqwest client")?)
}
