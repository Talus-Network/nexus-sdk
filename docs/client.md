# 🪐 [`NexusClient`] (Rust)

The [`NexusClient`] provides a high-level interface for interacting with Nexus. It wraps key functionality including transaction signing and execution, gas management, cryptographic handshakes, and DAG workflow execution.

---

## ✨ Overview

The [`NexusClient`] provides access to:

- [`GasActions`]: manage gas coins and budgets
- [`CryptoActions`]: perform cryptographic handshakes with Nexus
- [`WorkflowActions`]: publish and execute workflows (DAGs)
- [`SchedulerActions`]: create and manage scheduler tasks, occurrences, and periodic schedules
- [`NetworkAuthActions`]: (when `signed_http` is enabled) manage message-signing key bindings for Tools/Leader nodes

You can initialize a `NexusClient` via [`NexusClient::builder()`] with:

- a Sui **ed25519 private key**
- an RPC URL (and optional GraphQL URL for event fetching)
- one or more gas coins + a gas budget
- the on-chain [`NexusObjects`]

---

## ⚙️ Building a `NexusClient`

```rust
use nexus_sdk::nexus::client::NexusClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build the Nexus client
    let client = NexusClient::builder()
        .with_rpc_url(/* your Sui RPC URL */)
        .with_private_key(/* Your wallet private key */)
        .with_gas(vec![/* your gas coins */], 10_000_000)
        .with_nexus_objects(/* your `nexus_sdk::types::NexusObjects` */)
        .build()
        .await?;

    println!("✅ Nexus client initialized!");

    Ok(())
}
```

---

## 🔏 Network Auth (signed HTTP)

When the `signed_http` feature is enabled, `NexusClient` exposes `network_auth()` for Tool/Leader node message-signing key operations:

- register/rotate a Tool message-signing key on-chain, and
- export a local allowlist file of permitted Leader nodes for Tool-side verification (no RPC at runtime).

This is the same functionality exposed via the CLI under `nexus tool signed-http ...`.

---

## 🔑 Signer

### Signing Mechanism

The `Signer` struct accepts a [`sui::crypto::Ed25519PrivateKey`] and is responsible for signing and executing transactions on behalf of the active wallet address.

### Key Public Behaviors

- Automatically signs and executes transactions.
- Keeps track of active wallet address.
- Keeps track of gas coins and updates the gas coin references after a transaction.
- Handles errors as [`NexusError`].

---

## ⛽ Nexus Gas Budget Management

Nexus gas budget is managed through the [`GasActions`] struct.

### Add Budget

```rust
use nexus_sdk::{nexus::client::NexusClient, sui};

let coin_object_id: sui::types::Address = /* your coin object ID */;

let result = nexus_client.gas().add_budget(coin_object_id).await?;

println!("Gas budget added in tx: {:?}", result.tx_digest);
```

**What it does:**

- Fetches the given coin object.
- Adds it to Nexus as available gas budget for workflows.
- Returns the transaction digest.

**Returns:**

[`AddBudgetResult`]: includes the transaction digest.

---

## 🔐 Cryptographic Actions

These actions manage secure handshakes and session setup.

### Perform a Handshake

```rust
use nexus_sdk::crypto::x3dh::IdentityKey;

let ik = IdentityKey::generate();

let handshake = nexus_client.crypto().handshake(&ik).await?;

println!("Session established!");
println!("Claim TX: {:?}", handshake.claim_tx_digest);
println!("Associate TX: {:?}", handshake.associate_tx_digest);

assert!(handshake.session);
```

**What it does:**

1. Claims a pre-key object from Nexus.
1. Fetches the corresponding [`PreKeyBundle`].
1. Initiates an [`X3DH`] handshake to create a secure [`Session`].
1. Associates the pre-key with the sender on-chain.

**Returns:**

[`HandshakeResult`]: includes session data and transaction digests for claim and association steps.

---

## ⚡ Workflow Actions

The [`WorkflowActions`] API allows you to publish, execute, and inspect **DAG-based workflows** on Nexus.

### 1. Publish a DAG

```rust
use nexus_sdk::types::Dag;

let dag = serde_json::from_str::<Dag>(include_str!(/* path to your DAG JSON file */))?;

let publish_result = nexus_client.workflow().publish(dag).await?;

println!("Published DAG ID: {:?}", publish_result.dag_object_id);
```

**What it does:**

- Builds and submits a programmable transaction that creates and publishes the DAG object.
- Returns the transaction digest and created DAG object ID.

**Returns:**

[`PublishResult`]: includes the transaction digest and DAG object ID.

---

### 2. Execute a Workflow

```rust
use nexus_sdk::types::{PortsData, StorageConf};
use nexus_sdk::crypto::session::Session;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

// Prepare input ports data
let mut entry_data: HashMap<String, PortsData> = /* your input data */

// Prepare storage and session
let storage_conf = StorageConf::default();
let session = Arc::new(Mutex::new(Session::default()));

let execute_result = nexus_client
    .workflow()
    .execute(
        publish_result.dag_object_id,
        entry_data,
        None, // use default entry group
        &storage_conf,
        session,
    )
    .await?;

println!("Execution object ID: {:?}", execute_result.execution_object_id);
```

**What it does:**

1. Commits input data to the appropriate storage (inline, Walrus).
1. Constructs and executes a transaction to start the workflow execution.
1. Returns the transaction digest and new DAG execution object ID.

**Returns:**

[`ExecuteResult`]: includes the transaction digest and execution object ID.

---

### 3. Inspect Workflow Execution

```rust
use tokio::time::Duration;

let inspect = nexus_client
    .workflow()
    .inspect_execution(
        execute_result.execution_object_id,
        execute_result.tx_digest,
        Some(Duration::from_secs(180)), // timeout
    )
    .await?;

let mut event_stream = inspect.next_event;

// Listen for events
while let Some(event) = event_stream.recv().await {
    println!("Event: {:?}", event);
}

// Await the poller completion
inspect.poller.await??;

println!("✅ Execution finished successfully!");
```

**What it does:**

- Polls Nexus events on Sui to stream live workflow execution updates.
- Automatically stops when an `ExecutionFinished` event is detected or timeout is reached.

**Returns:**

[`InspectExecutionResult`]: includes an event stream and a poller handle.

---

## ⏱️ Scheduler Actions

The [`SchedulerActions`] API allows you to create and manage **on-chain scheduler tasks**.

A scheduler task is split into:

- an **execution policy** (“what to run”): today this is “begin DAG execution”, but tasks are designed to support additional execution types in the future
- a **constraints policy** (“when it may run”): defines when the task is eligible to execute. In the current scheduler this eligibility is time-based and expressed via **occurrences** (start + optional deadline windows) produced by either queue-based scheduling or periodic scheduling

The scheduler APIs are task/schedule/occurrence oriented; starting DAG executions is just the current default execution policy.

Tasks also carry metadata and lifecycle state (active/paused/canceled), which you can update via `update_metadata` and `set_task_state`.

An **occurrence** is an eligibility window for a single task run (start time + optional deadline + `priority_fee_per_gas_unit`). When multiple occurrences are eligible, ordering is deterministic: earlier start wins; ties break on higher `priority_fee_per_gas_unit`; then FIFO.

Each eligible (consumed) occurrence triggers one run of the task’s execution policy. This means the same execution definition can run multiple times:

- **Periodic tasks**: the scheduler generates occurrences automatically from a `(first_start_ms, period_ms, …)` config, so the same execution runs periodically.
- **Queue tasks**: you can enqueue any number of occurrences; the task’s execution runs once per occurrence.

Each run is independent: the scheduler does not automatically pass outputs/data from one run to the next. If you need stateful behavior across runs (e.g., chaining results, retries with state, counters/backoff), persist and manage that state externally.

Queue-based scheduling is intentionally generic: by enqueueing occurrences at different times (and with different priorities), you can implement delayed runs, retries, backoff, and other custom strategies.

### 1. Create a Queue Task

```rust
use nexus_sdk::{
    nexus::scheduler::{CreateTaskParams, GeneratorKind},
    types::{NexusData, DEFAULT_ENTRY_GROUP},
};
use std::collections::HashMap;

let input_data = HashMap::from([(
    "entry_vertex".to_string(),
    HashMap::from([(
        "input_port".to_string(),
        NexusData::new_inline(serde_json::json!({"hello": "world"})).commit_inline_plain(),
    )]),
)]);

let queue_task = nexus_client
    .scheduler()
    .create_task(CreateTaskParams {
        dag_id: publish_result.dag_object_id,
        entry_group: DEFAULT_ENTRY_GROUP.to_string(),
        input_data,
        metadata: vec![("env".into(), "demo".into())],
        execution_priority_fee_per_gas_unit: 0,
        initial_schedule: None,
        generator: GeneratorKind::Queue,
    })
    .await?;

println!("Queue task ID: {:?}", queue_task.task_id);
```

Queue-based tasks can enqueue the first occurrence at creation time by passing `initial_schedule: Some(OccurrenceRequest::new(...))`.

### 2. Enqueue a One-Off Occurrence

```rust
use nexus_sdk::nexus::scheduler::OccurrenceRequest;

let occurrence = OccurrenceRequest::new(
    Some(/* start_ms */),
    None, // deadline_ms
    None, // start_offset_ms
    Some(/* deadline_offset_ms */),
    0,    // priority_fee_per_gas_unit
    true, // require_start
)?;

let scheduled = nexus_client
    .scheduler()
    .add_occurrence(queue_task.task_id, occurrence)
    .await?;

println!("Occurrence queued in tx: {:?}", scheduled.tx_digest);
```

### 3. Create a Periodic Task and Configure Scheduling

```rust
use nexus_sdk::{
    nexus::scheduler::{CreateTaskParams, GeneratorKind, PeriodicScheduleConfig},
    types::{NexusData, DEFAULT_ENTRY_GROUP},
};
use std::collections::HashMap;

let input_data = HashMap::from([(
    "entry_vertex".to_string(),
    HashMap::from([(
        "input_port".to_string(),
        NexusData::new_inline(serde_json::json!({"hello": "world"})).commit_inline_plain(),
    )]),
)]);

let periodic_task = nexus_client
    .scheduler()
    .create_task(CreateTaskParams {
        dag_id: publish_result.dag_object_id,
        entry_group: DEFAULT_ENTRY_GROUP.to_string(),
        input_data,
        metadata: vec![("env".into(), "demo".into())],
        execution_priority_fee_per_gas_unit: 0,
        initial_schedule: None,
        generator: GeneratorKind::Periodic,
    })
    .await?;

nexus_client
    .scheduler()
    .configure_periodic(
        periodic_task.task_id,
        PeriodicScheduleConfig {
            first_start_ms: /* absolute timestamp */,
            period_ms: /* period in milliseconds */,
            deadline_offset_ms: None,
            max_iterations: None,
            priority_fee_per_gas_unit: 0,
        },
    )
    .await?;

nexus_client
    .scheduler()
    .disable_periodic(periodic_task.task_id)
    .await?;
```

### 4. Update Task Metadata

```rust
let updated = nexus_client
    .scheduler()
    .update_metadata(queue_task.task_id, vec![("key".into(), "value".into())])
    .await?;

println!("Metadata updated in tx: {:?}", updated.tx_digest);
```

### 5. Pause / Resume / Cancel a Task

```rust
use nexus_sdk::nexus::scheduler::TaskStateAction;

nexus_client
    .scheduler()
    .set_task_state(queue_task.task_id, TaskStateAction::Pause)
    .await?;
```

---

## 🧱 Module Summary

| Module             | Purpose                                                 |
| ------------------ | ------------------------------------------------------- |
| `nexus::client`    | Core client, builder, signer, gas management            |
| `nexus::crypto`    | Cryptographic operations (X3DH, sessions, handshakes)   |
| `nexus::gas`       | Gas management for Nexus workflows                      |
| `nexus::workflow`  | DAG workflow publishing, execution, and event streaming |
| `nexus::scheduler` | Scheduler tasks, occurrences, and periodic schedules    |

---

## 🧭 Error Handling

All methods return a `Result<T, NexusError>`.
The `NexusError` enum categorizes issues from configuration errors to RPC and transaction issues.

---

## 🪶 Summary

The [`NexusClient`] aims to make building, publishing, and executing Nexus workflows _simple, safe, and async-ready_. It abstracts away Sui transaction signing and gas management while providing a clean modular interface.
