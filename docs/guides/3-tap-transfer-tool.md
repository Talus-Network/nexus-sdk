# Write the on-chain transfer tool

The previous page set up `TutorialState` and the `take_treasury` helper. This page adds the actual on-chain Move tool — a sibling module named `transfer_vertex` whose `execute` function is what the workflow invokes per skill execution.

The general mechanics of on-chain Move tools (`TaggedOutput`, witness types, the registration round-trip) are covered in detail in the [Onchain Tool Development guide](onchain-tool-development.md). This page only walks through what's _new_ for our TAP-bound transfer tool.

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

public fun fqn(): AsciiString {
    tutorial_transfer::transfer_vertex_fqn()
}

/// On-chain transfer.
///
/// Inputs (the workflow passes these via the DAG entry ports):
///   worksheet   — workflow-supplied `ProofOfUID` for stamping.
///   port 0      — `state: &mut TutorialState` (shared)
///   port 1      — `recipient: address`
///
/// The body drains the treasury, fires `public_transfer`, stamps the
/// worksheet, and returns a tagged output the workflow records on chain.
public fun execute(
    worksheet: &mut ProofOfUID,
    state: &mut TutorialState,
    recipient: address,
): TaggedOutput {
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

- **`worksheet: &mut ProofOfUID`** — supplied by the workflow. Every on-chain vertex tool takes it. Calling `worksheet.stamp_with_data(witness_uid, b"…")` proves to the workflow that _this_ registered tool ran on _this_ walk. Skip the stamp and the workflow rejects the walk.
- **`take_treasury` + `public_transfer`** — the actual SUI move. Once the walk succeeds the recipient owns those funds outright.
- **`TaggedOutput::Transferred { amount, recipient }`** — the structured output the workflow records on chain. Downstream consumers can read it back via `nexus dag inspect-execution`.

## 4. Re-validate locally

Re-run the validator to confirm both modules still compile:

```bash
nexus tap validate-skill --config skill.tap.json
```

You should still see `[✓] Validating TAP skill config...` — but now both modules are present and the package gates withdrawal correctly.

Next: [DAG and skill config](4-tap-dag-and-skill-config.md).
