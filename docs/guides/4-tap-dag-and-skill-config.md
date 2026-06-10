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
    "payment_policy": "UserFunded",
    "schedule_policy": {
      "recurrence": "Once",
      "allow_recursive": false
    },
    "fixed_tools": []
  },
  "interface_revision": 1
}
```

Field by field:

- **`tap_package_name`** must match the Move package name (`tutorial_transfer`) — `publish-skill` uses it as the named-address override when it compiles the package.
- **`dag_path` / `tap_package_path`** are relative to this file. The scaffold wires both for you and we keep them as-is.
- **`requirements.input_schema_commitment`** is an opaque byte vector that identifies the expected input shape. The JSON above encodes the ASCII string `tutorial-input`; anything reproducible works because downstream tooling treats it as an identifier.
- **`payment_policy: "UserFunded"`** means the wallet calling `nexus tap execute` supplies the SUI for the standard TAP payment. The alternative shape is `{"AgentFunded": {"max_budget": <MIST>}}`, which draws from the agent's payment vault and caps the requested budget.
- **`schedule_policy.recurrence = "Once"`** keeps the demo synchronous. Recursive schedules use `{"Recursive": {"min_interval_ms": <MS>, "max_occurrences": <COUNT or null>}}` and must set `allow_recursive: true`.
- **`fixed_tools: []`** means this skill does not preserve any registry-verified fixed tools in its DAG. Fixed tools are a DAG preservation requirement, not an authorization grant list.
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
- A `skill.tap.json` whose `payment_policy` makes the invoker pay for each execution, whose `schedule_policy` keeps everything single-shot, and whose `fixed_tools` requirement is empty.
- A `validate-skill` and `dry-run` that both pass.

Everything is still local — nothing has touched the chain yet. The next page goes on chain three times: publish the Move package + DAG, register the on-chain tool, and bind the agent.

Next: [Publish, register, bind](5-tap-publish-and-register.md).
