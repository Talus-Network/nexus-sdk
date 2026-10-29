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

## Authentication

This branch implements **signed HTTP v3.1**, which adds a signed request context while retaining the `X-Nexus-Sig-V: 3` wire header and the SDK's `signed_http::v3` module. The strict version check still rejects `3.1` as a header value. Package versions are unchanged. See [Migrating from signed HTTP v3 to v3.1](../sdk/README.md#migrating-from-signed-http-v3-to-v31) for coordinated Leader and Toolkit upgrades; required mode rejects older requests without context headers.

Nexus Tools expose an HTTP API. **HTTPS** protects the connection between a Leader and a Tool. The Toolkit also provides configurable checks for request signatures and TLS client certificates, and can sign Tool results for on-chain verification.

- **HTTPS transport.** Leaders require HTTPS and validate the Tool's server certificate and hostname, with proxy, redirect, timeout, and destination restrictions. To terminate TLS in the Toolkit, set both `NEXUS_TOOL_TLS_CERT_PATH` and `NEXUS_TOOL_TLS_KEY_PATH` to nonempty paths containing a valid PEM certificate chain and matching private key. Setting only one path, or invalid credentials, prevents startup. When neither is set, the Toolkit serves plain HTTP and logs a warning; this is intended solely for hosts that terminate HTTPS in front of the Toolkit (e.g. Cloud Run or a load balancer), since Leaders never call `http://` Tool URLs. The `--meta` command does not start a server and does not require TLS credentials.
- **Input decoding.** In both modes, `/invoke` accepts the Leader's `ports` envelope and decodes its resolved values into the Tool input type. Plain input objects are not accepted. Omitting `signed_http` or setting its mode to `"disabled"` requires no Leader allowlist; requests are not authenticated and results are unsigned. HTTPS is still required.
- **Request signature verification (optional).** Load a JSON configuration file through `NEXUS_TOOLKIT_CONFIG_PATH`. Omitting `signed_http` or setting `signed_http.mode` to `"disabled"` disables these checks and response signing. When `signed_http` is present, its mode defaults to `"required"`: the runtime rejects `POST /invoke` requests with missing or invalid signatures or a Leader key outside the configured `allowed_leaders` / `allowed_leaders_path` allowlist. These checks run automatically before replay-cache access or execution. The runtime also checks that the resolved inputs match the signed input commitment. Tools can override `NexusTool::authorize` to apply additional policy to the authenticated request context; the default hook allows it.
- **Signed request context.** Alongside the input signature, the Leader sends `nexus-context-sig` and `nexus-context-timestamp`. The context signature covers the receiving Tool FQN, stable invocation nonce, and timestamp using `SHA-256(BCS((String, [u8; 32], u64)))`. Required mode checks the current allowed Leader key and context freshness before replay-cache access. The per-Tool `context_timestamp_window_ms` setting defaults to a symmetric, inclusive 30,000 ms window; retries retain their nonce and use a fresh timestamp.
- **TLS client-certificate verification (optional).** With signed HTTP enabled, set `signed_http.tools.<tool_id>.require_client_certificate` to `true` to additionally require the TLS peer's public key to match the allowlisted Leader key authenticating that Tool's invocation. This setting defaults to `false`. The Leader presents a self-signed client certificate derived from its signed-HTTP Ed25519 key when the Toolkit requests one. When the check is enabled, a missing or mismatched peer identity rejects `/invoke` before replay-cache access or execution.
- **TLS termination and proxies.** When client-certificate verification is disabled, a reverse proxy may terminate the Leader's TLS connection and connect to the Toolkit over HTTPS. When it is enabled, terminate TLS in the Toolkit or use TLS passthrough so the Toolkit receives the Leader's client certificate; forwarded client-certificate headers do not establish peer identity. Gateways must preserve the signature and context headers. `/meta` always advertises `https` and retains its forwarded-host and request-path behavior.
- **Signed Tool responses.** With signed HTTP enabled, configure `signed_http.tools.<tool_id>.response_signing_key` to have the Toolkit automatically sign each canonical Tool result and return the signature in the response headers. The Ed25519 signature binds the Leader's input signature, invocation nonce, and exact ordered BCS Tool output bytes for on-chain verification. The corresponding public key must be registered for the Tool's on-chain verifier. The response signing key is optional: without it, the runtime verifies requests but returns unsigned results. Local HTTP errors, including authentication failures and timeouts, are unsigned. The v3 input and result signature bytes are unchanged.

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
