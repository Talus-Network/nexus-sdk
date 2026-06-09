# TAP Development

This series teaches how to build, register, and operate a **standard TAP skill** end-to-end. Each page is short and self-contained; together they take you from an empty directory to a working agent that calls an on-chain Move tool which transfers SUI to a recipient address.

> **Prereqs.** You should already be comfortable with the [Setup guide](setup.md) (CLI install, Sui wallet, `nexus conf set`) and have read the [Onchain Tool Development guide](onchain-tool-development.md) for Move tool fundamentals (witness types, `TaggedOutput`, registration mechanics).

## What a TAP skill is

A standard Talus Agent Protocol (TAP) **skill** wraps up to three things behind one on-chain identity:

1. A **TAP Move package** — your custom Move code: shared state objects, the witness type that ties a vertex tool to your package, and any business-logic helpers the tool needs (e.g. coin custody). At the protocol level this is optional: `register_skill` itself doesn't take a package id, so a skill whose DAG uses only off-chain HTTP tools and no on-chain state doesn't need one. This tutorial's skill uses an on-chain transfer tool with cap-gated authorization, so the package contents below are required.
1. A **DAG** — the workflow definition the leader executes when the skill runs. For this tutorial the DAG has a single vertex that calls one on-chain Move tool.
1. A **skill config** (`skill.tap.json`) — declares the DAG, the TAP package path, the skill's payment/schedule/authorization requirements, and the shared objects the workflow needs to touch.

A skill lives under an **agent** (also on-chain). The agent record carries the operator address that's allowed to drive executions; the skill record carries the DAG + requirements. Each `(agent, skill)` pair has one or more **endpoint revisions** — a version-pinned bundle of `(shared_objects, requirements, config_digest)` that the workflow reads at runtime. `nexus tap bind` / `nexus tap register-skill` always create `interface_revision(1)` atomically with the skill record, and subsequent revisions are appended via `nexus tap announce`; the registry's endpoints table is append-only, so revisions never drop to zero.

Three things sit inside that bundle:

- **`shared_objects`** is the list of *skill-author-owned* shared Move objects that the skill's vertex tools will read or write at execution time, each tagged with a mutability bit (`{ id, mutable }`). It does **not** include workflow framework objects like `AgentRegistry`, `ToolRegistry`, `Clock`, the `Agent`, the `DAG`, or `ToolGas` cells — those are wired into every execute PTB automatically by the SDK. It's advisory metadata: it's committed into `config_digest` so the advertisement can't be swapped after announcement, but the PTB builder still has to fetch and pass these object refs explicitly per execution. An empty list is valid for a skill with no custom on-chain state. For this tutorial, the only entry will be the `TutorialState` shared object that holds the treasury `Balance<SUI>` and the grant-id binding, declared mutable because the transfer tool drains it.
- **`requirements`** carries the four commitments (input schema, workflow, metadata, capability schema) plus the payment policy, schedule policy, and vertex authorization schema (`fixed_tools` + `requires_payment`).
- **`config_digest`** is `sha2_256(BCS({ interface_revision, shared_objects, requirements }))`, checked by `assert_valid_config_digest` before any announcement is accepted.

## What we'll build

The tutorial's skill exposes a single vertex tool that does one job: **drain a SUI treasury sitting in the TAP package's shared state into a recipient address** passed as a workflow input. The state is funded out-of-band (we'll add a `fund_treasury` helper), and each skill execution moves the treasury balance to the recipient.

Authorization runs through the cap-gated TAP path: the skill registers its on-chain transfer tool with `--workflow-authorization-cap-first`, the skill config declares the tool in `fixed_tools` with `requires_payment: true`, and `nexus tap execute --grant-bind` mints a `WorkflowVertexAuthorizationGrant` inside the execute PTB and writes its id into `TutorialState`. The vertex tool's `execute` consumes the `VertexAuthorizationCheckCap` and `assert!`s that its `grant_id` matches the value just written — anyone else who tries to drain the treasury from their own `(agent, skill)` produces a cap whose grant id was never recorded in our state, so the assertion fails before any coin moves.

## End-to-end flow

```text
nexus tap scaffold        →  empty TAP package skeleton + skill.tap.json
        │
        ▼
edit Move source          →  add state object + on-chain transfer tool
        │
        ▼
nexus tap validate-skill  →  local-only checks (no chain)
        │
        ▼
nexus tap publish-skill   →  publishes Move package + DAG, writes artifact.json
        │
        ▼
nexus tool register       →  registers the on-chain transfer tool in ToolRegistry
        onchain
        │
        ▼
nexus tap bind            →  creates an agent + registers the skill atomically
        │
        ▼
fund the treasury         →  one-shot Move call that deposits SUI into state
        │
        ▼
nexus tap execute         →  submits the TAP execution (payment, DAG inputs)
        │
        ▼
verify recipient balance  →  the treasury arrived in the destination wallet
```

Each arrow is one `nexus` command and one short stop on the way. The next five pages walk through them.

## Pages

1. [Scaffold the TAP package](2-tap-scaffold-and-package.md) — `nexus tap scaffold`, plus the Move state module the scaffold doesn't write for you.
1. [Write the on-chain transfer tool](3-tap-transfer-tool.md) — the `transfer_vertex` Move module that the workflow invokes.
1. [DAG and skill config](4-tap-dag-and-skill-config.md) — wire the on-chain tool's FQN into `dag.json` and adjust `skill.tap.json`.
1. [Publish, register, bind](5-tap-publish-and-register.md) — `tap publish-skill`, `tool register onchain`, `tap bind`, and the on-chain confirmations.
1. [Execute and verify the transfer](6-tap-execute-and-settle.md) — fund the treasury, run `tap execute`, watch the recipient balance go up.

## What this guide does **not** cover

The TAP CLI surface is broader than what one tutorial can show. After you finish the series, the [CLI reference](../cli.md) covers:

- Vault funding and vault-funded scheduling (`nexus tap vault deposit`, `nexus tap schedule-from-vault`).
- Address-funded scheduling and the default-executor variant (`nexus tap schedule-address-funded`, `nexus tap schedule-default-address-funded`).
- Endpoint revision announcements for already-bound skills (`nexus tap announce`).
- Inspecting payment receipts and execution costs (`nexus tap payments list`, `nexus dag execution-cost`).
