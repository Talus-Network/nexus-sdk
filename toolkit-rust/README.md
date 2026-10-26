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

Nexus Tools are HTTP servers. Registered production endpoints are expected to use **HTTPS**, which Leader nodes validate using the server certificate and hostname, and **signed HTTP** authenticates `POST /invoke`.

- The Toolkit serves HTTP when both `NEXUS_TOOL_TLS_CERT_PATH` and `NEXUS_TOOL_TLS_KEY_PATH` are absent; set both to terminate ordinary server TLS locally, and startup rejects a partial pair. Required signed HTTP does not require listener TLS.
- The Leader presents a self-signed client certificate derived from its existing signed-HTTP Ed25519 key when Toolkit requests one. Signed requests retain the existing input signature and add `nexus-context-sig` plus `nexus-context-timestamp`; the context signature covers the receiving Tool FQN, stable invocation nonce, and timestamp using `SHA-256(BCS((String, [u8; 32], u64)))`.
- Set `signed_http.tools.<tool_id>.require_client_certificate` to `true` to require the TLS peer key to match the allowlisted Leader key authenticating that Tool's signed invocation. It defaults to `false`; missing or mismatched peer identity rejects `/invoke` before replay-cache access or execution.
- Required mode checks the current allowed Leader key and context freshness before replay-cache access. The per-Tool `context_timestamp_window_ms` setting defaults to a symmetric, inclusive 30,000 ms window; retries keep their nonce and use a fresh timestamp.
- `/meta` always advertises `https`, and retains the existing forwarded-host and request-path behavior. Gateways must preserve the context headers and existing signature headers.
- A reverse proxy may terminate TLS while client-certificate verification is disabled. When it is enabled, terminate TLS in Toolkit or use TLS passthrough; forwarded client-certificate headers never establish peer identity.
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
