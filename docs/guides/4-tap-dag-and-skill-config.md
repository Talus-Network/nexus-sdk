# DAG and skill config

The Move package now compiles and exposes `transfer_vertex::execute`. Before we publish, we need to teach the workflow about it. That happens in two files:

- `dag.json` — the workflow definition. We tell it about the on-chain vertex, its FQN, and which entry ports we'll feed at execute time.
- `skill.tap.json` — the standard TAP skill manifest. We point it at the DAG and the TAP package and keep the rest of the requirements at scaffold defaults.

Both files were created by `nexus tap scaffold` with off-chain-tool defaults. We're going to overwrite them.

## 1. Rewrite `dag.json`

The scaffold writes a one-vertex DAG referencing a placeholder off-chain weather tool. Replace it with our on-chain transfer vertex:

```json
{
  "vertices": [
    {
      "kind": {
        "variant": "on_chain",
        "tool_fqn": "tutorial.local.transfer_vertex@1"
      },
      "name": "transfer_vertex",
      "entry_ports": [{ "name": "0" }, { "name": "1" }]
    }
  ],
  "edges": []
}
```

What's going on:

- **`variant: "on_chain"`** flips the vertex kind. The workflow runtime will expect an on-chain Move tool registered under the FQN below.
- **`tool_fqn: "tutorial.local.transfer_vertex@1"`** is the FQN you set in the Move source and that the registration on the next page will use. The Move source, the DAG, and the on-chain tool registration must all agree on this string.
- **`name: "transfer_vertex"`** is the vertex name inside the DAG. It's what we type when we feed inputs at execute time (`--input-json '{"transfer_vertex": {...}}'`).
- **`entry_ports`** lists the inputs the workflow will collect from the invoker. Port `"0"` is the `state` object and port `"1"` is the recipient address. The workflow prepends `worksheet` to the call automatically, so the Move `execute(worksheet, state, recipient)` lines up.

You're not declaring types here; the leader reads the registered tool's input schema at runtime and validates the entry-port JSON against it.

## 2. Rewrite `skill.tap.json`

```json
{
  "name": "tutorial transfer",
  "tap_package_name": "tutorial_transfer",
  "dag_path": "dag.json",
  "tap_package_path": "tap",
  "requirements": {
    "input_schema_commitment": [
      116, 117, 116, 111, 114, 105, 97, 108, 45, 105, 110, 112, 117, 116
    ],
    "workflow_commitment": [
      116, 117, 116, 111, 114, 105, 97, 108, 45, 119, 111, 114, 107, 102, 108,
      111, 119
    ],
    "metadata_commitment": [116, 117, 116, 111, 114, 105, 97, 108],
    "payment_policy": {
      "mode": "user_funded",
      "max_budget": 0,
      "token_type_commitment": [],
      "refund_mode": 0
    },
    "schedule_policy": {
      "recurrence_kind": "once",
      "min_interval_ms": 0,
      "max_occurrences": 1,
      "allow_recursive": false
    },
    "vertex_authorization_schema": {
      "schema_commitment": [],
      "fixed_tools": [],
      "requires_payment": false
    }
  },
  "shared_objects": [],
  "interface_revision": 1
}
```

Field by field:

- **`tap_package_name`** must match the Move package name (`tutorial_transfer`) — `publish-skill` uses it as the named-address override when it compiles the package.
- **`dag_path` / `tap_package_path`** are relative to this file. The scaffold wires both for you and we keep them as-is.
- **`requirements.input_schema_commitment` / `workflow_commitment` / `metadata_commitment`** are opaque byte vectors that endpoint announcements commit to. The JSON above encodes the ASCII strings `tutorial-input`, `tutorial-workflow`, and `tutorial`. Anything reproducible works; downstream tooling treats these as identifiers.
- **`payment_policy.mode = "user_funded"`** means the wallet calling `nexus tap execute` supplies the SUI for the standard TAP payment. The alternative, `agent_funded`, draws from the agent's payment vault.
- **`payment_policy.max_budget = 0`** disables the on-chain budget cap; the invoker still passes a `--payment-max-budget` at execute time, but the skill itself doesn't constrain it.
- **`schedule_policy.recurrence_kind = "once"`** keeps the demo synchronous. We're not using the scheduler in this guide.
- **`vertex_authorization_schema.fixed_tools: []`** and **`requires_payment: false`** mean the workflow does **not** mint a per-walk authorization cap for the vertex tool. The tool's `execute` runs on every dispatched walk without any caller-side check. That's the unauthorized shape this guide builds — the closing note on the last page covers what it would take to add a cap-gated check.
- **`shared_objects: []`** because the workflow doesn't need to lock any _additional_ shared objects beyond what the vertex tool already takes as arguments. `TutorialState` is supplied through the entry port, not declared here.
- **`interface_revision: 1`** is the standard TAP interface generation; bump it only when the on-chain TAP interface ships a new revision.

## 3. Dry-run

Run the local dry-run to confirm the DAG, the on-chain tool reference, and the skill requirements all line up:

```bash
nexus tap dry-run --config skill.tap.json
```

You should see the validation summary again — no chain calls happen yet. If the DAG references an unknown FQN or the entry ports don't match the registered tool's schema, the dry-run is where you'll catch it.

> Note: `dry-run` checks the DAG's structure and the skill's requirements against the configured Nexus deployment, but it does **not** execute the Move tool. The tool only runs once a leader picks the walk up after `nexus tap execute …`.

## 4. What changed

You now have:

- A `dag.json` whose only vertex is the on-chain transfer tool we wrote.
- A `skill.tap.json` whose `payment_policy` makes the invoker pay for each execution, whose `schedule_policy` keeps everything single-shot, and whose `vertex_authorization_schema` is left at its scaffold default (no `fixed_tools`, no `requires_payment`).
- A `validate-skill` and `dry-run` that both pass.

Everything is still local — nothing has touched the chain yet. The next page goes on chain three times: publish the Move package + DAG, register the on-chain tool, and bind the agent.

Next: [Publish, register, bind](5-tap-publish-and-register.md).
