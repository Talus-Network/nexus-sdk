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

Create a `nexus::recovery::RecoveryReader` with the RPC URL and a `ListConfig` policy. The reader shares that policy across window selection, transaction discovery, metadata, and application reads. Existing `RecoveryWindow` and `discover_work_objects` entry points use the same reader with the Sui client's defaults.

Transaction discovery uses the Sui client's resumable List API and lets the server choose its request size. Transport deadlines bound individual RPCs and stalled response bodies; recovery has no overall read deadline. Checkpoint searches retain their bounds when a read fails. Completed scan ranges and metadata batches are retained while other reads recover.

Recovery accepts a frame only after validating its full payload, and commits its IDs and resume cursor together. A scan completes only at its requested checkpoint bound. An earlier ledger tip, missing history, invalid response, or failed read leaves recovery pending. `RecoveryReader::read` applies the configured retry delays to application reads; its callback must validate its result and must not perform mutations. Retry delays use the policy's initial delay, maximum exponential delay, and additive jitter. List observers can report transport retries and resumed progress.

Callers can cancel by dropping the recovery future or selecting it against a cancellation signal. Keep readiness disabled until recovery completes. Persistent coverage or response problems need correction before recovery can finish; the reader never reports a partial inventory as success.

## Execution recovery

`sui::grpc::with_read_retry_until` gives concurrent crawler preparation one observation deadline. Only failed transport reads repeat; completed sibling reads remain available. End the scope before an external effect, or use `without_read_retry` around that effect. Dropping preparation cancels reads and backoff. `set_retry_request_budget` bounds recovery requests across every pooled client for an endpoint. Ordinary requests bypass this budget; `with_retry_budget` applies it to an existing recovery attempt without replaying requests or changing transaction identity.

`OccurrenceHandle::resolve_expired` inspects current chain state, settles available results, refunds eligible invocations by exact identity, and settles a finished occurrence into its Task. It returns confirmed resolutions and the final observed occurrence, including work still awaiting eligibility. Repeating recovery after settlement performs no mutation. Independent executions remain concurrent. `cost().outstanding_invocation_ids()` exposes unresolved locks for inspection. The existing `abort_expired(Some(id))` operation remains available for a specific invocation.

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
