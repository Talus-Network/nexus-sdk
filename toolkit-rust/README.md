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
nexus-toolkit = "2.1.0"
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

For more detailed instructions and examples, visit the [Nexus Toolkit docs][nexus-toolkit-docs].

<!-- List of references -->

[nexus-cli-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/cli
[nexus-toolkit-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/toolkit-rust

## Invocation failure isolation

With `panic = "unwind"`, the runtime catches panics in input decoding, tool
construction, authorization, execution, and output serialization. The caller
receives a generic HTTP 500 response without the panic payload. Failed invocations
are not signed as tool results. Other requests can continue.

Panic recovery is a property of this Rust runtime, not a Nexus protocol requirement.
Tools may use `panic = "abort"`, but a panic then terminates their process. To enable
recovery, set `panic = "unwind"` in the final application's `[profile.release]`.
Cargo ignores profiles declared by dependencies.

The runtime also applies `NexusTool::timeout()` to asynchronous invocation work.
A deadline returns HTTP 504. This cancellation cannot undo external side effects,
and it cannot preempt synchronous computation or recover from memory exhaustion.
Tools must bound their own synchronous work and intermediate allocations.

## Optional input utilities

The default toolkit provides the `NexusTool` contract and runtime without enabling
these utilities. Any tool author can enable either feature independently:

```toml
nexus-toolkit = { version = "2.1.0", features = ["schema", "network"] }
```

The `schema` feature adds `schema::compile` for JSON schemas accepted as tool input.
It permits references within the supplied document and refuses retrieval from files
or remote endpoints, even when another dependency enables the schema library's
retrieval features. Compilation errors do not expose referenced values. Tools that
do not enable this feature do not acquire its schema compiler dependency. This is
separate from the input and output schemas that every tool declares.

The `network` feature adds an HTTP client with an explicit destination policy:

```rust
use nexus_toolkit::network::{Client, DestinationPolicy};

let client = Client::builder(DestinationPolicy::Public)
    .redirect_limit(3)
    .build()?;
let response = client.get("https://example.com/data")?.send().await?;

// A trusted internal service is a separate, explicit policy.
let internal = Client::builder(DestinationPolicy::Origin(
    "http://inventory.internal:8080".parse()?,
))
.build()?;
```

Clients check initial requests and redirects automatically. `Client::execute` also
checks the final URL of a built or modified request. The public policy validates the
exact DNS answers used by the connection. The origin policy permits one configured
scheme, host, and effective port, including private addresses; it permits all paths
at that origin. The operator must choose this origin, not an invocation's author.
Both policies disable environment proxies and default to no redirects and a 30
second request timeout.

These utilities do not add trait methods, restrict other HTTP clients, or sandbox
native tool code. File access, credential selection, and additional service rules
remain responsibilities of the tool and its deployment.
