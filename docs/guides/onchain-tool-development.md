# Onchain Tool Development Guide

This guide walks you through building, publishing, and registering an *onchain
tool* for Nexus end to end. Onchain tools are Sui Move modules that the Nexus
workflow executes as part of an on-chain transaction — they are where you mutate
on-chain state, move assets, or do anything that must be verifiable and atomic
on Sui.

By the end you'll have a working counter tool: a shared object that the tool
increments on every invocation, returning the old and new values, and registered
in the Nexus tool registry.

{% hint style="info" %} Prerequisites

- Familiarity with the Sui Move language.
- Follow the [setup guide](setup.md) to make sure you've got the
  [Nexus CLI](../cli.md) and the
  [Sui CLI](https://docs.sui.io/guides/developer/getting-started/sui-install)
  installed and your environment configured.
- A basic understanding of Nexus workflows and DAGs — see the
  [DAG Construction Guide](dag-construction.md).
  {% endhint %}

## 1. What Is an Onchain Tool?

An onchain tool is a Move module with a standardized `execute` function that the
Nexus workflow calls on Sui. Two ideas make a Move module a Nexus tool:

- It **stamps a worksheet**. The `execute` function receives a
  `&mut ProofOfUID` worksheet and must stamp it with the tool's witness id. This
  proves to the framework that your tool actually ran.
- It returns a **`TaggedOutput`**. Instead of aborting on failure, `execute`
  returns a tagged output whose variant (for example `ok` or `err`) and named
  payloads become the tool's output variants and ports — exactly like an
  offchain tool's `Output` enum.

The module also declares an `Output` enum, but it is used **only for schema
generation** at registration time — the runtime emits the `TaggedOutput`, not
the enum.

How onchain tools compare to offchain tools:

| Aspect | Onchain tool | Offchain tool |
| --- | --- | --- |
| Runtime | Sui Move module | HTTP service (Rust) |
| Execution | Runs on Sui as part of the PTB | Leader invokes over HTTPS |
| Best for | On-chain state changes, asset moves | External APIs, LLMs, arbitrary compute |
| Proof of execution | Worksheet stamp (`ProofOfUID`) | Signed HTTP response |

If you need to wrap an external API or run off-chain computation instead, see
the [Offchain Tool Development Guide](offchain-tool-development.md).

## 2. Scaffold the Project

Create a new Move tool project with the Nexus CLI:

```bash
nexus tool new --name counter_tool --template move --target ./
cd counter_tool
```

This generates a ready-to-build Move package with a fully worked `execute`
function that you adapt to your logic. The rest of this guide explains each part
of the generated module so you understand what to change.

### Configure Dependencies

The generated `Move.toml` declares the Nexus package dependencies and a
placeholder address for your package:

```toml
[package]
name = "counter_tool"
edition = "2024.beta"

[dependencies]
nexus_primitives = { local = "path/to/nexus/primitives" }
nexus_interface = { local = "path/to/nexus/interface" }
nexus_workflow = { local = "path/to/nexus/workflow" }

[addresses]
counter_tool = "0x0"
```

{% hint style="warning" %}
Point the dependency sources and addresses at your target network. Use local
paths to a checked-out copy of the Nexus Move packages, or git revisions for the
network you're publishing to. These published addresses differ between
localnet, testnet, and mainnet, so never hard-code one network's addresses for
another.
{% endhint %}

## 3. Module Structure

In `sources/counter_tool.move`, the module starts with its imports and the core
objects: a one-time witness, a *tool witness* (the stamp locator), and your
tool's state.

```move
module counter_tool::counter_tool;

use nexus_interface::authorization::AgentVertexAuthorization;
use nexus_primitives::data;
use nexus_primitives::authorization::ProvenValue;
use nexus_primitives::proof_of_uid::ProofOfUID;
use nexus_primitives::tagged_output::{Self, TaggedOutput};
use sui::bag::{Self, Bag};
use sui::transfer::share_object;
use std::ascii::String as AsciiString;

/// One-time witness for package initialization.
public struct COUNTER_TOOL has drop {}

/// Witness object used as the worksheet stamp locator for this tool.
public struct CounterWitness has key, store {
    id: UID,
}

/// Your tool's state object.
public struct CounterState has key {
    id: UID,
    /// Stores the witness object that identifies this tool's stamp.
    witness: Bag,
    /// Application-specific state: the running count.
    count: u64,
}
```

The `init` function creates the state, stores the witness inside its `Bag`, and
shares the state object so the workflow can pass it into `execute`:

```move
/// Initialize the tool's shared state.
fun init(_otw: COUNTER_TOOL, ctx: &mut TxContext) {
    let state = CounterState {
        id: object::new(ctx),
        witness: {
            let mut bag = bag::new(ctx);
            bag.add(b"witness", CounterWitness { id: object::new(ctx) });
            bag
        },
        count: 0,
    };
    share_object(state);
}
```

## 4. Define the Output Variants

Declare an `Output` enum describing every variant your tool can emit. The Nexus
CLI reads this enum to generate the tool's output schema during registration.

```move
/// Tool execution output variants.
/// Used only for automatic schema generation during registration — the runtime
/// emits a `TaggedOutput`, not this enum.
public enum Output {
    Ok {
        old_count: u64,
        new_count: u64,
        increment: u64,
    },
    Err {
        reason: AsciiString,
    },
    LargeIncrement {
        old_count: u64,
        new_count: u64,
        increment: u64,
        warning: AsciiString,
    },
}
```

## 5. Implement the `execute` Function

`execute` is the core of the tool. It follows a standardized signature and must
stamp the worksheet before returning a `TaggedOutput`.

```move
/// Execute function with the standardized Nexus signature.
///
/// CRITICAL REQUIREMENTS:
/// 1. Internal authorization parameter: _authorization: ProvenValue<AgentVertexAuthorization>
/// 2. Internal workflow proof parameter: worksheet: &mut ProofOfUID
/// 3. Last parameter: ctx: &mut TxContext
/// 4. Return type: TaggedOutput
/// 5. Must stamp the worksheet with the tool witness id
public fun execute(
    _authorization: ProvenValue<AgentVertexAuthorization>,
    worksheet: &mut ProofOfUID,
    state: &mut CounterState,
    // Your custom input ports follow.
    increase_with: u64,
    ctx: &mut TxContext,
): TaggedOutput {
    // REQUIRED: stamp the worksheet with the tool witness id to prove execution.
    worksheet.stamp_with_data(&state.witness().id, b"counter_tool_executed");

    let old_count = state.count;

    if (increase_with == 0) {
        // Return an error variant instead of aborting, so the data flows on.
        tagged_output::new(b"err")
            .with_named_payload(b"reason", data::inline_one(b"Cannot increment by zero").as_string())
    } else if (increase_with > 100) {
        state.count = state.count + increase_with;
        tagged_output::new(b"large_increment")
            .with_named_payload(b"old_count", data::inline_one(old_count.to_string().into_bytes()).as_number())
            .with_named_payload(b"new_count", data::inline_one(state.count.to_string().into_bytes()).as_number())
            .with_named_payload(b"increment", data::inline_one(increase_with.to_string().into_bytes()).as_number())
            .with_named_payload(b"warning", data::inline_one(b"Large increment, consider smaller steps").as_string())
    } else {
        state.count = state.count + increase_with;
        tagged_output::new(b"ok")
            .with_named_payload(b"old_count", data::inline_one(old_count.to_string().into_bytes()).as_number())
            .with_named_payload(b"new_count", data::inline_one(state.count.to_string().into_bytes()).as_number())
            .with_named_payload(b"increment", data::inline_one(increase_with.to_string().into_bytes()).as_number())
    }
}
```

A few notes on the signature:

- `_authorization: ProvenValue<AgentVertexAuthorization>` and
  `worksheet: &mut ProofOfUID` are the framework-supplied parameters — the
  workflow passes them automatically; you don't construct them.
- Place your own input ports (here `increase_with: u64`) between the framework
  parameters and the trailing `ctx: &mut TxContext`. If your logic needs the
  time, add a `clock: &Clock` parameter and import `sui::clock::Clock`.
- Stamping the worksheet is mandatory. Skipping
  `worksheet.stamp_with_data(...)` means the framework cannot verify execution
  and the walk fails.

### Field Value Types

`with_named_payload` needs a type hint so Nexus formats the JSON correctly. Use
the matching constructor:

```move
// Numeric values (u8..u256)
.with_named_payload(b"count", data::inline_one(value.to_string().into_bytes()).as_number())

// String values (quoted in JSON)
.with_named_payload(b"message", data::inline_one(b"Hello world").as_string())

// Boolean values (true/false, unquoted)
.with_named_payload(b"success", data::inline_one(b"true").as_bool())

// Address values (0x-prefixed, quoted)
.with_named_payload(b"sender", data::inline_one(addr.to_string().into_bytes()).as_address())

// Raw JSON values (objects, arrays, null — passed through as-is)
.with_named_payload(b"metadata", data::inline_one(b"{\"key\":\"value\"}").as_raw())

// Many values (for loops in Nexus)
.with_named_payload(b"items", data::inline_many(items).as_number())
```

## 6. Helpers and Test Scaffolding

Add the witness accessor, a public getter that exposes the tool witness id
(useful when registering), and a test-only initializer:

```move
/// Borrow the witness object stored in the state bag.
fun witness(self: &CounterState): &CounterWitness {
    self.witness.borrow(b"witness")
}

/// Get the tool witness id, used during onchain registration.
public fun tool_witness_id(self: &CounterState): ID {
    object::uid_to_inner(&self.witness().id)
}

/// Read the current count.
public fun count(self: &CounterState): u64 {
    self.count
}

#[test_only]
public fun init_for_test(otw: COUNTER_TOOL, ctx: &mut TxContext) {
    init(otw, ctx);
}
```

Add tests under the package's `/tests` folder that drive `execute` through each
output variant (zero, normal, and large increments) and assert the resulting
count.

## 7. Publish to Sui

Publish the package to your target network:

```bash
sui client publish

# Save the package id from the output.
export PACKAGE_ID="0x..."
```

From the publish output, note two things:

1. **Package id** (`PACKAGE_ID` above) — the address of your published package.
1. **Shared state object id** — the `CounterState` object created by `init`.

You then need the **tool witness id**: the id of the `CounterWitness` object
stored inside the state's `witness` bag. This is the inert *stamp locator* used
during registration — it is **not** the same as the shared state object id (the
state object is what the workflow passes into `execute`).

Find the tool witness id either by inspecting the shared state object and reading
its `witness` dynamic field, or by calling the `tool_witness_id` getter:

```bash
sui client object <COUNTER_STATE_ID>
```

## 8. Register the Tool

Register the tool with the Nexus CLI. The CLI analyzes your `execute` function
and `Output` enum to generate the input and output schemas automatically:

```bash
nexus tool register onchain \
  --package "$PACKAGE_ID" \
  --module counter_tool \
  --tool-fqn "xyz.mydomain.counter_tool@1" \
  --description "Increments an on-chain counter" \
  --tool-witness-id "0x..." \
  --timeout 5s
```

What each flag does:

- `--package` / `--module` — the published package address and the Move module
  name that contains `execute`.
- `--tool-fqn` — the fully qualified name (`domain.name@version`) for this tool.
- `--description` — a human-readable description.
- `--tool-witness-id` — the `CounterWitness` object id from the previous step.
- `--timeout` — the execution timeout (defaults to `5s`; must be between `1s`
  and `2m`).

Two more flags are available when you need them:

- `--collateral-coin <OBJECT_ID>` — the coin to use as collateral (the second
  gas coin is chosen automatically if omitted).
- `--workflow-authorization-cap-first` — use the cap-gated
  `register_on_chain_tool_with_workflow_authorization_cap` entrypoint.

Confirm the registration:

```bash
nexus tool list
nexus tool inspect --tool-fqn "xyz.mydomain.counter_tool@1"
```

## 9. Use the Tool in a Workflow

Once registered, reference the tool from a DAG by its FQN with the `on_chain`
variant, the same way you would an offchain tool:

```json
{
  "vertices": [
    {
      "kind": {
        "variant": "on_chain",
        "tool_fqn": "xyz.mydomain.counter_tool@1"
      },
      "name": "increment",
      "entry_ports": [
        {
          "name": "increase_with"
        }
      ]
    }
  ]
}
```

The `increment` vertex's `ok` variant exposes the `old_count`, `new_count`, and
`increment` output ports, which you can wire into downstream tools via edges.

## Next Steps

- Read the [DAG Construction Guide](dag-construction.md) to compose this tool
  with others into a full workflow.
- Study the example onchain tool module under
  [nexus-next/sui/examples](../../nexus-next/sui/examples/) and the
  corresponding JSON DAGs under
  [sdk/src/dag/_dags](../../sdk/src/dag/_dags/).
- Build an offchain tool that wraps an external API with the
  [Offchain Tool Development Guide](offchain-tool-development.md).
