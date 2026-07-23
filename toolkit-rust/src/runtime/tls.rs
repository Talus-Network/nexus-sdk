//! Direct TLS support for [`crate::bootstrap!`].
//!
//! Each accepted connection negotiates TLS when Warp first polls it. This keeps
//! connection acceptance independent from negotiation progress.

use {
    anyhow::Context as _,
    std::{
        future::Future,
        io,
        net::SocketAddr,
        path::{Path, PathBuf},
        pin::Pin,
        sync::Arc,
        task::{ready, Context, Poll},
    },
    tokio::{
        io::{AsyncRead, AsyncWrite, ReadBuf},
        net::TcpStream,
    },
    tokio_rustls::{
        rustls::{
            pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
            ServerConfig,
        },
        server::TlsStream,
        Accept,
        TlsAcceptor,
    },
    tokio_stream::{wrappers::TcpListenerStream, Stream, StreamExt},
};

/// Direct TLS configuration for [`crate::bootstrap!`].
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

enum State {
    Handshaking(Accept<TcpStream>),
    Streaming(TlsStream<TcpStream>),
}

impl State {
    fn poll_stream(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<&mut TlsStream<TcpStream>>> {
        loop {
            match self {
                Self::Handshaking(handshake) => {
                    let stream = ready!(Pin::new(handshake).poll(cx))?;
                    *self = Self::Streaming(stream);
                }
                Self::Streaming(stream) => return Poll::Ready(Ok(stream)),
            }
        }
    }
}

struct Connection {
    state: State,
}

impl Connection {
    fn new(stream: TcpStream, config: Arc<ServerConfig>) -> Self {
        Self {
            state: State::Handshaking(TlsAcceptor::from(config).accept(stream)),
        }
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let connection = self.get_mut();
        let stream = ready!(connection.state.poll_stream(cx))?;

        Pin::new(stream).poll_read(cx, buffer)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let connection = self.get_mut();
        let stream = ready!(connection.state.poll_stream(cx))?;

        Pin::new(stream).poll_write(cx, buffer)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.state {
            State::Handshaking(_) => Poll::Ready(Ok(())),
            State::Streaming(ref mut stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.state {
            State::Handshaking(_) => Poll::Ready(Ok(())),
            State::Streaming(ref mut stream) => Pin::new(stream).poll_shutdown(cx),
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
/// listener is bound. Each accepted [`TcpStream`] negotiates TLS when Warp first
/// polls it for [`AsyncRead`] or [`AsyncWrite`]. This lets the listener continue
/// accepting connections while another client is negotiating.
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
    let config = Arc::new(load_server_config(&cert_path, &key_path)?);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .context("failed to bind TLS listener")?;
    let address = listener
        .local_addr()
        .context("failed to read TLS listener address")?;
    let incoming = TcpListenerStream::new(listener).map(move |connection| {
        let config = config.clone();
        connection.map(|stream| Connection::new(stream, config))
    });

    Ok((address, incoming))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
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
