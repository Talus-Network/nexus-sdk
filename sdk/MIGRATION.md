# SDK Migration Guide

This guide covers direct SDK migration after `nexus-sdk` moved to generated
Move bindings. It is for code that imports `nexus-sdk` directly. Toolkit users
should follow the Toolkit guide instead of depending on this crate unless they
need SDK internals.

## Goal

Move callers from old hand maintained SDK mirror types to the generated Move
binding boundary, while keeping workflow code on the high level `NexusClient`
actions where possible.

The changelog records every change. This guide gives the smallest path that
preserves behavior.

## Model

The new SDK has four layers.

| Layer | Purpose | Caller rule |
| --- | --- | --- |
| Move packages | Source of truth for persisted shapes and entry calls | Do not mirror these shapes locally |
| `move_bindings` | Generated Rust view of the Move ABI | Import Move structs, enums, type tags, and call targets from here |
| `transactions` | PTB builder layer | Use only when composing a custom transaction |
| `NexusClient` actions | Workflow layer | Prefer this for agent, skill, scheduler, payment, and execution flows |

The main invariant is simple: every value that crosses the Move boundary must
come from `nexus_sdk::move_bindings` or from an SDK helper that returns such a
value. Old local mirror types are no longer the authority.

The active `Protocol` root is the authority for package and shared object
bindings. A standard `NexusClient` action resolves that configuration once at
the start of an operation and uses the resulting immutable snapshot throughout
the operation.

## Protocol Resolution

Build normal clients from the stable `Protocol` object rather than from a saved
set of package and shared object references. Every standard action resolves the
active supported protocol before reading state or building a transaction, then
uses one immutable snapshot for all nested work.

`NexusClient::refresh_protocol` returns a new client when explicit snapshot
control is needed. It does not mutate the original client.

Creating a custom `NexusTransaction` is an operation boundary. Call
`client.transaction().await?`; the returned transaction owns the resolved
snapshot and uses it for every call added to that transaction.

If activation occurs after an operation has been built but before submission,
submission returns `NexusError::StaleProtocol`. The next standard action uses
the active protocol automatically. The SDK does not repeat the failed action
because an ambiguous submission may already have executed. The caller must
inspect transaction or object state before deciding whether a retry is safe.

`NexusError::UnsupportedProtocolVersion` means the active protocol is newer
than this SDK understands. Install a newer SDK or CLI. Selecting an older live
protocol is not supported.

Protocol support does not imply that the SDK can decode every future object
schema. Each versioned state read checks the exact schema before decoding it.
`NexusError::UnsupportedStateSchema` means the operation requires a state
binding that this SDK does not contain. Install the SDK built for that state
schema. The SDK never attempts to decode the payload as an older type.

The initial NetworkAuth reader supports schema one and rejects schema two
before decoding. Supporting a new schema requires its generated binding, an
explicit projection in the reader, and a consumer release whose protocol
support includes that change. The end to end release fixture prepares such a
consumer for schema two without changing the initial release boundary.

## Migration Order

1. Build the client from the stable `Protocol` root.
2. Replace imports from removed mirror modules with generated bindings.
3. Move Tool registration and payment code from `gas` to `tool`.
4. Rebuild TAP skill inputs around `TapPublishArtifact`.
5. Replace endpoint revision flows with current skill update flows.
6. Replace scheduled execution helpers with scheduled task APIs.
7. Replace old payment source bytes with typed payment source helpers.
8. Run SDK checks, then fix any remaining compile errors at the import boundary.

## Import Map

| Old shape | New shape |
| --- | --- |
| `nexus_sdk::idents::*` | Generated call targets and tag helpers in `nexus_sdk::move_bindings` |
| Hand maintained Move mirrors in `nexus_sdk::types::*` | Package scoped generated types in `nexus_sdk::move_bindings::*` |
| Endpoint revision records | Current skill records and `SkillRevisionContext` |
| `TapVertexAuthorizationSchema` | `SkillRequirement.fixed_tools` |
| Endpoint config digest fields | No direct replacement in active TAP flows |
| Shared object requirement fields in TAP artifacts | No direct replacement in active TAP flows |
| Scheduled execution request helpers | `TaskSpec` plus `Schedule` through `NexusClient::scheduler()` |
| `WorkflowActions::inspect_execution(execution, timeout)` | `WorkflowActions::inspect_execution(execution, InspectExecutionOptions { timeout, poll_interval })` |
| `move_bindings::gas` | `move_bindings::tool` |
| Nexus fee operations through `NexusClient::gas()` | `NexusClient::network()` |
| Tool price and ticket operations through gas APIs | `NexusClient::tool()` |
| Nexus GasService transaction builders | `transactions::tool_payment` or `transactions::network` according to state ownership |
| Shared `GasService` Tool payment state | The `ToolPayment` object derived from each `Tool` |
| `VerifierRegistry` | Tool verifier records in `ToolRegistry` and registered keys in `NetworkAuth` |

Use this import style for new TAP code:

```rust
use nexus_sdk::{
    move_bindings::interface::{
        agent::{SkillRequirement, SkillSchedulePolicy},
        payment::{PaymentSourceKind, SkillPaymentPolicy},
        version::InterfaceVersion,
    },
    types::{tap_input_commitment_from_dag_inputs, SkillConfig, TapPublishArtifact},
};
```

## TAP Skill Migration

Old TAP skill state was centered on endpoint revisions, config digests, shared
objects, and vertex authorization schemas. The new model is centered on a
current skill record:

```text
agent_id + skill_id
    -> current interface revision
    -> DAG binding
    -> skill requirements
```

Build skill inputs as a `SkillConfig`, then derive a `TapPublishArtifact` after
the DAG is published.

```rust
let requirements = SkillRequirement {
    input_commitment: tap_input_commitment_from_dag_inputs([("entry", "input")]),
    payment_policy: SkillPaymentPolicy::user_funded(),
    schedule_policy: SkillSchedulePolicy::default(),
    fixed_tools: vec![],
};

let config = SkillConfig {
    name: "math".to_owned(),
    dag_path: "dag.json".into(),
    requirements,
    interface_revision: InterfaceVersion::new(1),
};

let artifact = TapPublishArtifact::from_config(&config, dag_id)?;
```

Register a new skill with the artifact:

```rust
let tap = client.tap();

let agent = tap.create_agent().await?;
let skill = tap.register_skill(agent.agent_id, &artifact).await?;
```

Update an existing skill by publishing the new DAG and applying the new
artifact:

```rust
let update = tap
    .update_skill_from_artifact(agent.agent_id, skill.skill_id, &next_artifact)
    .await?;

let current_revision = update.current_interface_revision;
```

## Payment Migration

Payment policy now belongs to `SkillRequirement`.

```rust
let user_policy = SkillPaymentPolicy::user_funded();
let agent_policy = SkillPaymentPolicy::agent_funded(10_000_000);
```

Payment source bytes should be derived from `PaymentSourceKind` rather than
assembled manually.

```rust
let user_source = bcs::to_bytes(&PaymentSourceKind::user_funded(user_address))?;
let agent_source = bcs::to_bytes(&PaymentSourceKind::agent_funded(agent_id))?;
```

For the common user funded path, the SDK also accepts an empty source where the
active sender is the payer.

Tool invocation price, earnings, settings, and tickets now belong to the Tool
domain. Every registered Tool has one derived `ToolPayment` object. Tool
registration returns an `OverTool` owner capability and a separate
`OverToolPayment` administration capability.

Use the Tool owner capability for Tool identity and earnings. Use the payment
administration capability for invocation price and ticket configuration.

```rust
client
    .tool()
    .set_invocation_cost(&tool_fqn, payment_admin, 1_000_000)
    .await?;

client
    .tool()
    .enable_expiry_tickets(&tool_fqn, payment_admin, 100_000)
    .await?;
```

`transactions::gas` now contains only Sui transaction gas helpers such as
depositing SUI into an address balance. Nexus network economics live in
`transactions::network`. Tool payment builders live in
`transactions::tool_payment`.

## Scheduled Task Migration

Scheduling now has one public request model:

```text
Task -> Schedule -> Occurrence
```

Build work and funding with `TaskSpec`. Build timing with `Schedule`,
`Occurrence`, and `Recurrence`. Use `create_task` for an empty composable Task,
or `schedule_task` to create a Task and apply a nonempty complete Schedule in
one transaction.

```rust
use nexus_sdk::{
    scheduler::{
        Occurrence, Schedule, TaskFunding, TaskOperation, TaskSpec,
    },
    types::DEFAULT_ENTRY_GROUP,
};

let task = TaskSpec::new(
    TaskOperation::default_dag(dag_id),
    DEFAULT_ENTRY_GROUP,
    TaskFunding::address(50_000_000),
    50_000_000,
)?
.with_inputs(input_data);

let receipt = client
    .scheduler()
    .schedule_task(task, Schedule::new().with_occurrence(Occurrence::now()))
    .await?;
let occurrence = receipt
    .delta()
    .scheduled()
    .first()
    .expect("schedule contains one occurrence")
    .reference();
let snapshot = client
    .scheduler()
    .task(occurrence.task_id())
    .occurrence(occurrence.occurrence_id())
    .snapshot()
    .await?;
```

Inspect Tasks and occurrences through their client handles. Occurrence
snapshots remain available after settlement and after the Task is closed.

## Workflow Result Migration

On chain tool result flows now split producer work from settlement work.

| Old intent | New action |
| --- | --- |
| Submit an off chain result | `transactions::dag::submit_off_chain_tool_result_for_walk_ptb` |
| Submit an on chain result | `transactions::dag::submit_on_chain_tool_result_for_walk_ptb` |
| Consume a finalized on chain result | `transactions::dag::consume_on_chain_tool_result_for_walk_ptb` |
| Settle a committed result | `WorkflowActions::settle_committed_tool_result_for_walk` |
| Resolve an expired walk | `WorkflowActions::resolve_expired_walk` |

There is no SDK finalize helper in the active flow. Finalization happens in the
result object flow, and settlement consumes the finalized or committed state.

## Binding Regeneration

Generated bindings are built from committed normalized Move package IR under
`sdk/src/move_bindings/ir/*.json`. Normal SDK builds render Rust bindings from
that IR through `build.rs`.

Normal regeneration refreshes the six Nexus package files and preserves the reduced Move standard
library and Sui framework support IR. Update those framework files explicitly only when the pinned
Sui version changes.

Regenerate the IR only when the published Move ABI changes:

```sh
just sdk rebind ../nexus/sui/bin/target/objects.localnet.toml http://127.0.0.1:9000
```

This requires a published objects TOML and a running Sui gRPC endpoint. See the
SDK just recipe for exact defaults [Rebind command].

Sui package metadata does not contain function parameter names. The command therefore keeps the
network derived `argN` names unless a matching Move source tree is provided explicitly:

```sh
just sdk rebind \
  ../nexus/sui/bin/target/objects.localnet.toml \
  http://127.0.0.1:9000 \
  ../nexus/sui
```

The source tree only supplies parameter names. Signatures, types, and abilities still come from the
selected network package. Source names are trusted metadata and are not proof that the local source
produced the published bytecode.

Before writing the committed IR, regeneration replaces current and original Nexus package IDs with
stable SDK binding slots. This includes package IDs in cross package type references, so rebinding
the same ABI from another deployment produces the same canonical IR. The package object version is
also normalized because the renderer does not consume it. Runtime `NexusObjects` supplies current
package IDs for calls and original package IDs for type identity.

## Checks

Run these after migration:

```sh
just sdk check
just sdk test
```

If compile errors remain, handle them in this order:

1. Fix imports so Move types come from `move_bindings`.
2. Replace old TAP artifact fields with `SkillRequirement`.
3. Replace old schedule helpers with task creation.
4. Replace manual payment bytes with `PaymentSourceKind`.
5. Recheck workflow result calls against the split result flow.

## References

See the SDK overview [SDK README] and the full change list [Changelog].

[SDK README]: ./README.md
[Changelog]: ../CHANGELOG.md
[Rebind command]: ./.just
