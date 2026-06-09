# Scaffold the TAP package

This page generates the on-disk shape of a TAP skill and replaces the scaffolded Move stub with a real state module that holds a SUI treasury and the per-vertex witness object the on-chain tool will be registered against.

## 1. Generate the scaffold

Pick a working directory and run:

```bash
nexus tap scaffold --name "tutorial transfer" --target .
cd tutorial-transfer
```

The scaffold writes four files under `tutorial-transfer/`:

```text
tutorial-transfer/
├── dag.json                      # workflow definition (we'll edit later)
├── skill.tap.json                # skill config (we'll edit later)
└── tap/
    ├── Move.toml                 # the TAP package manifest
    └── sources/
        └── tutorial_transfer.move
```

Each generated file is a stub. The scaffolded `tutorial_transfer.move` is a single drop witness with an `init_for_test` helper — enough to compile, but nothing the workflow can actually call.

## 2. Trim the scaffolded `tap/Move.toml` and stage `nexus_primitives`

The scaffold ships with four `[dependencies]` entries (`nexus_primitives`, `nexus_interface`, `nexus_registry`, `nexus_workflow`) so authors who reach for the full standard-TAP surface don't have to add deps mid-build. The minimal vertex tool we're about to write only touches `nexus_primitives` — `data`, `proof_of_uid`, and `tagged_output`. Drop the other three entries so the build resolves the smallest possible dep tree:

```toml
[dependencies]
nexus_primitives = { local = "deps/primitives" }
```

Then stage the `nexus_primitives` package under `tap/deps/primitives/`. It lives in `nexus-next/sui/primitives/`; either copy the directory in or point the `local` path at wherever your installation keeps it. The staged dependency needs its own `Move.toml` with an `[environments]` table and a `Published.toml` carrying the deployed package address.

The scaffold writes an `[environments]` table pre-filled with the public-testnet chain id, which is what this guide targets:

```toml
[environments]
testnet = "4c78adac"
```

Leave that as-is unless you're publishing to a different network. If you are, replace the row with `<env_alias> = "<chain-id>"` for your target network (the alias must match a name in `sui client envs`, and the chain id is what `sui client chain-identifier` prints while that env is active).

The end result looks like:

```toml
[package]
name = "tutorial_transfer"
version = "1.0.0"
edition = "2024"

[dependencies]
nexus_primitives = { local = "deps/primitives" }

[environments]
testnet = "4c78adac"
```

{% hint style="info" %}
`nexus tap validate-skill` enforces the new-style 2024 layout: the manifest must have `[package].version`, `edition = "2024"` (no `.beta`), an `[environments]` table, and **no** `[addresses]` section. Old-style packages can't resolve their dependency graph against the new-style published Nexus packages, so the validator rejects them up front with a pointer at the field that needs fixing.
{% endhint %}

## 3. Replace the scaffold's Move source

The interesting work is in `tap/sources/tutorial_transfer.move`. Replace its contents with the module below. The state object holds a SUI treasury that the vertex tool will drain on each execution, plus the per-vertex witness whose UID feeds `nexus tool register onchain --tool-witness-id` and the worksheet stamp at runtime.

```move
module tutorial_transfer::tutorial_transfer;

use std::ascii::String as AsciiString;
use sui::coin::{Self, Coin};
use sui::sui::SUI;
use sui::transfer::public_share_object;

/// One-time witness — guarantees `init` runs exactly once at publish.
public struct TUTORIAL_TRANSFER has drop {}

/// Per-vertex witness. Its UID becomes the on-chain tool's `tool_witness_id`
/// (passed to `nexus tool register onchain`) and the seed the vertex stamps
/// onto the workflow worksheet at runtime.
public struct TransferVertexWitness has key, store {
    id: UID,
}

/// Shared state for the tutorial skill. Holds the SUI treasury that the
/// transfer vertex drains on each execution plus the per-vertex witness
/// the on-chain tool registers against.
public struct TutorialState has key, store {
    id: UID,
    transfer_vertex_witness: TransferVertexWitness,
    treasury: option::Option<Coin<SUI>>,
}

fun init(_otw: TUTORIAL_TRANSFER, ctx: &mut TxContext) {
    public_share_object(TutorialState {
        id: object::new(ctx),
        transfer_vertex_witness: TransferVertexWitness { id: object::new(ctx) },
        treasury: option::none(),
    });
}

/// Canonical FQN for the on-chain transfer vertex tool. The DAG references
/// this FQN, the tool registration uses this FQN, and the vertex module
/// returns this FQN from its own `fqn()` helper.
public fun transfer_vertex_fqn(): AsciiString {
    b"tutorial.local.transfer_vertex@1".to_ascii_string()
}

/// The id passed to `nexus tool register onchain --tool-witness-id`.
public fun transfer_vertex_tool_witness_id(state: &TutorialState): ID {
    object::uid_to_inner(&state.transfer_vertex_witness.id)
}

/// The witness UID the vertex stamps onto its worksheet at runtime.
public(package) fun transfer_vertex_witness_uid(state: &TutorialState): &UID {
    &state.transfer_vertex_witness.id
}

/// Top up the treasury that future skill executions will drain.
public fun fund_treasury(state: &mut TutorialState, coin: Coin<SUI>) {
    if (state.treasury.is_some()) {
        let mut existing = option::extract(&mut state.treasury);
        coin::join(&mut existing, coin);
        option::fill(&mut state.treasury, existing);
    } else {
        option::fill(&mut state.treasury, coin);
    }
}

/// Drain the treasury. `public(package)` keeps the function callable only
/// from sibling modules in `tutorial_transfer` (i.e. `transfer_vertex`), so
/// nothing outside this package can pull funds out directly.
public(package) fun take_treasury(state: &mut TutorialState): Coin<SUI> {
    option::extract(&mut state.treasury)
}
```

Key things to notice:

- `TutorialState` is shared (`public_share_object`) so the workflow and the treasury funder can both reach it.
- `TransferVertexWitness` is stored inside `TutorialState` so its `UID` is stable for the package's lifetime — the on-chain tool registers against it once.
- `take_treasury` is `public(package)`. The sibling `transfer_vertex` module (next page) is the only caller; nothing outside the `tutorial_transfer` package can extract the coin directly.

## 4. Validate locally

Run the local validator before going anywhere near the chain:

```bash
nexus tap validate-skill --config skill.tap.json
```

The validator resolves `tap_package_path` and `dag_path` relative to `--config`, so you do not need to point at the Move package separately. You should see:

```text
[✓] Validating TAP skill config...
```

If the Move package fails to compile, the error surfaces here — fix the source and re-run. `validate-skill` doesn't touch the network, so iterations are fast.

## What you have now

- A `tap/Move.toml` and `tap/sources/tutorial_transfer.move` that compile against the published Nexus deps for your network.
- A `TutorialState` shared object with a SUI-coin treasury, a witness object, and a `fund_treasury` entry function.
- A `skill.tap.json` and `dag.json` still in their scaffold-default shapes — we'll edit those two pages from now.

Next: [Write the on-chain transfer tool](3-tap-transfer-tool.md).
