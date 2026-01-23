# nexus-sdk

> [!NOTE]
> This is an internal crate intended primarily for use within other Nexus
> packages. For Nexus Tool development, please use the higher-level
> [Nexus Toolkit][nexus-toolkit-docs].

## Usage

Generally, you won't need to depend on this crate directly. Instead, use the
[Nexus Toolkit][nexus-toolkit-docs], which provides interfaces for Nexus Tool
development.

However, if you specifically require direct access to internal helper functions,
you can include this crate in your project's `Cargo.toml` file:

```toml
[dependencies.nexus-sdk]
git = "https://github.com/Talus-Network/nexus-sdk"
tag = "v0.5.0"
package = "nexus-sdk"
```

## Signed HTTP (Leader nodes <-> Tools)

This crate includes the signed HTTP protocol used for Leader node <=> Tool communication:

- Ed25519 signatures over a small JSON “claims” blob shipped in `X-Nexus-Sig-*` headers
- request/response binding, body integrity binding (SHA-256), freshness windows, and replay resistance

It is feature-gated under `signed_http` and is used by `nexus-toolkit` to authenticate `/invoke` requests and sign responses.

See the end-to-end guide:

- [Tool communication guide](https://github.com/Talus-Network/nexus-sdk/blob/main/docs/guides/tool-communication.md)

<!-- List of references -->

[nexus-toolkit-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/toolkit-rust
