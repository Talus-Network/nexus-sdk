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

- The Toolkit can terminate TLS directly when `NEXUS_TOOL_TLS_CERT_PATH` and `NEXUS_TOOL_TLS_KEY_PATH` are both set. When `signed_http.mode = "required"`, both paths are required and startup rejects a plaintext listener.
- The Leader automatically presents an in-memory self-signed client certificate derived from its configured signed-HTTP Ed25519 key. The Tool accepts signed `/invoke` only when the TLS peer key exactly matches the current allowlist entry selected by the request's Leader ID and key ID.
- Client certificates are requested but optional during the TLS handshake, so health and metadata routes remain public. Signed invokes without a client certificate or with a different registered Leader's certificate are rejected before replay-cache access.
- Keep TLS end to end between the Leader and Toolkit, or use a TCP/TLS-passthrough load balancer. A TLS-terminating reverse proxy removes the Leader identity and cannot forward it through HTTP headers.
- The Leader continues validating the Tool server certificate and hostname, requires HTTPS, and applies its existing proxy, redirect, timeout, and destination restrictions.
- Signed HTTP is enabled through a JSON config file loaded from `NEXUS_TOOLKIT_CONFIG_PATH`; its v3 input and result signature bytes are unchanged.

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
