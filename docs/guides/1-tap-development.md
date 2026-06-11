# TAP Development

This series teaches how to build, register, and operate a **standard TAP skill** end-to-end. Each page is short and self-contained; together they take you from an empty directory to a working agent that calls an on-chain Move tool which transfers SUI to a recipient address.

> **Prereqs.** You should already be comfortable with the [Setup guide](setup.md) (CLI install, Sui wallet, `nexus conf set`) and have read the [Onchain Tool Development guide](onchain-tool-development.md) for Move tool fundamentals (witness types, `TaggedOutput`, registration mechanics).

## What a TAP skill is

A standard Talus Agent Protocol (TAP) **skill** wraps up to three things behind one registry identity:

1. A **TAP Move package** — your custom Move code: shared state objects, the witness type that ties a vertex tool to your package, and any business-logic helpers the tool needs (e.g. coin custody). At the protocol level this is optional: `register_skill` itself doesn't take a package id, so a skill whose DAG uses only off-chain HTTP tools and no on-chain state doesn't need one. This tutorial's skill uses an on-chain transfer tool, so the package contents below are required.
1. A **DAG** — the workflow definition the leader executes when the skill runs. For this tutorial the DAG has a single vertex that calls one on-chain Move tool.
1. A **skill config** (`skill.tap.json`) — declares the DAG, the TAP package path, and the skill's input, payment, schedule, and fixed-tool requirements.

A skill lives under an **agent** (also on-chain). Mutable custody of the `Agent` object is the lifecycle authorization handle, and the registry stores the agent's active flag plus skill records. Each skill record carries the DAG binding, simplified requirements, and a `current_interface_revision` that fresh executions and scheduled-task creation use. `nexus tap bind` / `nexus tap register-skill` create revision `1` with the skill record, and `nexus tap update-skill` moves the current skill contract to a new revision for future starts.

The current skill contract has these important parts:

- **DAG binding** points to the published DAG that workflow execution should run. The concrete shared objects a skill needs, such as this tutorial's `TutorialState`, are supplied as execution inputs rather than stored in a separate endpoint-revision table.
- **`requirements.input_schema_commitment`** is an opaque byte vector that identifies the expected input shape for tooling and dry-run checks.
- **`requirements.payment_policy`** is either `UserFunded` or `AgentFunded { max_budget }`; user-funded execution supplies the payment coin at call time, while agent-funded execution draws from the agent's payment vault.
- **`requirements.schedule_policy`** declares whether scheduled execution is one-shot or recursive, including `allow_recursive` and recurrence bounds.
- **`requirements.fixed_tools`** is the canonical set of registry-verified tools that must remain present in the bound DAG. It is a preservation requirement, not the old authorized-tool or vertex-authorization schema.

## What we'll build

The tutorial's skill exposes a single vertex tool that does one job: **drain a SUI treasury sitting in the TAP package's shared state into a recipient address** passed as a workflow input. The state is funded out-of-band (we'll add a `fund_treasury` helper), and each skill execution moves the treasury balance to the recipient. The workflow dispatches the walk, the leader runs the Move tool, the recipient receives SUI.

{% hint style="warning" %}
**This tutorial is intentionally minimal.** The on-chain transfer tool is registered through the plain `register_on_chain_tool` entry point and the skill config carries an empty `fixed_tools` list, so any workflow execution against this skill can drain the treasury — there is no per-call authorization check. The end of the last page covers what that means in practice and points at the follow-up guide for cap-gated authorization (`VertexAuthorizationCheckCap`, `WorkflowVertexAuthorizationGrant`, and `fixed_tools`), which is the production-ready way to wrap the same transfer logic.
{% endhint %}

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
- Address-funded scheduling and the default-agent variant (`nexus tap schedule-address-funded`, `nexus tap schedule-default-address-funded`).
- Current skill updates for already-bound skills (`nexus tap update-skill`).
- Inspecting payment receipts and execution costs (`nexus tap payments list`, `nexus dag execution-cost`).
