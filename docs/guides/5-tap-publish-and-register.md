# Publish, register, bind

Three on-chain transactions in this page:

1. `nexus tap publish-skill` — publishes the TAP Move package, publishes the DAG, and writes a publish artifact JSON.
1. `nexus tool register onchain --workflow-authorization-cap-first` — adds the on-chain transfer vertex to the tool registry through the cap-gated entry point so the workflow will mint a `VertexAuthorizationCheckCap` for every dispatch.
1. `nexus tap bind` — creates a Talus agent and registers the skill against it in one transaction.

Each step records what the next one needs. Capture the IDs as you go.

## 1. Publish the Move package and DAG

```bash
nexus tap publish-skill \
    --config tutorial-transfer/skill.tap.json \
    --out tutorial-transfer/artifact.json \
    --sui-gas-budget 500000000 \
    --json
```

What `publish-skill` does in a single transaction:

- Builds and publishes `tap/` as a new Move package.
- Publishes the DAG in `dag.json` on chain, getting back a `dag_id`.
- Substitutes the `0x0` sentinel in `skill.tap.json`'s `fixed_tools[].package_id` with the freshly-published `tap_package_id` (so the artifact's authorization schema commits to the real package address).
- Computes the endpoint config digest from the substituted requirements, shared objects, and interface revision.
- Writes a `TapPublishArtifact` JSON to `--out`. Downstream `tap register-skill` / `tap bind` / `tap announce` all consume this file.

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
PKG=$(jq -r '.tap_package_id' tutorial-transfer/artifact.json)
DAG=$(jq -r '.dag_id'         tutorial-transfer/artifact.json)
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

## 3. Register the on-chain transfer tool (cap-gated)

```bash
nexus tool register onchain \
    --package        "$PKG" \
    --module         transfer_vertex \
    --tool-fqn       tutorial.local.transfer_vertex@1 \
    --description    "Tutorial transfer vertex" \
    --tool-witness-id "$WITNESS" \
    --workflow-authorization-cap-first \
    --reuse-if-exists \
    --sui-gas-budget 500000000 \
    --json
```

A few things worth knowing:

- **`--workflow-authorization-cap-first`** is the cap-gated registration. It marks the tool record in the registry so the workflow knows to mint a `VertexAuthorizationCheckCap` for every dispatch. Without this flag, the workflow refuses to mint a cap and `dag::create_vertex_authorization_grant` aborts with `EVertexAuthorizationGrantToolNotCapFirst` on the next page.
- **`--package` and `--module`** must match the Move package id and module name. The CLI derives the tool's input/output schemas from the on-chain Move ABI, so a mismatch fails fast.
- **`--tool-fqn`** must equal the FQN you set in the Move source and in `dag.json`. All three must agree.
- **`--tool-witness-id`** ties the tool registration to the `TransferVertexWitness` inside `TutorialState`.
- **`--reuse-if-exists`** makes the call idempotent: if the FQN is already registered with matching parameters, the command decodes the existing refs and returns `reused: true` instead of erroring.

The JSON output includes the derived `tool_id`, `tool_gas_id`, decoded schemas, and — critically — `"workflow_authorization_cap_first": true`. Confirm that flag is `true` in the response before moving on. If it's `false` the tool is registered through the non-cap entry and the rest of the tutorial won't work; in that case unregister, pick a fresh FQN, and re-register with `--workflow-authorization-cap-first`.

## 4. Create the agent and register the skill

`nexus tap bind` does this in a single PTB: it calls `tap::create_agent` and then `tap::register_skill_with_vertex_authorization_schema` (auto-routed because our `vertex_authorization_schema` has non-default `fixed_tools` and `requires_payment: true`).

```bash
nexus tap bind \
    --artifact tutorial-transfer/artifact.json \
    --operator "$(sui client active-address)" \
    --sui-gas-budget 500000000 \
    --json
```

Output:

```json
{
  "function":            "bind_agent_skill",
  "agent_id":            "0x31984f6acbb08ffa1dc053659c9e4af5327459b1ba2ca723ae04ca72dae98cf3",
  "skill_id":            0,
  "tap_package_id":      "0x556e5acd093ff4ba407cd5677abf27a72a7cf7e3023ae862806260d0ccfd54b2",
  "config_digest_hex":   "...",
  ...
}
```

Capture the agent id:

```bash
AGENT=$(nexus tap bind \
    --artifact tutorial-transfer/artifact.json \
    --operator "$(sui client active-address)" \
    --sui-gas-budget 500000000 \
    --json | jq -r '.agent_id')
```

> **Re-running the guide?** `tap bind` always creates a _new_ agent. If you already have one, use `nexus tap register-skill --agent-id <existing> --artifact tutorial-transfer/artifact.json` instead. That command auto-detects the cap-gated schema and routes through the same PTB.

## 5. Confirm in the registry

```bash
nexus tap registry show --json | jq '{
    agents: [.agents[] | select(.agent_id == env.AGENT)],
    skills: [.skills[] | select(.agent_id == env.AGENT)]
}'
```

You should see exactly one agent entry with your `agent_id` + `operator`, and one skill entry with `skill_id: 0` and `dag_id` matching the value you captured from `publish-skill`. The skill's stored requirements carry the `fixed_tools` entry pointing at your published `tap_package_id` (the `0x0` sentinel got substituted by `publish-skill`).

`nexus tap default-target show` is unaffected — that's the registry-managed default executor, not your new agent.

## What you have now

- A published TAP Move package and DAG. Their ids are in `artifact.json`.
- A registered cap-gated on-chain transfer vertex tool. The registry record has `workflow_authorization_cap_first: true`, so the workflow will mint and require a `VertexAuthorizationCheckCap` for every dispatch.
- A new Talus agent with one skill bound to it. The skill's `skill_id` is `0` (skills are numbered per-agent starting at 0). The on-chain skill record carries the `fixed_tools` schema so the workflow knows which tool the cap covers.

The agent is empty so far — the treasury inside `TutorialState` is still `option::none()`. The next page funds it and runs the first execution.

Next: [Execute and verify the transfer](6-tap-execute-and-settle.md).
