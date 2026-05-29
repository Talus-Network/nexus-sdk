# Scaffold the TAP package

This page generates the on-disk shape of a TAP skill and replaces the scaffolded Move stub with a real state module that holds a SUI treasury **plus** a per-execution "pending grant" slot. The grant slot is what gives the production-correct treasury gate later: only the legitimate caller can fill it, and the vertex tool refuses to drain the treasury without it.

## 1. Generate the scaffold

Pick a working directory and run:

```bash
nexus tap scaffold --name "tutorial transfer" --target .
```

The command writes four files under `tutorial-transfer/`:

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

## 2. Point Move.toml at the published Nexus dependencies

The scaffold's `tap/Move.toml` declares relative-path dependencies on `nexus_primitives`, `nexus_interface`, `nexus_registry`, and `nexus_workflow` under `../../nexus/sui/`. Update those paths so they resolve to the published Sui Move sources for your target network (their published addresses are recorded in the same `objects.testnet.toml` you pointed `nexus conf set --nexus.objects` at).

A working `tap/Move.toml` on testnet (substitute your own paths and chain id):

```toml
[package]
name = "tutorial_transfer"
version = "1.0.0"
edition = "2024"

[dependencies]
nexus_primitives = { local = "deps/primitives" }
nexus_interface  = { local = "deps/interface" }
nexus_registry   = { local = "deps/registry" }
nexus_workflow   = { local = "deps/workflow" }

[addresses]
tutorial_transfer = "0x0"

[environments]
testnet = "<chain-id-from-sui-client-chain-identifier>"
```

Each staged dependency under `deps/<name>/` needs its own `Move.toml` with a `published-at` line carrying the deployed package address from `objects.testnet.toml` and an `[environments]` section matching the one above. `nexus_workflow`, in turn, depends on `nexus_registry`/`nexus_interface`/`nexus_primitives`, so wire all four.

## 3. Replace the scaffold's Move source

The interesting work is in `tap/sources/tutorial_transfer.move`. Replace its contents with the module below. Compared to a plain "treasury + transfer" example we add two pieces that cap-gated TAP execution requires:

- `authorized_grant_id: Option<address>` — a slot that the SDK fills with the freshly-minted `VertexAuthorizationGrant` id before each execution.
- `bind_pending_grant(state, vertex_bytes, grant_id)` — a public Move function with a SDK-fixed signature that `nexus tap execute --grant-bind` calls inside the execute PTB to lock the state to _this_ execution's grant.

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
/// transfer vertex drains, plus a per-execution grant id that the SDK
/// populates via `bind_pending_grant` before the leader dispatches the walk.
public struct TutorialState has key, store {
    id: UID,
    transfer_vertex_witness: TransferVertexWitness,
    treasury: option::Option<Coin<SUI>>,
    authorized_grant_id: option::Option<address>,
}

const EAlreadyBound: u64 = 0;

fun init(_otw: TUTORIAL_TRANSFER, ctx: &mut TxContext) {
    public_share_object(TutorialState {
        id: object::new(ctx),
        transfer_vertex_witness: TransferVertexWitness { id: object::new(ctx) },
        treasury: option::none(),
        authorized_grant_id: option::none(),
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

/// State-bind hook for `nexus tap execute --grant-bind`. The SDK calls this
/// function as part of the execute PTB *after* the workflow has minted the
/// vertex-authorization grant attached to the new `DAGExecution` and *before*
/// the leader dispatches the walk. The signature must be exactly
/// `(&mut TutorialState, vector<u8>, address)` — the SDK passes the vertex
/// name's bytes as the second argument and the freshly-minted grant id as
/// the third.
///
/// We `abort` if a grant is already pending so the same state can't be
/// re-bound by a different (agent, skill) racing for the treasury. The
/// vertex tool clears the slot at the end of every walk so repeat
/// invocations work.
public fun bind_pending_grant(
    state: &mut TutorialState,
    _vertex_name: vector<u8>,
    grant_id: address,
) {
    assert!(state.authorized_grant_id.is_none(), EAlreadyBound);
    option::fill(&mut state.authorized_grant_id, grant_id);
}

/// Consume the grant id previously bound by `bind_pending_grant`. Only the
/// sibling `transfer_vertex` module can call this — the assertion lives there.
public(package) fun extract_authorized_grant(state: &mut TutorialState): address {
    option::extract(&mut state.authorized_grant_id)
}

/// Drain the treasury. `public(package)` keeps the function callable only
/// from sibling modules in `tutorial_transfer` (i.e. `transfer_vertex`), so
/// nothing outside this package can pull funds out by side-stepping the
/// grant check.
public(package) fun take_treasury(state: &mut TutorialState): Coin<SUI> {
    option::extract(&mut state.treasury)
}
```

Key things to notice:

- `TutorialState` is shared (`public_share_object`) so the workflow, the treasury funder, and the `bind_pending_grant` caller can all reach it.
- `TransferVertexWitness` is stored inside `TutorialState` so its `UID` is stable for the package's lifetime — the on-chain tool registers against it once.
- `bind_pending_grant` is `public` because the SDK's execute PTB calls it via a Move call. The signature is fixed by the SDK convention: `(&mut TutorialState, vector<u8>, address)`.
- `take_treasury` and `extract_authorized_grant` are `public(package)`. The sibling `transfer_vertex` module (next page) is the only caller.

## 4. Validate locally

Run the local validator before going anywhere near the chain:

```bash
nexus tap validate-skill \
    --config tutorial-transfer/skill.tap.json \
    --tap-package tutorial-transfer/tap
```

You should see:

```text
[✓] Validating TAP skill config...
```

If the Move package fails to compile, the error surfaces here — fix the source and re-run. `validate-skill` doesn't touch the network, so iterations are fast.

## What you have now

- A `tap/Move.toml` and `tap/sources/tutorial_transfer.move` that compile against the published Nexus deps for your network.
- A `TutorialState` shared object with a SUI-coin treasury, a witness object, a `fund_treasury` entry function, and a `bind_pending_grant` hook that the SDK will fill before each execution.
- A `skill.tap.json` and `dag.json` still in their scaffold-default shapes — we'll edit those two pages from now.

Next: [Write the on-chain transfer tool](3-tap-transfer-tool.md).
