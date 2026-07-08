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

## Migration Order

1. Replace imports from removed mirror modules with generated bindings.
1. Rebuild TAP skill inputs around `TapPublishArtifact`.
1. Replace endpoint revision flows with current skill update flows.
1. Replace scheduled execution helpers with scheduled task APIs.
1. Replace old payment source bytes with typed payment source helpers.
1. Run SDK checks, then fix any remaining compile errors at the import boundary.

## Import Map

| Old shape | New shape |
| --- | --- |
| `nexus_sdk::idents::*` | Generated call targets and tag helpers in `nexus_sdk::move_bindings` |
| Hand maintained Move mirrors in `nexus_sdk::types::*` | Package scoped generated types in `nexus_sdk::move_bindings::*` |
| Endpoint revision records | Current skill records and `SkillRevisionContext` |
| `TapVertexAuthorizationSchema` | `SkillRequirement.fixed_tools` |
| Endpoint config digest fields | No direct replacement in active TAP flows |
| Shared object requirement fields in TAP artifacts | No direct replacement in active TAP flows |
| `TapActions::schedule_skill_execution*` | `TapActions::create_agent_task` or `SchedulerActions::create_task` |
| `WorkflowActions::inspect_execution(execution, checkpoint, ...)` | `WorkflowActions::inspect_execution(execution, ...)` |

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

## Scheduled Task Migration

The old scheduled TAP execution helpers were attach style APIs. The new model
creates a scheduled task first, then runs occurrences from that task.

Use `TapActions::create_agent_task` when the task is tied to an explicit agent
and skill:

```rust
use nexus_sdk::nexus::{
    scheduler::GeneratorKind,
    tap::{AgentTaskPayment, CreateAgentTaskParams},
};

let task = client
    .tap()
    .create_agent_task(CreateAgentTaskParams {
        entry_group: "default".to_owned(),
        input_data,
        metadata: vec![],
        execution_priority_fee_per_gas_unit: 0,
        initial_schedule: None,
        generator: GeneratorKind::Queue,
        agent_id,
        skill_id,
        payment: AgentTaskPayment::UserFunded {
            prepay_amount,
            refund_recipient: None,
            occurrence_budget,
            selected_dag: None,
            authorization_templates: vec![],
        },
    })
    .await?;
```

Use `SchedulerActions::create_task` when creating a default agent scheduler
task. If you provide `agent_id`, you must also provide `skill_id`; if you omit
one, omit both.

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

Regenerate the IR only when the published Move ABI changes:

```sh
just sdk rebind ../nexus/sui http://127.0.0.1:9000
```

This requires a published objects TOML and a running Sui gRPC endpoint. See the
SDK just recipe for exact defaults [Rebind command].

## Checks

Run these after migration:

```sh
just sdk check
just sdk test
```

If compile errors remain, handle them in this order:

1. Fix imports so Move types come from `move_bindings`.
1. Replace old TAP artifact fields with `SkillRequirement`.
1. Replace old schedule helpers with task creation.
1. Replace manual payment bytes with `PaymentSourceKind`.
1. Recheck workflow result calls against the split result flow.

## References

See the SDK overview [SDK README] and the full change list [Changelog].

[SDK README]: ./README.md
[Changelog]: ../CHANGELOG.md
[Rebind command]: ./.just
