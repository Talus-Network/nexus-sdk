# nexus-sdk

> [!NOTE]
> This is the lower level crate for applications that interact directly with Nexus. For Nexus Tool development, use the [Nexus Toolkit][nexus-toolkit-docs].

## Usage

Most Nexus Tool projects should use the [Nexus Toolkit][nexus-toolkit-docs], which provides interfaces for Nexus Tool development.

Applications that need direct RPC, transaction, or protocol type access can add this crate to `Cargo.toml`:

```toml
nexus-sdk = { version = "2.0.0", features = ["full", "move_publish"] }
```

Move package transaction construction requires the `move_publish` feature. It accepts compiled module bytes and dependency package IDs through `MovePackageArtifact`; project compilation remains the responsibility of the caller.

If you are upgrading direct SDK usage after the move to generated Move bindings,
see the [SDK migration guide](./MIGRATION.md).

## Signed HTTP (Leader nodes <-> Tools)

This crate includes the signed HTTP protocol used for Leader node <=> Tool communication:

- the Leader signs `SHA-256(BCS(canonical_tool_inputs))`
- the Tool signs the Leader signature followed by `SHA-256(result_bytes)`
- Leader identity, active key selection, and the Tool's replay/cache nonce remain transport headers rather than signed claims

It is feature-gated under `signed_http` and is used by `nexus-toolkit` to authenticate `/invoke` requests and sign responses.

## Standard TAP Payments

The SDK models the current standard TAP payment interface, including the mandatory agent payment vault created for every Talus agent.

Relevant helpers include:

- `tap_payment_source_for_address(...)` for direct `create_agent_skill_payment` source bytes accepted by the Move policy.
- `TapPaymentSource::invoker(...)` and `TapPaymentSource::agent_vault(...)` for typed payment-source payloads used by SDK models and non-direct policy surfaces.
- `TapAgentPaymentVault` plus `fetch_agent_payment_vault(...)`.
- `tap::deposit_agent_payment_vault(...)` and `tap::withdraw_agent_payment_vault(...)` PTB builders.

Direct standard TAP payment creation currently follows the Move policy exactly: user-funded sources are empty or payer-address BCS, and agent-funded direct sources are agent-id address BCS. Agent-vault settlement uses the dedicated vault payment builder rather than typed source bytes in the direct builder.

<!-- List of references -->

[nexus-toolkit-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/toolkit-rust
