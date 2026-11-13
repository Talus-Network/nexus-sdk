# Google Cloud KMS signer

`nexus-gcp-kms` provides `GcpKmsSigner`, a `nexus_sdk::nexus::transaction_signer::TransactionSigner` backed by a Google Cloud KMS Ed25519 or P-256 key version. The private key never leaves KMS.

```toml
[dependencies]
nexus-sdk = { git = "https://github.com/Talus-Network/nexus-sdk", tag = "v2.2.0", features = ["nexus"] }
nexus-gcp-kms = { git = "https://github.com/Talus-Network/nexus-sdk", tag = "v2.2.0" }
```

Use one revision for both crates so the `TransactionSigner` types are the same.

```rust,ignore
let signer = nexus_gcp_kms::GcpKmsSigner::new(
    "projects/<p>/locations/<l>/keyRings/<r>/cryptoKeys/<k>/cryptoKeyVersions/<n>",
)
.await?;
```

Credentials come from Application Default Credentials. `GcpKmsSigner::new` installs rustls' ring provider as the process default when none is installed.
