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

## 2. Pick a payment coin and execute the skill

`nexus tap execute` submits one transaction that:

- Locks a standard TAP `ExecutionPayment` (paid out of the wallet's coins).
- Initialises the `DAGExecution` object and shares it.
- Calls the workflow's `request_network_to_execute_walks`, which emits a `RequestWalkExecutionEvent` for the `transfer_vertex` walk. The leader picks the event up, runs `transfer_vertex::execute(worksheet, state, recipient)` on chain, and marks the walk `Successful`. Your wallet sees `consumed` MIST debited from the payment object; the recipient receives whatever was in the treasury.

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

EXEC_JSON=$(nexus tap execute \
    --agent-id           "$AGENT" \
    --skill-id           0 \
    --input-json         "$INPUT_JSON" \
    --payment-max-budget 50000000 \
    --sui-gas-coin       "$PAYMENT_COIN" \
    --sui-gas-budget     500000000 \
    --json)

EXEC=$(printf '%s' "$EXEC_JSON" | jq -r '.execution_id')
echo "Execution: $EXEC"
```

The output gives you `execution_id` and `tx_checkpoint`:

```json
{
  "agent_dag": true,
  "agent_id": "0x31984f6acbb08ffa1dc053659c9e4af5327459b1ba2ca723ae04ca72dae98cf3",
  "skill_id": 0,
  "execution_id": "0x7b582d5fe921f4a35dcdb9897c5fc66e3d2ebae5301e3fd43376ed9576e15ea9",
  "digest": "...",
  "tx_checkpoint": 117137,
  "submit": { /* ... */ }
}
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

You can also confirm the treasury has been drained:

```bash
curl -s "$RPC_URL" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sui_getObject\",\"params\":[\"$STATE\",{\"showContent\":true}]}" |
    jq '.result.data.content.fields.treasury'
```

The field should be `null` — the coin moved out of state into the recipient address.

## 4. Inspect the execution and payment

`nexus dag inspect-execution` reads the on-chain `DAGExecution` plus its event stream, returning the structured `TaggedOutput` the vertex emitted:

```bash
nexus dag inspect-execution \
    --dag-execution-id "$EXEC" \
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

## What you built

End-to-end, your skill does this on every call:

1. Invoker submits `nexus tap execute` with a payment coin. Workflow locks the payment, records the `DAGExecution`, shares it, and emits a `RequestWalkExecutionEvent` for the `transfer_vertex` walk.
1. Leader picks the walk up, runs `transfer_vertex::execute(worksheet, state, recipient)` on chain.
1. `execute` drains the treasury, fires `public_transfer` to the recipient, stamps the worksheet, and returns the `Transferred` tagged output.
1. Workflow records the walk as `Successful`; the payment settles asynchronously.
1. You see the recipient balance go up; `inspect-execution` and `payments show` confirm the on-chain trail.

Re-running just means funding the treasury again and calling `tap execute` again with a fresh recipient (or the same one).

{% hint style="danger" %}
**This flow is minimal, not production-authorized.** Anyone who can reach a workflow dispatch against `(agent, skill)` will drain the treasury because there is no per-call authorization check on `transfer_vertex::execute`. The only thing keeping the funds in place is the `public(package)` visibility on `take_treasury`, which prevents *direct* Move-side calls from other packages; it does **not** prevent another skill author from publishing their own DAG that lists `tutorial.local.transfer_vertex@1` in its entry vertex and submitting `tap execute` against their own agent. Treat this guide as an introduction to the workflow lifecycle, not as a production pattern. The cap concept (`WorkflowVertexAuthorizationGrant`, `VertexAuthorizationCheckCap`, fixed-tool requirements, and `--workflow-authorization-cap-first`) plus the state-bound grant-id check that closes the multi-`(agent, skill)` attack will land in a follow-up cap-gated TAP guide.
{% endhint %}

## Next steps

The CLI surface for the rest of standard TAP is the natural follow-up reading — start with [CLI reference: `nexus tap`](../cli.md) and look at:

- **`nexus tap vault deposit`** to pre-fund a payment vault on the agent instead of paying per-call.
- **`nexus tap schedule-from-vault`**, **`nexus tap schedule-address-funded`**, and **`nexus tap schedule-default-address-funded`** to drive scheduled executions tied to a scheduler task.
- **`nexus tap update-skill`** for moving an existing skill to a new current revision.
- **`nexus dag execution-cost`** and **`nexus tap payments list`** for observability on what executions cost the operator across an agent's lifetime.
