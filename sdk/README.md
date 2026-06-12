# nexus-sdk

> [!NOTE]
> This is an internal crate intended primarily for use within other Nexus packages. For Nexus Tool development, please use the higher-level [Nexus Toolkit][nexus-toolkit-docs].

## Usage

Generally, you won't need to depend on this crate directly. Instead, use the [Nexus Toolkit][nexus-toolkit-docs], which provides interfaces for Nexus Tool development.

However, if you specifically require direct access to internal helper functions,you can include this crate in your project's `Cargo.toml` file:

```toml
[dependencies.nexus-sdk]
git = "https://github.com/Talus-Network/nexus-sdk"
tag = "v2.0.0-rc.2"
package = "nexus-sdk"
```

## Signed HTTP (Leader nodes <-> Tools)

This crate includes the signed HTTP protocol used for Leader node <=> Tool communication:

- Ed25519 signatures over a small JSON “claims” blob shipped in `X-Nexus-Sig-*` headers
- request/response binding, body integrity binding (SHA-256), freshness windows, and replay resistance

It is feature-gated under `signed_http` and is used by `nexus-toolkit` to authenticate `/invoke` requests and sign responses.

See the end-to-end guide:

- [Tool communication guide](https://github.com/Talus-Network/nexus-sdk/blob/main/docs/guides/tool-communication.md)

## Standard TAP Payments

The SDK models the current standard TAP payment interface, including the mandatory agent payment vault created for every Talus agent.

Relevant helpers include:

- `tap_payment_source_for_address(...)` for direct `create_agent_skill_payment` source bytes accepted by the Move policy.
- `TapPaymentSource::invoker(...)` and `TapPaymentSource::agent_vault(...)` for typed payment-source payloads used by SDK models and non-direct policy surfaces.
- `TapAgentPaymentVault` plus `fetch_agent_payment_vault(...)`.
- `tap::deposit_agent_payment_vault(...)` and `tap::withdraw_agent_payment_vault(...)` PTB builders.
- `gas::add_agent_budget(...)` for standard Talus agent-scoped gas funding.

Direct standard TAP payment creation currently follows the Move policy exactly: user-funded sources are empty or payer-address BCS, and agent-funded direct sources are agent-id address BCS. Agent-vault settlement uses the dedicated vault payment builder rather than typed source bytes in the direct builder.

<!-- List of references -->

[nexus-toolkit-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/toolkit-rust
