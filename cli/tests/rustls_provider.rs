//! The CLI's TLS clients (walrus, jsonrpsee, hyper-rustls) call
//! `rustls::ClientConfig::builder()`, which panics when more than one rustls
//! crypto provider is compiled in and none is installed. `nexus-gcp-kms` must not
//! add a second provider next to ring.
//!
//! This file is its own test binary, so the process starts with no provider
//! installed, like `nexus` does.

#[test]
fn client_config_builder_works_without_installed_provider() {
    assert!(
        rustls::crypto::CryptoProvider::get_default().is_none(),
        "this test needs a fresh process with no provider installed"
    );
    let result = std::panic::catch_unwind(|| {
        rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth()
    });
    assert!(
        result.is_ok(),
        "rustls::ClientConfig::builder() panicked: more than one rustls crypto provider is enabled"
    );
}
