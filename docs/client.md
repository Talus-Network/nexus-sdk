# 🪐 [`NexusClient`] (Rust)

The [`NexusClient`] provides a high-level interface for interacting with Nexus. It wraps key functionality including transaction signing and execution, gas management, cryptographic handshakes, and DAG workflow execution.

---

## ✨ Overview

The [`NexusClient`] provides access to:

- [`GasActions`] — manage gas coins and budgets
- [`CryptoActions`] — perform cryptographic handshakes with Nexus
- [`WorkflowActions`] — publish and execute workflows (DAGs)

You can initialize a `NexusClient` via [`NexusClient::builder()`] using either:

- a **Sui wallet context**, or
- a **Secret mnemonic and Sui client**

---

## ⚙️ Building a `NexusClient`

```rust
use nexus_sdk::nexus::client::NexusClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build the Nexus client
    let client = NexusClient::builder()
        .with_wallet_context(/* your `sui_sdk::wallet_context::WalletContext */)
        .with_gas(vec![/* your gas coins */], 10_000_000)?
        .with_nexus_objects(/* your `nexus_sdk::types::NexusObjects` */)
        .build()
        .await?;

    println!("✅ Nexus client initialized!");

    Ok(())
}
```

Alternatively, you can initialize using a mnemonic:

```rust
use nexus_sdk::nexus::client::NexusClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build the Nexus client
    let client = NexusClient::builder()
        .with_mnemonic(/* secret mnemonic */, /* your `sui_sdk::SuiClient` */)?
        .with_gas(vec![/* your gas coins */], 10_000_000)?
        .with_nexus_objects(/* your `nexus_sdk::types::NexusObjects` */)
        .build()
        .await?;

    println!("✅ Nexus client initialized!");

    Ok(())
}
```

---

## 🔑 Signer

### Supported Signer Types

| Type               | Description                                                       |
| ------------------ | ----------------------------------------------------------------- |
| `Signer::Wallet`   | Uses a local [`WalletContext`] for signing transactions           |
| `Signer::Mnemonic` | Uses a [`SuiClient`] + in-memory keystore derived from a mnemonic |

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
use nexus_sdk::nexus::client::NexusClient;

let coin_object_id = sui_types::base_types::ObjectID::random();

let result = nexus_client.gas().add_budget(coin_object_id).await?;

println!("Gas budget added in tx: {:?}", result.tx_digest);
```

**What it does:**

- Fetches the given coin object.
- Adds it to Nexus as available gas budget for workflows.
- Returns the transaction digest.

**Returns:**

[`AddBudgetResult`] — includes the transaction digest.

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

[`HandshakeResult`] — includes session data and transaction digests for claim and association steps.

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

[`PublishResult`] — includes the transaction digest and DAG object ID.

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

[`ExecuteResult`] — includes the transaction digest and execution object ID.

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

[`InspectExecutionResult`] — includes an event stream and a poller handle.

---

## 🧱 Module Summary

| Module            | Purpose                                                 |
| ----------------- | ------------------------------------------------------- |
| `nexus::client`   | Core client, builder, signer, gas management            |
| `nexus::crypto`   | Cryptographic operations (X3DH, sessions, handshakes)   |
| `nexus::gas`      | Gas management for Nexus workflows                      |
| `nexus::workflow` | DAG workflow publishing, execution, and event streaming |

---

## 🧭 Error Handling

All methods return a `Result<T, NexusError>`.
The `NexusError` enum categorizes issues from configuration errors to RPC and transaction issues.

---

## 🪶 Summary

The [`NexusClient`] aims to make building, publishing, and executing Nexus workflows _simple, safe, and async-ready_. It abstracts away Sui transaction signing and gas management while providing a clean modular interface.
