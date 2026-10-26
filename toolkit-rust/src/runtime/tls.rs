//! TLS transport for [`crate::bootstrap!`].

use {
    anyhow::Context as _,
    ed25519_dalek::{pkcs8::DecodePublicKey, VerifyingKey},
    hyper_util::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto,
        service::TowerToHyperService,
    },
    std::{
        io,
        net::SocketAddr,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    },
    tokio_rustls::{
        rustls::{
            client::danger::HandshakeSignatureValid,
            crypto::{
                verify_tls12_signature as verify_tls12_client_signature,
                verify_tls13_signature as verify_tls13_client_signature,
                WebPkiSupportedAlgorithms,
            },
            pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
            server::{
                danger::{ClientCertVerified, ClientCertVerifier},
                NoServerSessionStorage,
                ParsedCertificate,
            },
            CertificateError,
            DigitallySignedStruct,
            Error as TlsError,
            ServerConfig,
            SignatureScheme,
        },
        server::TlsStream,
        TlsAcceptor,
    },
    tokio_stream::{Stream, StreamExt},
    tower_service::Service,
    warp::http::Request,
};

const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// TLS configuration for [`crate::bootstrap!`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Config {
    /// TLS is disabled.
    Disabled,
    /// TLS is enabled with credentials read from files.
    Enabled {
        /// Path to a PEM encoded certificate chain.
        cert_path: PathBuf,
        /// Path to a PEM encoded private key.
        key_path: PathBuf,
    },
}

impl Config {
    /// Loads the certificate and private key paths from the environment.
    ///
    /// # Errors
    ///
    /// Returns an error unless `NEXUS_TOOL_TLS_CERT_PATH` and
    /// `NEXUS_TOOL_TLS_KEY_PATH` are both absent or both contain a path.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_paths(
            std::env::var_os("NEXUS_TOOL_TLS_CERT_PATH").map(PathBuf::from),
            std::env::var_os("NEXUS_TOOL_TLS_KEY_PATH").map(PathBuf::from),
        )
    }

    fn from_paths(cert_path: Option<PathBuf>, key_path: Option<PathBuf>) -> anyhow::Result<Self> {
        match (cert_path, key_path) {
            (None, None) => Ok(Self::Disabled),
            (Some(cert_path), Some(key_path))
                if !cert_path.as_os_str().is_empty() && !key_path.as_os_str().is_empty() =>
            {
                Ok(Self::Enabled {
                    cert_path,
                    key_path,
                })
            }
            _ => anyhow::bail!(
                "NEXUS_TOOL_TLS_CERT_PATH and NEXUS_TOOL_TLS_KEY_PATH must both be set to nonempty paths"
            ),
        }
    }
}

fn load_server_config(cert_path: &Path, key_path: &Path) -> anyhow::Result<ServerConfig> {
    let certificates = CertificateDer::pem_file_iter(cert_path)
        .with_context(|| {
            format!(
                "failed to open TLS certificate file {}",
                cert_path.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "failed to read TLS certificate file {}",
                cert_path.display()
            )
        })?;
    if certificates.is_empty() {
        anyhow::bail!(
            "TLS certificate file {} contains no certificates",
            cert_path.display()
        );
    }

    let private_key = PrivateKeyDer::from_pem_file(key_path)
        .with_context(|| format!("failed to read TLS private key file {}", key_path.display()))?;
    // The toolkit owns the server transport, so it must select its provider
    // explicitly. Tool dependencies may enable another rustls provider and
    // make process wide inference ambiguous.
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let verifier = Arc::new(Ed25519ClientCertVerifier {
        algorithms: provider.signature_verification_algorithms,
    });
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .context("failed to select safe TLS protocol versions")?
        .with_client_cert_verifier(verifier)
        .with_single_cert(certificates, private_key)
        .context("failed to build TLS server configuration")?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config.session_storage = Arc::new(NoServerSessionStorage {});
    config.max_early_data_size = 0;
    config.max_tls13_tickets = 0;

    Ok(config)
}

#[derive(Debug)]
struct Ed25519ClientCertVerifier {
    algorithms: WebPkiSupportedAlgorithms,
}

impl ClientCertVerifier for Ed25519ClientCertVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        false
    }

    fn root_hint_subjects(&self) -> &[tokio_rustls::rustls::DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _now: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        if !intermediates.is_empty() {
            return Err(TlsError::General(
                "Leader client identity must contain one certificate".into(),
            ));
        }
        ed25519_public_key(end_entity)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_client_signature(message, certificate, signature, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_client_signature(message, certificate, signature, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

fn ed25519_public_key(certificate: &CertificateDer<'_>) -> Result<[u8; 32], TlsError> {
    let parsed = ParsedCertificate::try_from(certificate)?;
    VerifyingKey::from_public_key_der(parsed.subject_public_key_info().as_ref())
        .map(|key| key.to_bytes())
        .map_err(|_| TlsError::InvalidCertificate(CertificateError::BadEncoding))
}

/// Peer identity attached only to requests accepted on a TLS connection.
#[derive(Clone, Debug)]
pub struct TlsPeerIdentity(Option<[u8; 32]>);

impl TlsPeerIdentity {
    pub(crate) fn public_key(&self) -> Option<[u8; 32]> {
        self.0
    }
}

#[derive(Clone)]
struct PeerExtensionService<S> {
    inner: S,
    peer: TlsPeerIdentity,
}

impl<S> PeerExtensionService<S> {
    fn new(inner: S, peer: TlsPeerIdentity) -> Self {
        Self { inner, peer }
    }
}

impl<S, B> Service<Request<B>> for PeerExtensionService<S>
where
    S: Service<Request<B>>,
{
    type Error = S::Error;
    type Future = S::Future;
    type Response = S::Response;

    fn poll_ready(
        &mut self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        request.extensions_mut().insert(self.peer.clone());
        self.inner.call(request)
    }
}

/// Creates the TLS connection stream consumed by [`crate::bootstrap!`].
///
/// The certificate chain and private key are loaded and validated before the
/// listener is bound. Connections that fail or exceed the handshake timeout are
/// discarded without stopping the listener.
///
/// # Errors
///
/// Returns an error when the credentials cannot be loaded or validated, or
/// when the TCP listener cannot be bound or queried for its local address.
pub async fn bind(
    address: SocketAddr,
    cert_path: PathBuf,
    key_path: PathBuf,
) -> anyhow::Result<(
    SocketAddr,
    impl Stream<Item = io::Result<TlsStream<tokio::net::TcpStream>>>,
)> {
    let acceptor = TlsAcceptor::from(Arc::new(load_server_config(&cert_path, &key_path)?));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .context("failed to bind TLS listener")?;
    let address = listener
        .local_addr()
        .context("failed to read TLS listener address")?;

    let mut builder = tls_listener::builder(acceptor);
    builder.handshake_timeout(TLS_HANDSHAKE_TIMEOUT);
    let incoming = builder
        .listen(listener)
        .connections()
        .filter_map(|result| match result {
            Ok(connection) => Some(Ok(connection)),
            Err(tls_listener::Error::ListenerError(error)) => Some(Err(error)),
            Err(error) => {
                log::debug!(
                    "discarding TLS connection from {:?}: {error}",
                    error.peer_addr()
                );
                None
            }
        });

    Ok((address, incoming))
}

/// Serves Warp routes over accepted TLS connections.
pub async fn serve<F, S>(routes: F, incoming: S)
where
    F: warp::Filter<Error = warp::Rejection> + Clone + Send + Sync + 'static,
    F::Extract: warp::Reply,
    S: Stream<Item = io::Result<TlsStream<tokio::net::TcpStream>>>,
{
    tokio::pin!(incoming);
    while let Some(connection) = incoming.next().await {
        let connection = match connection {
            Ok(connection) => connection,
            Err(error) => {
                log::debug!("TLS listener failed: {error}");
                continue;
            }
        };
        let peer_key = connection
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|certificates| certificates.first())
            .and_then(|certificate| ed25519_public_key(certificate).ok());
        let service =
            PeerExtensionService::new(warp::service(routes.clone()), TlsPeerIdentity(peer_key));
        let service = TowerToHyperService::new(service);
        tokio::spawn(async move {
            let builder = auto::Builder::new(TokioExecutor::new());
            if let Err(error) = builder
                .serve_connection_with_upgrades(TokioIo::new(connection), service)
                .await
            {
                log::debug!("TLS connection failed: {error}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        tokio::net::TcpStream,
        warp::{http::StatusCode, Filter},
    };

    #[tokio::test]
    async fn accepts_https_while_another_handshake_is_stalled() {
        let rcgen::CertifiedKey { cert, key_pair } =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_pem = cert.pem();
        let temp_dir = tempfile::tempdir().unwrap();
        let cert_path = temp_dir.path().join("cert.pem");
        let key_path = temp_dir.path().join("key.pem");
        std::fs::write(&cert_path, &cert_pem).unwrap();
        std::fs::write(&key_path, key_pair.serialize_pem()).unwrap();

        let (address, incoming) = bind(([127, 0, 0, 1], 0).into(), cert_path, key_path)
            .await
            .unwrap();
        let server = tokio::spawn(serve(warp::path("health").map(|| StatusCode::OK), incoming));
        let stalled_connection = TcpStream::connect(address).await.unwrap();

        let root = reqwest::Certificate::from_pem(cert_pem.as_bytes()).unwrap();
        let client = reqwest::Client::builder()
            .add_root_certificate(root)
            .build()
            .unwrap();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            client
                .get(format!("https://localhost:{}/health", address.port()))
                .send(),
        )
        .await
        .unwrap();
        drop(stalled_connection);
        server.abort();

        assert_eq!(response.unwrap().status(), reqwest::StatusCode::OK);
    }

    #[test]
    fn config_requires_both_paths() {
        assert_eq!(Config::from_paths(None, None).unwrap(), Config::Disabled);
        assert_eq!(
            Config::from_paths(Some("cert.pem".into()), Some("key.pem".into())).unwrap(),
            Config::Enabled {
                cert_path: "cert.pem".into(),
                key_path: "key.pem".into(),
            }
        );
        for (cert, key) in [
            (Some("cert.pem".into()), None),
            (None, Some("key.pem".into())),
            (Some(PathBuf::new()), Some("key.pem".into())),
            (Some("cert.pem".into()), Some(PathBuf::new())),
        ] {
            assert!(Config::from_paths(cert, key).is_err());
        }
    }

    #[test]
    fn client_certificates_must_contain_an_ed25519_spki() {
        use ed25519_dalek::pkcs8::EncodePrivateKey as _;

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let private_key = signing_key.to_pkcs8_der().unwrap();
        let private_key_der =
            tokio_rustls::rustls::pki_types::PrivatePkcs8KeyDer::from(private_key.as_bytes());
        let key_pair =
            rcgen::KeyPair::from_pkcs8_der_and_sign_algo(&private_key_der, &rcgen::PKCS_ED25519)
                .unwrap();
        let certificate = rcgen::CertificateParams::new(vec!["leader".to_owned()])
            .unwrap()
            .self_signed(&key_pair)
            .unwrap();
        let certificate = CertificateDer::from(certificate.der().to_vec());
        assert_eq!(
            ed25519_public_key(&certificate).unwrap(),
            signing_key.verifying_key().to_bytes()
        );
        assert!(ed25519_public_key(&CertificateDer::from(vec![1, 2, 3])).is_err());

        let ecdsa_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let ecdsa_certificate = rcgen::CertificateParams::new(vec!["other".to_owned()])
            .unwrap()
            .self_signed(&ecdsa_key)
            .unwrap();
        assert!(
            ed25519_public_key(&CertificateDer::from(ecdsa_certificate.der().to_vec())).is_err()
        );
    }

    #[test]
    fn tls_server_disables_resumption_and_early_data() {
        let rcgen::CertifiedKey { cert, key_pair } =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let cert_path = temp_dir.path().join("cert.pem");
        let key_path = temp_dir.path().join("key.pem");
        std::fs::write(&cert_path, cert.pem()).unwrap();
        std::fs::write(&key_path, key_pair.serialize_pem()).unwrap();

        let config = load_server_config(&cert_path, &key_path).unwrap();
        assert!(!config.session_storage.can_cache());
        assert!(!config.ticketer.enabled());
        assert_eq!(config.max_early_data_size, 0);
        assert_eq!(config.max_tls13_tickets, 0);
    }
}
