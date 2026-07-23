//! TLS transport for [`crate::bootstrap!`].

use {
    anyhow::Context as _,
    std::{
        io,
        net::SocketAddr,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    },
    tokio::io::{AsyncRead, AsyncWrite},
    tokio_rustls::{
        rustls::{
            pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
            ServerConfig,
        },
        TlsAcceptor,
    },
    tokio_stream::{Stream, StreamExt},
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
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .context("failed to build TLS server configuration")?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(config)
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
    impl Stream<Item = io::Result<impl AsyncRead + AsyncWrite + Send + Unpin + 'static>>,
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
        let server = tokio::spawn(
            warp::serve(warp::path("health").map(|| StatusCode::OK)).run_incoming(incoming),
        );
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
}
