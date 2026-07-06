# Policy DFA

This guide is for Nexus readers who need to understand the reusable onchain policy primitive before reading scheduler code or building a package that gates actions by an ordered trace. It introduces `nexus_primitives::policy`, the `nexus_primitives::automaton` DFA it wraps, and the behavior a caller can rely on when advancing policy state with witness types or object IDs.

## Why Nexus uses a DFA-backed policy

A policy describes which sequence of typed events is allowed. In Nexus, those events are `Symbol` values: either `Witness(TypeName)` for a Move witness type or `Uid(object::ID)` for a concrete Sui object. That alphabet lets a policy describe paths across packages without adding direct dependencies between every package that participates in the flow.

Move does not provide a runtime regex engine. `automaton.move` gives Nexus the programmable form of a regular language: a deterministic finite automaton with a finite state set, finite alphabet, start state, accepting states, and a transition table. Determinism matters because a caller with the current state and next symbol has exactly one next state. There is no ambiguous branch for an untrusted executor to reinterpret.

Keeping the policy onchain also keeps the allowed trace beside the guarded asset or process. Offchain code can drive the flow by submitting symbols, but the policy object decides whether the trace remains in the accepted language.

## How the automaton stores the accepted language

`nexus_primitives::automaton::DeterministicAutomaton<State, Symbol>` stores states, alphabet symbols, a dense transition matrix, an accepting bitmap, and a start index in `TableVec` storage. Construction checks that state and alphabet entries are unique, that the transition table has exactly one successor for every `(state, symbol)` pair, that every successor exists in the state basis, and that the start and accepting states exist.

Evaluation is constant-shape: `delta_indexed` reads the transition row for the current state index and the column for the symbol index. `run`, `run_from`, and `accepts` fold that same transition logic over a word. If a state or symbol is outside the basis, helper functions abort instead of silently treating the input as allowed.

`ConfiguredAutomaton` wraps a DFA with a UID, so callers can attach transition metadata as dynamic fields. Metadata can be registered for a specific `(state, symbol)` transition or as a wildcard `(*, symbol)`. Lookup checks the specific transition first and falls back to the wildcard entry. Missing metadata aborts when borrowed.

## How the policy wrapper adds typed symbols

`nexus_primitives::policy::Policy<T>` fixes the automaton state type to `u64` and the symbol type to `Policy::Symbol`, then stores caller-defined payload data of type `T`. It exposes the current state index, the current state value, the underlying DFA, transition metadata registration, accepting-state checks, and reset.

The main advancement functions are:

```move
// Advance the policy with a Move witness type; the witness value is created by the package step being recorded.
policy::advance_with_witness<W, T>(&mut policy, witness);
// Advance the policy with a concrete Sui object ID; get `uid` from `object::id`, `object::uid_to_inner`, or a previously stored object ID.
policy::advance_with_uid<T>(&mut policy, uid);
```

Each function resolves the typed symbol, checks that the symbol is in the alphabet, then applies the DFA transition to update `state_index`. `is_accepting` tells callers whether the current state is accepting. `reset` returns the policy to the start state while keeping its configured automaton and payload.

`new_linear` builds a prefix-enforcing DFA from a sequence of symbols. The generated states are `0..sequence_length`, and the accepting state is the final index. The expected symbol at each step advances to the next state. Other in-alphabet symbols self-loop at the current state. Symbols outside the alphabet abort. If a package needs hard failure for a symbol that is otherwise in the alphabet, it should configure an explicit non-accepting sink state instead of relying on `new_linear` self-loops.

## Concrete example: ordered approval trace

The smallest useful policy is an ordered trace. Imagine a package that lets an asset leave escrow only after the same request has been drafted, reviewed, and approved. The package can represent each step with a zero-sized witness type and build a `new_linear` policy over those witness symbols. The example below is written as a test helper so it can clean up the key `Policy` object with `policy::destroy_policy`; production code would store the policy inside the guarded object or share the policy object instead.

```move
// Import the live policy module whose `new_linear`, `advance_with_witness`, `state`, `is_accepting`, and test cleanup helpers are defined in ../../sui/primitives/sources/policy.move.
use nexus_primitives::policy;
// Import Sui `TableVec` construction because `policy::new_linear` expects an ordered `TableVec<policy::Symbol>` sequence, matching the signature in ../../sui/primitives/sources/policy.move.
use sui::table_vec;
// Define the first witness token; the `drop` ability is required because `policy::advance_with_witness` consumes the witness value after resolving its type name.
public struct Drafted has drop {}
// Define the second witness token; callers can create `Reviewed {}` only from code that has access to this type, so the package controls who can record review.
public struct Reviewed has drop {}
// Define the final witness token; reaching this symbol moves the linear policy into its accepting state.
public struct Approved has drop {}
// Define the payload stored inside `Policy<T>`; `store` is required by `public struct Policy<T: store>` in ../../sui/primitives/sources/policy.move.
public struct ApprovalPolicyData has copy, drop, store { request_id: u64 }
// Mark the helper test-only because it uses `policy::destroy_policy`, which exists only for tests and avoids leaving a key object unconsumed.
#[test_only]
// Accept `TxContext` because both `table_vec::empty` and `policy::new_linear` allocate Sui objects or table-backed storage through the transaction context.
fun approval_policy_example(ctx: &mut TxContext) {
    // Create the ordered sequence container; this is the exact collection type consumed by `policy::new_linear`.
    let mut sequence = table_vec::empty<policy::Symbol>(ctx);
    // Add the required first step, so state `0` advances to state `1` only when `Drafted {}` is recorded.
    sequence.push_back(policy::witness_symbol<Drafted>());
    // Add the required second step, so state `1` advances to state `2` only when `Reviewed {}` is recorded.
    sequence.push_back(policy::witness_symbol<Reviewed>());
    // Add the required final step, so state `2` advances to accepting state `3` only when `Approved {}` is recorded.
    sequence.push_back(policy::witness_symbol<Approved>());
    // Store caller-owned context next to the DFA; real packages use this payload for guarded-object IDs, revision numbers, or policy-local counters.
    let payload = ApprovalPolicyData { request_id: 42 };
    // Build the policy; `new_linear` creates states `0`, `1`, `2`, `3`, makes `3` accepting, and self-loops on other in-alphabet symbols.
    let mut approval = policy::new_linear(&sequence, payload, ctx);
    // The sequence table was only needed for construction, so the test helper drops it after `new_linear` has copied the symbols into the DFA alphabet.
    sequence.drop();
    // Confirm the initial cursor is state `0`, which is the start state produced by `new_linear`.
    assert!(policy::state(&approval) == 0, 0);
    // Confirm the policy is not yet accepting because no required witness has been recorded.
    assert!(!policy::is_accepting(&approval), 1);
    // Record the draft step; `advance_with_witness` resolves the `Drafted` type into `Symbol::Witness` and applies the DFA transition.
    policy::advance_with_witness<Drafted, ApprovalPolicyData>(&mut approval, Drafted {});
    // Confirm the cursor advanced from state `0` to state `1` after the first expected witness.
    assert!(policy::state(&approval) == 1, 2);
    // Record the review step; this is the only expected symbol that advances from state `1` in the linear policy.
    policy::advance_with_witness<Reviewed, ApprovalPolicyData>(&mut approval, Reviewed {});
    // Confirm the cursor advanced from state `1` to state `2` after review.
    assert!(policy::state(&approval) == 2, 3);
    // Record the approval step; this final expected witness moves the cursor to the accepting state.
    policy::advance_with_witness<Approved, ApprovalPolicyData>(&mut approval, Approved {});
    // Confirm the cursor reached state `3`, the final state for a three-symbol `new_linear` policy.
    assert!(policy::state(&approval) == 3, 4);
    // Confirm the guarded action, such as releasing escrow or finishing a task, can now require `is_accepting`.
    assert!(policy::is_accepting(&approval), 5);
    // Destroy the policy in the test helper; production code would keep the policy in the guarded object or share it for later advancement.
    policy::destroy_policy(approval);
    // End the test helper after every owned value has either been dropped or consumed.
}
```

The resulting state trace is concrete:

| Input | State before | State after | Accepting after input |
| --- | --- | --- | --- |
| none | start | `0` | no |
| `Drafted {}` | `0` | `1` | no |
| `Reviewed {}` | `1` | `2` | no |
| `Approved {}` | `2` | `3` | yes |

If the caller submits `Reviewed {}` while still at state `0`, `new_linear` treats it as an in-alphabet but out-of-position symbol and keeps the policy at state `0`. If the caller submits a witness type that was not included in `sequence`, `advance_with_witness` aborts with the policy module's symbol-not-in-alphabet error. That distinction is why `new_linear` is useful for “wait until the right ordered trace appears” flows, while stricter protocols should define a custom DFA with an explicit rejecting sink state.

## How scheduler uses policy

The scheduler creates two policy planes for every `Task`. The constraints' policy tracks whether the selected occurrence generator has been consumed. The execution policy tracks whether scheduled payment has been converted into the matching workflow execution path.

Queue and periodic generators are registered as witness symbols on the constraints' policy. When `check_queue_occurrence` or `check_periodic_occurrence` consumes an active occurrence, scheduler advances the constraints' policy with `QueueGeneratorWitness` or `PeriodicGeneratorWitness`. `prepare_execution_from_scheduled_payment` then advances the execution policy with either the default-agent scheduled execution witness or the registered-agent scheduled execution witness. `finish` closes the run only when both policies are accepting.

This pattern lets scheduler keep timing logic pluggable without making the task lifecycle trust offchain code. A leader can call the public check and prepare functions, but the task can finish only when the onchain policies record the required sequence.

## How to design a policy-backed flow

1. Define the alphabet as witness types or concrete object IDs that represent the steps you want to allow.
1. Build the automaton with the state set, start state, accepting states, and transition table that encode the allowed traces.
1. Store any per-symbol or per-transition configuration that the guarded flow needs.
1. Call `advance_with_witness` or `advance_with_uid` before the side effect that depends on the trace.
1. Gate final payout, completion, reserve collection, or state transition with `is_accepting`.
1. Reset only when the guarded process intentionally starts a new run.

For a linear approval flow, the policy alphabet might contain `Witness(Draft)`, `Witness(Reviewed)`, and `Witness(Approved)`. The accepting state is reached only after those symbols arrive in order. For object routing, the alphabet can contain `Uid(A)`, `Uid(B)`, and `Uid(C)` so a package can enforce that specific objects were visited in a required sequence.
