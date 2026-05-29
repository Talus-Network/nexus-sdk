# Execute and verify the transfer

Time to run the skill. From here on you need three values from earlier pages:

```bash
PKG=<tap_package_id from publish-skill>
STATE=<TutorialState object id from the publish>
AGENT=<agent_id from tap bind>
```

Pick a recipient address — any Sui wallet, including a vanity address you don't actually own. The example below uses `0x000…face`:

```bash
RECIPIENT=0x000000000000000000000000000000000000000000000000000000000000face
RPC_URL=$(grep -E '^rpc_url' ~/.nexus/conf.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
```

## 1. Fund the treasury

The skill's `execute` empties whatever coin is sitting in `TutorialState.treasury`. We deposit 0.1 SUI before running the workflow. `fund_treasury` is a plain Move entry function, so we use a small Sui PTB to split a coin off the gas object and pass it in:

```bash
sui client ptb \
    --split-coins gas '[100000000]' --assign deposit \
    --move-call "$PKG::tutorial_transfer::fund_treasury" "@$STATE" deposit.0 \
    --gas-budget 50000000 \
    --json | jq '.effects.status'
```

The `100000000` is the deposit amount in MIST (0.1 SUI). The PTB:

- Splits a 0.1 SUI coin off the transaction's gas coin.
- Calls `tutorial_transfer::fund_treasury(state, deposit_coin)`.
- Leaves the rest of the gas coin where it was.

Confirm the treasury balance:

```bash
curl -s "$RPC_URL" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sui_getObject\",\"params\":[\"$STATE\",{\"showContent\":true}]}" |
    jq -r '.result.data.content.fields.treasury.fields.balance'
```

Expected: `100000000`.

## 2. Pick a payment coin and execute the skill (cap-gated)

`nexus tap execute --grant-bind` submits one transaction that:

- Locks a standard TAP `ExecutionPayment` (paid out of the wallet's coins).
- Initialises the `DAGExecution` object.
- **Mints a `WorkflowVertexAuthorizationGrant`** for `transfer_vertex` and attaches it to the execution.
- **Calls `tutorial_transfer::bind_pending_grant(state, b"transfer_vertex", grant_id)`** so the state slot picks up the just-minted grant id.
- Shares the execution and emits the request-for-walk event.

The leader then picks up the walk, mints a `VertexAuthorizationCheckCap` from the attached grant, runs `transfer_vertex::execute(cap, worksheet, state, recipient)` on chain, and the workflow marks the walk `Successful`. Your wallet sees `consumed` MIST debited from the payment object; the recipient receives whatever was in the treasury.

Pick any coin that has enough room for the payment max-budget (50 million MIST in the example below — adjust to taste):

```bash
PAYMENT_COIN=$(sui client gas --json |
    jq -r '[.[] | select(.mistBalance > 60000000)][0].gasCoinId')
```

Then run the skill:

```bash
INPUT_JSON=$(jq -cn \
    --arg state     "$STATE" \
    --arg recipient "$RECIPIENT" \
    '{transfer_vertex: {"0": $state, "1": $recipient}}')

GRANT_BIND="transfer_vertex:$STATE:$PKG::tutorial_transfer::bind_pending_grant"

nexus tap execute \
    --agent-id           "$AGENT" \
    --skill-id           0 \
    --input-json         "$INPUT_JSON" \
    --payment-max-budget 50000000 \
    --grant-bind         "$GRANT_BIND" \
    --sui-gas-coin       "$PAYMENT_COIN" \
    --sui-gas-budget     500000000 \
    --json
```

`--grant-bind` is a colon-delimited triple:

```text
<vertex_name>:<state_object_id>:<package>::<module>::<function>
```

- `vertex_name` — the DAG vertex to mint a grant for. Must match the vertex in `dag.json`.
- `state_object_id` — the shared `TutorialState` object the SDK passes as the first argument when it calls the bind function.
- `package::module::function` — the bind hook (`bind_pending_grant` from page 2). The function signature must be exactly `(&mut <YourState>, vector<u8>, address)`.

Repeat `--grant-bind` once per cap-gated vertex if your skill has more than one.

The output gives you `execution_id` and `tx_checkpoint`:

```json
{
  "agent_dag": true,
  "agent_id": "0x31984f6acbb08ffa1dc053659c9e4af5327459b1ba2ca723ae04ca72dae98cf3",
  "skill_id": 0,
  "execution_id": "0x7b582d5fe921f4a35dcdb9897c5fc66e3d2ebae5301e3fd43376ed9576e15ea9",
  "digest": "...",
  "tx_checkpoint": 117137,
  "submit": {
    /* ... */
  }
}
```

Capture the execution id and the checkpoint:

```bash
EXEC=$(... | jq -r '.execution_id')
CKPT=$(... | jq -r '.tx_checkpoint')
```

## 3. Wait for the recipient balance to increase

The actual transfer happens when the leader executes the on-chain vertex tool. From your CLI's point of view, the easiest check is the recipient's SUI balance:

```bash
for i in $(seq 1 30); do
    BAL=$(curl -s "$RPC_URL" -H 'Content-Type: application/json' \
        -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"suix_getBalance\",\"params\":[\"$RECIPIENT\"]}" |
        jq -r '.result.totalBalance')
    if [ "$BAL" != "0" ] && [ -n "$BAL" ]; then
        echo "Recipient balance: $BAL MIST"
        break
    fi
    sleep 2
done
```

You should see the deposit amount (`100000000` MIST in the example) land within ~15 seconds on a healthy network.

You can also confirm the state's grant slot has been cleared (the vertex tool extracts it as part of the consume-and-drain):

```bash
curl -s "$RPC_URL" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sui_getObject\",\"params\":[\"$STATE\",{\"showContent\":true}]}" |
    jq '.result.data.content.fields | {treasury: .treasury, authorized_grant_id: .authorized_grant_id}'
```

Both fields should be `null` — the treasury moved out, the grant was consumed.

## 4. Inspect the execution and payment

`nexus dag inspect-execution` reads the on-chain `DAGExecution` plus its event stream, returning the structured `TaggedOutput` the vertex emitted:

```bash
nexus dag inspect-execution \
    --dag-execution-id    "$EXEC" \
    --execution-checkpoint "$CKPT" \
    --json | jq
```

Look for the `transferred` tagged variant with `amount` and `recipient` matching what you deposited and the destination wallet you supplied.

You can also inspect the payment object directly:

```bash
PAYMENT_ID=$(curl -s "$RPC_URL" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sui_getObject\",\"params\":[\"$EXEC\",{\"showContent\":true}]}" |
    jq -r '.result.data.content.fields.tap_payment_id')

nexus tap payments show --payment-id "$PAYMENT_ID" --json
```

Useful fields in that JSON:

- **`consumed`** — gas the leader's tool-eval consumed. Should be > 0.
- **`outstanding_locks`** — `0` once the walk is no longer holding the payment.
- **`accomplished` / `refunded` / `terminal` / `final_state`** — the settlement signal. Payment settlement is asynchronous and may lag the walk completion by seconds to minutes depending on leader cadence; the recipient balance arrives first, the payment terminal flag lands shortly after.

If you want to block until the payment object lands in a terminal state, `nexus tap payments wait --payment-id <ID> --timeout-secs 120` polls for you. It's optional — for verification, the recipient balance is the source of truth.

## 5. Cost summary

`nexus dag execution-cost` rolls up the standard TAP payment consumption:

```bash
nexus dag execution-cost --dag-execution-id "$EXEC" --json
```

Returns `payment_id`, `max_budget`, `locked_budget`, `consumed`, `outstanding_locks`, plus `accomplished`/`refunded` flags. This is the same data `payments show` returns, scoped to one execution.

## 6. Why this is now production-correct

The cap-gated flow we just ran is what closes the treasury gate end-to-end:

1. `--grant-bind` made the execute PTB mint a `WorkflowVertexAuthorizationGrant` and write its id into `TutorialState.authorized_grant_id` _before_ the leader saw a request-for-walk event. Any walk driven by anyone other than this PTB would start from `authorized_grant_id: None` — the vertex tool's `extract_authorized_grant` would simply abort.
1. The workflow minted the cap from that grant. The cap's `grant_id` is the same value we just wrote into state.
1. The vertex tool's `assert!(cap.grant_id == expected)` ties the cap, the grant, and the state together. A walk dispatched from any other `(agent, skill)` produces a cap whose `grant_id` was never written into our state, so the assertion fails and the walk aborts before any coin moves.

If someone unrelated tries to attack the skill — register their own `(agent', skill')` listing our tool in `fixed_tools`, mint _their_ grant, drain _our_ treasury — the cap they hand to `execute` carries their grant id, not ours. The assertion fails on their first attempt. The recipient receives nothing. The attacker still paid the gas for the failed walk; we lose nothing.

## What you built

End-to-end, your skill does this on every call:

1. Invoker submits `tap execute --grant-bind` with a payment coin.
1. Workflow locks the payment, records the `DAGExecution`, mints a `VertexAuthorizationGrant` attached to it, and the SDK calls `bind_pending_grant(state, b"transfer_vertex", grant_id)` to lock state to that grant.
1. Workflow emits the request-for-walk event; the SDK shares the execution.
1. Leader picks the walk up, mints a `VertexAuthorizationCheckCap` from the grant, dry-runs `transfer_vertex::execute`, then submits the real transaction.
1. `execute` asserts `cap.grant_id == state.authorized_grant_id`, consumes the cap, drains the treasury, fires `public_transfer` to the recipient, stamps the worksheet, and returns the `Transferred` tagged output.
1. Workflow records the walk as `Successful`; the payment settles asynchronously.
1. You see the recipient balance go up; `inspect-execution` and `payments show` confirm the on-chain trail.

Re-running just means funding the treasury again and calling `tap execute --grant-bind` again with a fresh recipient (or the same one). The previous walk consumed the grant and cleared the state slot; the new walk mints a fresh grant.

## Next steps

You now have the cap-gated synchronous-transfer pattern down. The CLI surface for the rest of standard TAP is the natural follow-up reading — start with [CLI reference: `nexus tap`](../cli.md) and look at:

- **`nexus tap vault deposit`** to pre-fund a payment vault on the agent instead of paying per-call.
- **`nexus tap schedule-from-vault`**, **`nexus tap schedule-address-funded`**, and **`nexus tap schedule-default-address-funded`** to drive scheduled executions tied to a scheduler task.
- **`nexus tap announce`** for shipping a new endpoint revision of an existing skill.
- **`nexus dag execution-cost`** and **`nexus tap payments list`** for observability on what executions cost the operator across an agent's lifetime.
