# nexus-sdk

> [!NOTE]
> This is the lower level crate for applications that interact directly with Nexus. For Nexus Tool development, use the [Nexus Toolkit][nexus-toolkit-docs].

## Usage

Most Nexus Tool projects should use the [Nexus Toolkit][nexus-toolkit-docs], which provides interfaces for Nexus Tool development.

Applications that need direct RPC, transaction, or protocol type access can add this crate to `Cargo.toml`:

```toml
nexus-sdk = { version = "2.1.0", features = ["full", "move_publish"] }
```

Version `2.1.0` includes Rust API changes. Read the [migration guide][migration] before updating from `2.0.0`.

Move package transaction construction requires the `move_publish` feature. It accepts compiled module bytes and dependency package IDs through `MovePackageArtifact`; project compilation remains the responsibility of the caller.

If you are upgrading direct SDK usage after the move to generated Move bindings,
see the [SDK migration guide](./MIGRATION.md).

## Recovery reads

Recovery returns only a complete, validated inventory. Failed requests, interrupted streams, missing history, and invalid responses leave recovery pending while it retries. Transaction scans retain validated IDs and resume from their last confirmed cursor. Each read or wait for a stream frame times out after 30 seconds; retry delays increase to at most five seconds and include jitter.

Callers can cancel recovery by dropping its future or selecting it against a cancellation signal. Keep readiness disabled until recovery completes. Warnings identify the failed operation and its next retry delay; a persistently invalid endpoint needs correction before recovery can finish.

`nexus::recovery::retry_read` applies the same policy to application recovery reads. Its callback must validate a complete result and must not perform mutations.

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
[migration]: https://github.com/Talus-Network/nexus-sdk/blob/v2.1.0/sdk/MIGRATION.md#upgrading-from-200-to-210
