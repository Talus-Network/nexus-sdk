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
[dependencies.nexus-toolkit]
git = "https://github.com/Talus-Network/nexus-sdk"
tag = "v0.5.0"
package = "nexus-toolkit"
```

---

## HTTPS + signed HTTP

Nexus Tools are HTTP servers. Nexus expects Tools to be reachable over **HTTPS** (TLS certificate validated by Leader nodes via system roots) and to require **signed HTTP** (Ed25519 signatures in `X-Nexus-Sig-*` headers) for `POST /invoke`.

- The toolkit runtime is HTTP-only; deploy it behind a TLS terminator (reverse proxy / load balancer).
- To enforce signed HTTP in the runtime, set `signed_http.mode = "required"` in the toolkit config.
- Signed HTTP is application-layer authentication for `/invoke`: the Tool verifies which Leader node signed the request, and the Leader node verifies which Tool signed the response (with request/response binding and replay resistance).
- Signed HTTP is enabled via a JSON config file loaded from `NEXUS_TOOLKIT_CONFIG_PATH`.
- Signed HTTP verification happens after the TLS handshake. If you want to reduce unwanted TLS handshakes/traffic, apply edge policy at your TLS terminator (rate limiting, firewall/WAF, mTLS, or private ingress such as Cloudflare Tunnel).
- Nexus Leader nodes do not currently present client certificates when calling Tools (no mTLS client authentication today). In a future update, Nexus will support self-signed certificates and TLS client authentication (mTLS) for Tool communication.

See the end-to-end deployment guide:

- [Tool communication guide](https://github.com/Talus-Network/nexus-sdk/blob/main/docs/guides/tool-communication.md)

For more detailed instructions and examples, visit the [Nexus Toolkit docs][nexus-toolkit-docs].

<!-- List of references -->

[nexus-cli-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/cli
[nexus-toolkit-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/toolkit-rust
