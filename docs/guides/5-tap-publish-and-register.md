# Publish, register, bind

Three on-chain transactions in this page:

1. `nexus tap publish-skill` — publishes the TAP Move package, publishes the DAG, and writes a publish artifact JSON.
1. `nexus tool register onchain` — adds the on-chain transfer vertex to the tool registry.
1. `nexus tap bind` — creates a Talus agent and registers the skill against it in one transaction.

Each step records what the next one needs. Capture the IDs as you go.

## 1. Publish the Move package and DAG

```bash
nexus tap publish-skill \
    --config skill.tap.json \
    --out artifact.json \
    --sui-gas-budget 500000000 \
    --json | tee publish-skill.json
```

What `publish-skill` does in a single transaction:

- Builds and publishes `tap/` as a new Move package.
- Publishes the DAG in `dag.json` on chain, getting back a `dag_id`.
- Computes the substituted requirements from `skill.tap.json` (in this guide there is nothing to substitute because `fixed_tools` is empty).
- Builds a `TapPublishArtifact` from the DAG id, interface revision, and simplified requirements.
- Writes a `TapPublishArtifact` JSON to `--out`. Downstream `tap register-skill`, `tap bind`, and `tap update-skill` consume this file.

The JSON output gives you the two IDs you'll keep referring to:

```json
{
  "standard_tap": true,
  "function": "publish_skill",
  "tap_package_id": "0x556e5acd093ff4ba407cd5677abf27a72a7cf7e3023ae862806260d0ccfd54b2",
  "dag_id":         "0xcf1ce804f37973437354784683895736304727e9e6a19bbe467cd1f2a8fa2267",
  ...
}
```

(IDs in this guide are illustrative — yours will differ.)

Capture them as shell variables for the rest of the page:

```bash
PKG=$(jq -r '.tap_package_id' publish-skill.json)
DAG=$(jq -r '.dag_id'         publish-skill.json)
```

## 2. Find the freshly-published TutorialState id

The Move `init` function creates and _shares_ one `TutorialState`. Because it's a shared object it doesn't show up in `sui client objects <addr>` (which only lists owned objects), and `sui client object --json` returns BCS-encoded contents rather than parsed Move fields. We use the Sui JSON-RPC instead, which gives us the parsed `objectChanges` from the publish transaction and the parsed field tree of the shared state:

```bash
RPC_URL=$(grep -E '^rpc_url' ~/.nexus/conf.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ACTIVE=$(sui client active-address)

STATE=$(curl -s "$RPC_URL" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"suix_queryTransactionBlocks\",\"params\":[{\"filter\":{\"FromAddress\":\"$ACTIVE\"},\"options\":{\"showObjectChanges\":true}},null,3,true]}" |
    jq -r --arg pkg "$PKG" \
       '.result.data[].objectChanges[]?
        | select(.objectType? // "" | endswith("::tutorial_transfer::TutorialState"))
        | select(.objectType | startswith($pkg))
        | .objectId' | head -1)

WITNESS=$(curl -s "$RPC_URL" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sui_getObject\",\"params\":[\"$STATE\",{\"showContent\":true}]}" |
    jq -r '.result.data.content.fields.transfer_vertex_witness.fields.id.id')

echo "STATE=$STATE WITNESS=$WITNESS"
```

`STATE` is the shared `TutorialState` object id; `WITNESS` is the UID we'll register as the on-chain tool's `tool_witness_id`. The publish transaction is in the wallet's recent history because `nexus tap publish-skill` was the last state-changing call.

## 3. Register the on-chain transfer tool

```bash
nexus tool register onchain \
    --package        "$PKG" \
    --module         transfer_vertex \
    --tool-fqn       tutorial.local.transfer_vertex@1 \
    --description    "Tutorial transfer vertex" \
    --tool-witness-id "$WITNESS" \
    --sui-gas-budget 500000000 \
    --json
```

A few things worth knowing:

- **`--package` and `--module`** must match the Move package id and module name. The CLI derives the tool's input/output schemas from the on-chain Move ABI, so a mismatch fails fast.
- **`--tool-fqn`** must equal the FQN you set in the Move source and in `dag.json`. All three must agree.
- **`--tool-witness-id`** ties the tool registration to the `TransferVertexWitness` inside `TutorialState`.

The JSON output includes the derived `tool_id`, `tool_gas_id`, and the decoded schemas. The tool registers through the plain (non-cap-gated) entry point, so the workflow will dispatch walks against it without minting any per-walk authorization cap.

## 4. Create the agent and register the skill

`nexus tap bind` does this in a single PTB: it calls `tap::create_agent` and then `tap::register_skill` with the artifact's DAG id, input commitment, payment policy, schedule policy, and empty fixed-tool requirement.

```bash
nexus tap bind \
    --artifact artifact.json \
    --sui-gas-budget 500000000 \
    --json
```

Output:

```json
{
  "function":            "bind_agent_skill",
  "agent_id":            "0x31984f6acbb08ffa1dc053659c9e4af5327459b1ba2ca723ae04ca72dae98cf3",
  "skill_id":            0,
  "dag_id":              "0xcf1ce804f37973437354784683895736304727e9e6a19bbe467cd1f2a8fa2267",
  ...
}
```

`tap bind` always creates a _new_ agent every time it's called. Capture the agent id from the first invocation by piping the JSON to a file so a second `tap bind` does not silently provision a second agent on top of the first:

```bash
nexus tap bind \
    --artifact artifact.json \
    --sui-gas-budget 500000000 \
    --json > bind.json

AGENT=$(jq -r '.agent_id' bind.json)
```

> **Re-running the guide?** If you already have an agent, use `nexus tap register-skill --agent-id <existing> --artifact artifact.json` instead of `tap bind`.

## 5. Confirm in the registry

```bash
nexus tap registry show --json | jq --arg agent "$AGENT" '{
    agents: [.agents[] | select(.agent_id == $agent)],
    skills: [.skills[] | select(.agent_id == $agent)]
}'
```

You should see exactly one agent entry with your `agent_id`, and one skill entry with `skill_id: 0`, `current_interface_revision: 1`, and a DAG binding matching the value you captured from `publish-skill`. The skill's stored requirements carry an empty `fixed_tools` list.

`nexus tap default-agent show` is unaffected — that's the registry-managed default agent, not your new agent.

## What you have now

- A published TAP Move package and DAG. Their ids are in `publish-skill.json`; the reusable skill artifact is in `artifact.json`.
- A registered on-chain transfer vertex tool through the plain (non-cap-gated) entry point.
- A new Talus agent with one skill bound to it. The skill's `skill_id` is `0` (skills are numbered per-agent starting at 0).

The agent is empty so far — the treasury inside `TutorialState` is still `option::none()`. The next page funds it and runs the first execution.

Next: [Execute and verify the transfer](6-tap-execute-and-settle.md).
