# Write the on-chain transfer tool

The previous page set up `TutorialState`, a `take_treasury` helper, and the `bind_pending_grant` hook that the SDK will call inside the execute PTB. This page adds the actual on-chain Move tool — a sibling module named `transfer_vertex` whose `execute` function is what the workflow invokes per skill execution.

The general mechanics of on-chain Move tools (`TaggedOutput`, witness types, the registration round-trip) are covered in detail in the [Onchain Tool Development guide](onchain-tool-development.md). This page only walks through what's _new_ for our cap-gated TAP-bound transfer tool.

## 1. Why a second Move module

The on-chain tool the workflow calls is identified by `(package_id, module_name, function_name)`. The `module_name` ends up as a registry key, so each on-chain tool gets its own Move module. Our package will end up with two modules:

```text
tutorial_transfer::tutorial_transfer    // state + witness + helpers
tutorial_transfer::transfer_vertex      // the on-chain vertex tool
```

`transfer_vertex` is what gets registered. The leader looks up `transfer_vertex::execute` by module + function name when it picks the walk up.

## 2. Add `tap/sources/transfer_vertex.move`

Drop the file alongside `tutorial_transfer.move`:

```move
module tutorial_transfer::transfer_vertex;

use nexus_primitives::data;
use nexus_primitives::proof_of_uid::ProofOfUID;
use nexus_primitives::tagged_output::{Self as tagged_output, TaggedOutput};
use nexus_workflow::dag::{Self as dag, VertexAuthorizationCheckCap};
use std::ascii::String as AsciiString;
use sui::transfer::public_transfer;
use tutorial_transfer::tutorial_transfer::{Self, TutorialState};

public struct TRANSFER_VERTEX has drop {}

public enum Output {
    Transferred {
        amount: u64,
        recipient: address,
    },
}

const EGrantMismatch: u64 = 0;

public fun fqn(): AsciiString {
    tutorial_transfer::transfer_vertex_fqn()
}

/// Cap-gated on-chain transfer.
///
/// Inputs (the workflow passes these via the DAG entry ports + per-walk minting):
///   cap         — `VertexAuthorizationCheckCap` minted by the workflow
///                 from the grant the SDK created inside the execute PTB.
///   worksheet   — workflow-supplied `ProofOfUID` for stamping.
///   port 0      — `state: &mut TutorialState` (shared)
///   port 1      — `recipient: address`
///
/// The body proves authorization, consumes the cap, drains the treasury,
/// fires `public_transfer`, stamps the worksheet, and returns a tagged
/// output the workflow records on chain.
public fun execute(
    cap: VertexAuthorizationCheckCap,
    worksheet: &mut ProofOfUID,
    state: &mut TutorialState,
    recipient: address,
): TaggedOutput {
    // `extract_authorized_grant` pulls the grant id the SDK locked into state
    // via `bind_pending_grant`. The assertion below ties that value to the
    // cap's grant id; mismatch (or a missing slot) aborts before any coin
    // moves.
    let expected = tutorial_transfer::extract_authorized_grant(state);
    assert!(
        dag::vertex_authorization_check_cap_grant_id(&cap) == expected,
        EGrantMismatch,
    );
    dag::consume_vertex_authorization_check_cap(cap);

    let coin = tutorial_transfer::take_treasury(state);
    let amount = coin.value();
    public_transfer(coin, recipient);
    worksheet.stamp_with_data(
        tutorial_transfer::transfer_vertex_witness_uid(state),
        b"tutorial_transfer_done",
    );

    tagged_output::new(b"transferred")
        .with_named_payload(
            b"amount",
            data::inline_one(amount.to_string().into_bytes()).as_number(),
        )
        .with_named_payload(
            b"recipient",
            data::inline_one(recipient.to_ascii_string().into_bytes()).as_address(),
        )
}
```

## 3. What each line is doing

- **`cap: VertexAuthorizationCheckCap`** — the workflow mints this from the grant the SDK created and attached to the new `DAGExecution`. The cap is a non-`store`, non-`drop` hot potato: the only way to dispose of it is by handing it to `dag::consume_vertex_authorization_check_cap`. Anyone trying to call `execute` outside a workflow dispatch can't construct one.
- **`extract_authorized_grant` + `assert!`** — the cap proves "the workflow authorized this walk." The assertion proves "the workflow authorized _this_ walk against _this_ state." Without it, a malicious skill author could register their own `(agent, skill)` listing our tool in `fixed_tools`, mint _their_ grant, and drain our treasury. The state-side slot is what closes the loop.
- **`consume_vertex_authorization_check_cap(cap)`** — destroys the cap so a single grant maps to a single walk.
- **`worksheet: &mut ProofOfUID`** — supplied by the workflow. Every on-chain vertex tool takes it. Calling `worksheet.stamp_with_data(witness_uid, b"…")` proves to the workflow that _this_ registered tool ran on _this_ walk. Skip the stamp and the workflow rejects the walk.
- **`take_treasury` + `public_transfer`** — the actual SUI move. Once the walk succeeds the recipient owns those funds outright.
- **`TaggedOutput::Transferred { amount, recipient }`** — the structured output the workflow records on chain. Downstream consumers can read it back via `nexus dag inspect-execution`.

## 4. Why this combination is the gate

Three layers stack to keep the treasury safe:

1. **`take_treasury` is `public(package)`** so the only Move code that can extract the coin lives inside `tutorial_transfer`.
1. **The vertex tool consumes a `VertexAuthorizationCheckCap`** that the workflow only mints from a `WorkflowVertexAuthorizationGrant` attached to the running `DAGExecution`. No grant, no cap, no call.
1. **The grant id stored in `state.authorized_grant_id` matches the cap's grant id** because `nexus tap execute --grant-bind` mints the grant and writes the id into state inside the same execute PTB. A walk started by anyone else — different agent, different skill, different prepare flow — produces a cap whose grant id was never written into our state, so the assertion fails and the walk aborts before any coin moves.

If you skipped page 2's `bind_pending_grant` setter or this page's `extract_authorized_grant + assert!`, you would still have a cap-gated _tool_, but the treasury would still be reachable from anyone's DAG. The cap alone is not enough — the state-side grant id is what makes the authorization specific to _this_ skill's invocation.

## 5. Re-validate locally

Re-run the validator to confirm both modules still compile:

```bash
nexus tap validate-skill \
    --config tutorial-transfer/skill.tap.json \
    --tap-package tutorial-transfer/tap
```

You should still see `[✓] Validating TAP skill config...` — but now both modules are present and the package gates withdrawal correctly.

Next: [DAG and skill config](4-tap-dag-and-skill-config.md).
