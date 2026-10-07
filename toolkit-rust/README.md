# nexus-toolkit

The **Nexus Toolkit** provides essential interfaces and functions for easily
developing Nexus Tools using Rust.

## Usage

You have two easy ways to get started with the Nexus Toolkit:

### Using Nexus CLI (recommended)

The easiest way is to create a fresh Rust project preconfigured for Nexus Tool
development. To do this, first install the [Nexus CLI][nexus-cli-docs], then run:

```sh
nexus tool new --help
```

This command lists all available options to quickly set up your development
environment.

### Manually Adding Dependencies

You can also manually include the Nexus Toolkit in your existing project.
Add the following lines to your project's `Cargo.toml`:

```toml
nexus-toolkit = "2.1.1"
```

---

## HTTPS + signed HTTP

Nexus Tools are HTTP servers. Nexus expects Tools to be reachable over **HTTPS** (TLS certificate validated by Leader nodes via system roots) and to require **signed HTTP** (Ed25519 signatures in `X-Nexus-Sig-*` headers) for `POST /invoke`.

- The toolkit runtime can terminate TLS directly when `NEXUS_TOOL_TLS_CERT_PATH` and `NEXUS_TOOL_TLS_KEY_PATH` are both set. Otherwise, deploy it behind a TLS terminator (reverse proxy / load balancer).
- To enforce signed HTTP in the runtime, set `signed_http.mode = "required"` in the toolkit config.
- Signed HTTP is application-layer authentication for `/invoke`: the Tool verifies which Leader node signed the request, and the Leader node verifies which Tool signed the response (with request/response binding and replay resistance).
- Signed HTTP is enabled via a JSON config file loaded from `NEXUS_TOOLKIT_CONFIG_PATH`.
- Signed HTTP verification happens after the TLS handshake. If you want to reduce unwanted TLS handshakes/traffic, apply edge policy at your TLS terminator (rate limiting, firewall/WAF, mTLS, or private ingress such as Cloudflare Tunnel).
- Nexus Leader nodes do not currently present client certificates when calling Tools (no mTLS client authentication today). In a future update, Nexus will support self-signed certificates and TLS client authentication (mTLS) for Tool communication.

## Large outputs through Walrus

A tool can upload its own output through the SDK and return the resulting `NexusData` reference. The operator supplies the wallet and storage policy; the toolkit does not upload automatically. Complete the upload before returning the output and choose a tool timeout that covers it. Persist the SDK registration and pending upload if invocation cancellation or restarts must be recoverable.

Override `NexusTool::encode_output` to return explicit protocol ports:

```rust,ignore
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Output {
    Ok {
        // The next tool receives the decoded string, not receipt metadata.
        #[schemars(with = "String")]
        result: nexus_sdk::types::NexusData,
    },
}

fn encode_output(output: Output) -> anyhow::Result<nexus_sdk::types::OffchainToolOutput> {
    let Output::Ok { result } = output;
    nexus_sdk::types::OffchainToolOutput::from_ports(
        b"ok".to_vec(), [("result".into(), result)],
    )
}
```

Inside `invoke`, call `nexus_walrus::WalrusStorage::upload` through the [native Walrus adapter](https://github.com/Talus-Network/nexus-sdk/tree/v2.1.1/walrus) and use the returned `StoredBlob::nexus_data()` for the port. Ordinary outputs retain their existing inline encoding. The runtime validates explicit ports against metadata, then signs the exact canonical reference. Port order must match metadata order.

The leader uses its aggregator to read referenced output, checks its digest, and resolves it for the next tool. It does not upload or own the blob. The next toolkit runtime checks the signed input commitment before decoding JSON. Compact inline JSON retains the existing transport. Large data and JSON whose formatting would otherwise change travel as base64 bytes to preserve the digest. The runtime accepts both forms. Upgrade receiving tool runtimes before using large references or data that needs the new `bytes` form.

The shared SDK [execution limits](../sdk/README.md#execution-limits) allow 8 MiB across each complete input set or output set, with independent budgets for each. The default HTTP admission limit is 12 MiB to accommodate base64 and metadata. Omit `invoke_max_body_bytes` from toolkit configuration to use this default, or retain an explicit value to control HTTP admission. The configured HTTP limit and any proxy limits must accommodate the inputs your tool accepts. They do not change the resolved execution budget. Chain inline limits and the signed response size limits still apply. Large results must be uploaded explicitly before encoding.

For more detailed instructions and examples, visit the [Nexus Toolkit docs][nexus-toolkit-docs].

<!-- List of references -->

[nexus-cli-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/cli
[nexus-toolkit-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/toolkit-rust
