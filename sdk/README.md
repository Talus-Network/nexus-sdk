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

## Execution limits

[`execution_limits`](src/execution_limits.rs) defines the shared HTTP execution
contract for the SDK, CLI, Leader and Toolkit. Use these constants rather than
copying their values or selecting a different execution budget locally.

| Constant | Limit | Scope |
| --- | --- | --- |
| `MAX_RESOLVED_DATA_BYTES` | 8 MiB | All resolved bytes in one input set or one output set |
| `MAX_INVOKE_BODY_BYTES` | 12 MiB | Encoded HTTP invocation body, including base64 and metadata |

Inputs and outputs have independent budgets. Each invocation has its own budget;
the limit is not a total for an entire DAG. Inline and downloaded bytes count
equally, including every value in a `Many` port. Object IDs count as 32 bytes.
The toolkit rejects configuration that declares a different invocation body limit.
Changes to these limits require a coordinated SDK and runtime release.

These SDK rules do not change the published Move contracts. Their inline, port,
and encoded output bounds remain defined by the generated `protocol_limits`.

## Walrus storage

Enable `walrus_native` for wallet funded uploads and management. `full` includes
reading and protocol references without the native upload dependencies. The
native dependency is pinned to the Walrus revision that uses the workspace's
Sui version.

`nexus::wallet::WalletClient` owns one signing key and RPC connection. Nexus and
Walrus share it; no executable, temporary wallet file, or second key store is
needed. An existing `NexusClient` exposes `wallet()`. A standalone application
can call `WalletClient::connect(rpc_url, key)` and pass its clone to
`NexusClient::builder().with_wallet(wallet)`.

```rust,ignore
use nexus_sdk::walrus::{UploadOptions, WalrusStorage};

let storage = WalrusStorage::new(client.wallet()?.clone(), None).await?;
let stored = storage.upload(
    serde_json::to_vec(&result)?,
    UploadOptions { epochs: 5, ..Default::default() },
).await?;
let output_port = stored.nexus_data()?;
```

The connected Sui chain selects the testnet or mainnet deployment. The wallet
pays WAL for storage and SUI for gas and owns the Blob object. The SDK encodes
the data, registers storage, uploads to storage nodes, certifies it on Sui,
and verifies the aggregator readback. Each transaction receives a gas budget.
A separate WAL coin caps storage spending at the quote, subject to the caller's
maximum cost. Uploads are permanent until expiry unless explicitly deletable.
Only the Blob owner can extend storage or delete a deletable blob.

For durable operations, use `prepare`, `registration`, `register`, and `finish`.
Persist the signed registration before calling `register`, then persist the
returned `PendingUpload` before `finish`. After an uncertain submission, retry
that saved signed transaction. Do not create another registration. Saved
transactions are signature checked before submission. The convenience `upload`
method returns recovery information in `UploadError`; applications that may be
cancelled or restarted should persist each phase themselves.

`WalrusReader` needs only an aggregator. It bounds downloads, verifies SHA256,
and resolves canonical references into transient execution values. A combined
execution budget applies to all ports in one HTTP tool call, as defined above. The chain's
inline and encoded port limits remain unchanged. Sui tools still require
resolved data to fit their transaction limits. Reads preserve exact bytes,
including JSON formatting, so the digest authenticated by a tool remains valid.

`WalrusReference` carries local network, ownership, expiry and content metadata.
Only `NexusValue::WalrusData` blob IDs and digests become protocol inputs or
outputs. `scheduler::TaskInputPlan` validates selectors and port shapes before
payment and checks that upload results commit to the intended bytes. Call the
scheduler's authoritative `preflight_task_inputs` before materializing a plan.

Storage expiry is independent of task lifetime. The owner must retain data for
all future task occurrences and extend it before expiry. Walrus data is public;
a blob reference does not grant confidentiality.

## Runtime observations

Applications can install a `sui::observation::Observer` once before starting SDK
work. The optional interface reports logical reads, callback attempts, retry
waits, recovery admission, and RPC completion through final gRPC trailers.
Dropped futures report cancellation. RPC hooks cover clients created through
`sui::grpc::client`; constructing the reexported raw client bypasses that layer.

This interface does not register metrics or configure an exporter. The application
owns metric names, storage, and export. Callbacks must remain fast and must not
block, perform I/O, or panic. Operation and method variants are bounded, and no
execution identities or raw endpoint URLs are included. Recovery traffic denotes
admission policy and does not imply that an individual read has already failed.

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
