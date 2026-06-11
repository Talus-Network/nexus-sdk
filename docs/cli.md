# Nexus CLI

> concerns [`nexus-cli` repo][nexus-cli-repo]

The Nexus CLI is a set of tools that is used by almost all Actors in the Nexus ecosystem.

- Agent developers use it to create Talus Agent Packages
- Tool developers use it to scaffold, validate and register tools
- Nexus developers use it for debugging, testing and both use cases mentioned above

## Interface design

{% hint style="info" %}
Each command can be passed a `--json` flag that will return the output in JSON format. This is useful for programmatic access to the CLI.
{% endhint %}

### `nexus tool`

Set of commands for managing Tools.

---

**`nexus tool new --name <name> --template <template>`**

Create a new Tool scaffolding in a folder called `<name>`. Which files are generated is determined by the `--template` flag. I propose having `templates/tools/<template>.template` files that contain the Tool skeleton files. For example for `rust` it'd be a `Cargo.toml` with the `nexus-toolkit` dependency, and a `src/main.rs` file that shows a basic use case of the crate.

---

**`nexus tool validate offchain --url <URL>`**

Validate an off-chain Nexus Tool on the provided URL. This command checks whether the URL hosts a valid Nexus Tool interface:

1. `GET /meta` contains Tool metadata that is later stored in our Tool Registry, this contains the `fqn`, the `url` which should match the one in the command and the Tool input and output schemas. Output schema is also validated to contain a top-level `oneOf` to adhere to Nexus output variant concept.
1. `GET /health` simple health check endpoint that needs to return a `200 OK` in order for the validation to pass.
1. `POST /invoke` the CLI can check that the endpoint exists.

{% hint style="success" %}
As an improvement, the command could take a `[data]` parameter that invokes the Tool and checks the response against the output schema.
{% endhint %}

This command should also check that the URL is accessible by Leader nodes. For local testing, it should still be usable with `localhost` Tools, printing a warning.

---

**`nexus tool validate onchain --ident <IDENT>`**

Validate an on-chain Nexus Tool identified by `<IDENT>` (the Move module address used by the tool).

---

**`nexus tool register offchain (--url <URL> | --from-meta <FILE|->) [--invocation-cost <MIST>] [--collateral-coin <OBJECT_ID>] [--batch] [--no-save]`**

Registers an off-chain Nexus Tool with the Tool Registry. Either `--url` (the live HTTP endpoint) or `--from-meta` (a path to a JSON metadata file as produced by the tool binary's `--meta` flag, or `-` for stdin) is required. The live-URL path makes a request to `GET <url>/meta` to fetch the Tool definition; the `--from-meta` path skips that HTTP fetch (useful when the tool isn't reachable from the CLI host). The command then submits a TX to the Tool Registry, locks the collateral coin, and sets the single invocation cost (defaults to `0` MIST).

This returns 2 OwnerCap object IDs that can be used to manage the Tool and its Gas settlement methods.

If the `--batch` flag is passed, the command accepts a URL of a webserver hosting multiple tools and registers all of them at once. `nexus-toolkit` automatically generates a `GET /tools` endpoint that returns a list of URLs of all tools registered on that server. The CLI will then iterate over the list and register each tool. `--batch` is incompatible with `--from-meta`.

Upon successful registration, both OwnerCap object IDs are saved to the CLI configuration file and automatically used for subsequent commands. This happens unless the `--no-save` flag is passed, in which case the OwnerCaps are not saved.

The JSON output for each registered tool includes the transaction `digest`, `tool_fqn`, the derived `tool_id` and `tool_gas_id`, `owner_cap_over_tool_id` and `owner_cap_over_gas_id`, and the fully-decoded post-registration `Tool` record under the `tool` field — the same shape `nexus tool inspect` and `nexus tool register onchain` emit. In `--batch` mode each tool's result is one entry in the top-level JSON array.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tool register onchain --package <ADDRESS> --module <MODULE> --tool-fqn <FQN> --description <DESCRIPTION> --tool-witness-id <OBJECT_ID> [--workflow-authorization-cap-first] [--collateral-coin <OBJECT_ID>] [--timeout <DURATION>] [--no-save]`**

Registers an on-chain Nexus Tool that resolves to a Move package, module, and witness object on Sui. The CLI introspects the Move module's `execute` entry function to auto-generate the input schema and its `Output` enum for the output schema; both can be customized interactively when stdin is a TTY (skipped in `--json` mode). The tool's `Tool` and `ToolGas` object IDs are derived locally from the FQN and surfaced in the JSON response alongside the `OwnerCap<OverTool>` returned by the on-chain call.

Registration is partially idempotent — the Move-side `register_on_chain_tool` aborts with `EFqnAlreadyExists` when the FQN is already claimed, but the CLI surfaces that abort by changing the output to notify the user about the fact that the tool is already registered.

`--workflow-authorization-cap-first` routes through `register_on_chain_tool_with_workflow_authorization_cap`, which marks the registered tool as cap-gated. Use this when the workflow executor must mint a `WorkflowVertexAuthorizationGrant` before each call so the runtime can derive a `VertexAuthorizationCheckCap` and hand it to the tool's `execute` function — without the grant, the cap can't be minted and the vertex (i.e. the tool itself) can't run.

The JSON output includes the transaction `digest` + `tx_checkpoint`, the locally-derived `tool_id` and `tool_gas_id`, the `owner_cap_over_tool_id` and `owner_cap_over_gas_id` returned by the on-chain call, and the fully-decoded post-registration `Tool` record under the `tool` field — the same shape `nexus tool inspect` emits, so scripts only need to learn one Tool contract.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tool inspect --tool-fqn <FQN>`**

Derives the `Tool` and `ToolGas` object IDs from the configured `ToolRegistry`/`GasService` and the supplied FQN, probes both objects on-chain, and emits a stable JSON summary so callers do not need to BCS-decode the `Tool` object themselves. Works for both HTTP and Sui tools — the variant lives inside the decoded `Tool` record.

The JSON includes `tool_id`, `tool_gas_id`, `exists` (true when both objects are present), and the fully-decoded on-chain `Tool` record under the `tool` field (or `null` when `exists` is false). When the tool is HTTP, `tool.ref` is the `Http { url }` variant; when it is on-chain Sui, `tool.ref` is the `Sui { package_address, module_name, tool_witness_id }` variant. The stored `description`, `input_schema`, `output_schema`, `workflow_authorization_cap_first`, `registered_at_ms`, and `unregistered_at_ms` all live under `tool` too. When the tool does not exist yet, the derived IDs are still returned so the caller can pre-compute them.

---

**`nexus tool unregister --tool-fqn <FQN> [--owner-cap <OBJECT_ID>] [--yes]`**

Command that sends a TX to the Tool Registry and unregisters a Tool with the provided `<FQN>`. By default the command prompts for confirmation as unregistering a Tool will render all DAGs using it unusable; pass `--yes` (or `-y`) to skip the confirmation prompt, which is useful for CI pipelines.

If the OwnerCap object ID is not passed, the CLI will attempt to use the one saved in the configuration file.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tool claim-collateral --tool-fqn <fqn> --owner-cap [object_id]`**

After the period of time configured in our Tool Registry, let the Tool developer claim the collateral, transferring the amount back to their wallet.

If the OwnerCap object ID is not passed, the CLI will attempt to use the one saved in the configuration file.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tool set-invocation-cost --tool-fqn <fqn> --owner-cap [object_id] --invocation-cost <mist>`**

Tool owners can change the invocation cost of their Tools specified by the FQN. This operation requires that the `OwnerCap<OverGas>` object is passed to the command and owned by the transaction sender.

If the OwnerCap object ID is not passed, the CLI will attempt to use the one saved in the configuration file.

<!-- TODO: <https://github.com/Talus-Network/nexus-next/issues/283> -->

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tool auth`**

Commands for operating signed HTTP (message signatures) for off-chain Tools.

Signed HTTP is Nexus’ application-layer authentication for `POST /invoke`:

- Leader node → Tool requests are signed so the Tool can verify which Leader node is calling and prevent replay.
- Tool → Leader node responses are signed so the Leader node can verify provenance and bind a response to a specific request.

These commands help Tool operators:

- generate an Ed25519 Tool message-signing keypair,
- register/rotate the Tool’s message-signing public key on-chain (Network Auth), and
- export a local `allowed_leaders.json` file consumed by the Tool runtime for request verification (no RPC at runtime).

See also:

- [Tool Communication (HTTPS + Signed HTTP)](guides/tool-communication.md)

---

**`nexus tool auth keygen [--out <path>]`**

Generates a new Ed25519 Tool message-signing keypair.

- If `--out` is provided, writes a JSON file containing `private_key_hex` and `public_key_hex`.
- You will use the private key in the Tool runtime config (`tool_signing_key`).
- You will register the public key on-chain via `register-key`.

---

**`nexus tool auth register-key --tool-fqn <FQN> --signing-key <KEY_OR_PATH> [--owner-cap <OBJECT_ID>] [--description <TEXT>] [--skip-if-active] ...gas`**

Registers (or rotates) the Tool’s message-signing key in the on-chain Network Auth registry.

- Requires an `OwnerCap<OverTool>` (the tool ownership cap) to prove Tool identity.
- Requires a proof-of-possession signature so the chain can verify the registrant controls the private key.
- Returns the registered `tool_kid` (key id) which must match the Tool runtime config.
- `--skip-if-active` makes the command idempotent: if the supplied public key is already the active key for this tool, registration is skipped. Useful in CI to avoid re-registering an unchanged key.

If `--owner-cap` is omitted, the CLI will try to use the OwnerCap saved in the CLI config for that Tool.

---

**`nexus tool auth list-keys --tool-fqn <FQN>`**

Lists every message-signing key currently registered for the given tool in the on-chain Network Auth registry. Useful for confirming a `register-key` rotation landed and for auditing which keys can sign on the tool's behalf.

---

**`nexus tool auth export-allowed-leaders (--all | --leader <LEADER_CAP_ID>...) --out <PATH>`**

Exports a local allowlist file (JSON) of permitted Leader nodes and their active signing keys.

Use `--all` to export entries for all leaders registered in `network_auth` (recommended), or pass one or more `--leader` capability IDs.

This file is consumed by the Rust toolkit runtime (`allowed_leaders_path`) so the Tool can verify signed requests without performing Sui RPC calls at runtime.

---

**`nexus tool auth sync-allowed-leaders --out <path> [--interval <duration>] [--once]`**

Continuously syncs an `allowed_leaders.json` file from on-chain `network_auth` (polling).

- `--interval` accepts human durations like `500ms`, `5s`, `2m`, `1h` (default: `30s`).
- Use `--once` to sync a single time and exit.

This is useful for running as a sidecar next to a Tool so leader allowlists stay up-to-date without restarting the Tool.

---

**`nexus tool list`**

List all Nexus Tools available in the Tool Registry. This reads the dynamic object directly from Sui.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tool update-timeout --tool-fqn <fqn> --timeout <duration> [--owner-cap <object_id>]`**

Updates the timeout duration for the specified Tool. This timeout tells the Nexus execution engine how long to wait for a response from the Tool before considering the invocation failed.

- `--tool-fqn` specifies the fully qualified name of the tool.
- `--timeout` sets the new timeout duration.
- `--owner-cap` optionally specifies the owner capability ID for the tool. This must be provided unless the owner cap is saved in the CLI configuration file.

---

### `nexus dag`

Set of commands for managing JSON DAGs.

---

**`nexus dag validate --path <path>`**

Performs static analysis on a JSON DAG at the provided path. It enforces rules described in [the Workflow docs](../nexus-next/packages/workflow.md). Th

{% hint style="info" %}
If you're unsure about the terminology used below, please refer to the [glossary](../nexus-next/glossary.md).
{% endhint %}

1. For each entry group...
1. Find all input ports
1. For each input port...
1. Find all paths from relevant entry vertices to this input port
1. Ensure that net concurrency on that input port node is 0
   - `N` input ports on a tool reduce the graph concurrency by `N - 1` because walks are consumed if they are waiting for more input port data
   - `N` output ports on an output variant increase the graph concurrency by `N - 1` because `N` concurrent walks are spawned, while the 1 leading into the output variant is consumed
   - If net concurrency is `< 0`, the input port can never be reached
   - If net concurrency is `> 0`, there is a race condition on the input port

---

**`nexus dag publish --path <path>`**

Publishes a JSON DAG at the provided path to the Workflow. Static analysis is automatically performed prior to publishing. This command then returns the on-chain DAG object ID that can be used to execute it.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus dag execute --dag-id <OBJECT_ID> --input-json <DATA> [--entry-group <NAME>] [--remote vertex.port,...] [--inspect] [--priority-fee-per-gas-unit <MIST>] [--payment-coin <OBJECT_ID>] [--payment-budget <MIST>]`**

Execute a DAG with the provided `<OBJECT_ID>`. This command also accepts an entry `<NAME>` of vertices to be invoked. Find out more about entry groups in [[Package: Workflow]]. Entry group defaults to a standardized `_default_group` string.

The input `<DATA>` is a JSON string with the following structure:

- The top-level object keys refer to the _entry vertex names_
- Each top-level value is an object and its keys refer to the _input port names_ of each vertex (this object can be empty if the vertex has no input ports)
- Values of the second-level object are the data that should be passed to each input port

The `--inspect` argument automatically triggers `nexus dag inspect-execution` upon submitting the execution transaction.

The `--remote` argument accepts a list of `{vertex}.{port}` strings that refer to entry ports and their vertices. The data associated with these ports is stored remotely based on the configured preferred remote storage provider. Note that it is required that the user configure these remote storage providers via the `$ nexus conf set --help` command.

Supported remote storage providers are:

- Walrus

Talus agent execution payment is supplied via two optional flags:

- `--payment-coin <OBJECT_ID>` — SUI coin to lock as the execution payment. When omitted, the execution is recorded with no Talus agent payment context.
- `--payment-budget <MIST>` — optional cap on the payment budget. Defaults to the full balance of `--payment-coin` when omitted.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus dag inspect-execution --dag-execution-id <OBJECT_ID>`**

Inspects a DAG execution process from its `DAGExecution` object ID. The SDK derives the starting checkpoint by chasing `Owner::Shared(initial_shared_version)` → time-pinned `previous_transaction` → that transaction's `checkpoint`, so callers no longer have to track which checkpoint the execution was committed in. The command subscribes to the on-chain event stream starting at that checkpoint and emits each walk advance, end-state, terminal `_err_eval` record, and the final execution-finished event in human-readable form or as a JSON trace when `--json` is set.

---

**`nexus dag execution-cost --dag-execution-id <OBJECT_ID>`**

Shows the Talus agent execution payment consumed by a DAG execution. Decodes `DAGExecution.standard_tap_context` to find the linked `TapExecutionPayment` object and emits its `payment_id`, `max_budget`, `locked_budget`, `consumed`, `outstanding_locks`, `accomplished`, and `refunded` fields as stable JSON. Pair with `nexus tap payments wait --payment-id <ID>` to drive settlement to a terminal state.

---

### `nexus scheduler`

Manage scheduler tasks, occurrences, and periodic schedules.

#### Concepts

- **Task**: owned on-chain object that bundles metadata, policies, and lifecycle state (active/paused/canceled):
  - an **execution policy** (“what to do”): today this is “begin DAG execution”, but tasks are designed to support additional execution types in the future
  - a **constraints policy** (“when it may run”): defines when the task is eligible to execute. In the current scheduler this eligibility is time-based and expressed via **occurrences** (start + optional deadline windows) produced by either queue-based scheduling or periodic scheduling
- In other words: the scheduler is task/schedule/occurrence oriented; DAG execution is just the current default execution policy.
- **Queue-based scheduling**: you enqueue one-off **occurrences** for a task (as many as you want). This is intentionally generic: by enqueueing occurrences at different times (and with different priorities), you can implement custom strategies such as delayed runs, retries, backoff, and more.
- **Periodic scheduling**: you configure a repeating schedule (first start + period, plus optional deadline window and max iterations). The scheduler produces occurrences automatically based on that config.
- **Occurrence**: an eligibility window for a single task run. An occurrence carries a start time (or start offset), optional deadline window, and `priority_fee_per_gas_unit` (ordering/pricing signal). When the window is open and the occurrence is consumed, the task’s execution policy runs once. Ordering is deterministic: earlier start wins; ties break on higher `priority_fee_per_gas_unit`; then FIFO.
- Each eligible (consumed) occurrence triggers one run of the task’s execution policy: periodic tasks run the same execution periodically, and queue tasks run it once per enqueued occurrence.
- Each run is independent: the scheduler does not automatically pass outputs/data from one run to the next. If you need stateful behavior across runs, persist and manage that state externally.

---

**`nexus scheduler task create --dag-id <id> [--entry-group <group>] [--input-json <json>] [--remote vertex.port,...] [--metadata key=value ...] [--execution-priority-fee-per-gas-unit <mist>] [--schedule-start-ms <ms> | --schedule-start-offset-ms <ms>] [--schedule-deadline-offset-ms <ms>] [--schedule-priority-fee-per-gas-unit <mist>] [--generator queue|periodic] [--agent-id <id> --skill-id <u64>]`**

Creates a new scheduler task tied to the specified DAG. Key options:

- `--entry-group` points to the DAG entry function and defaults to `_default_group`.
- `--input-json` provides inline input data; `--remote vertex.port,...` forces specific inputs to be uploaded to the configured remote storage instead of inlining them on-chain.
- `--metadata key=value` attaches arbitrary metadata entries and replaces any existing entries if the command is re-run.
- `--execution-priority-fee-per-gas-unit` sets the priority fee for future DAG executions launched by the task.
- `--schedule-start-ms` supplies an absolute first-occurrence timestamp (milliseconds since epoch) while `--schedule-start-offset-ms` uses the current Sui clock as the base; the two switches are mutually exclusive.
- `--schedule-deadline-offset-ms` sets the completion window relative to whichever start time was selected, and `--schedule-priority-fee-per-gas-unit` sets the priority fee for that initial occurrence.
- `--generator` chooses the generator responsible for future occurrences (`queue` by default, `periodic` to enable recurring schedules).
- `--agent-id` and `--skill-id` (must be supplied together, or both omitted) scope the task to a registered TAP agent skill. When set, the workflow dispatches walks under the agent-bound execution policy (`BeginAgentExecutionWitness`) instead of the default DAG-execution policy, so the task can be paired with `tap schedule-from-vault`, `tap schedule-address-funded`, or `tap schedule-default-address-funded` to fund and trigger occurrences.

Initial schedule arguments (`--schedule-*`) are only valid for queue-based tasks. Selecting `--generator periodic` prepares the task for periodic execution, but you must configure the recurring schedule separately via `nexus scheduler periodic set`.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI and holds sufficient SUI for gas.
{% endhint %}

---

**`nexus scheduler task inspect --task-id <id>`**

Fetches the on-chain task object, prints high-level metadata (owner, metadata entries, payload sizes), and emits a JSON payload containing the raw task structure for tooling.

---

**`nexus scheduler task metadata --task-id <id> --metadata key=value [...]`**

Replaces all task metadata entries with the provided key/value pairs.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI and holds sufficient SUI for gas.
{% endhint %}

---

**`nexus scheduler task pause|resume|cancel --task-id <id>`**

Mutates the scheduling state for a task. `pause` and `resume` toggle consumption of occurrences, while `cancel` clears pending occurrences and permanently disables scheduling.

{% hint style="info" %}
These commands require that a wallet is connected to the CLI and holds sufficient SUI for gas.
{% endhint %}

---

**`nexus scheduler occurrence add --task-id <id> [--start-ms <ms> | --start-offset-ms <ms>] [--deadline-offset-ms <ms>] [--priority-fee-per-gas-unit <mist>]`**

Schedules a one-off occurrence for the task. `--start-ms` and `--start-offset-ms` are mutually exclusive and control when the occurrence enters the queue (absolute milliseconds or an offset from the current Sui clock). Deadlines are expressed only as offsets from that start time, and `--priority-fee-per-gas-unit` adjusts the priority fee applied to the queued occurrence.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI and holds sufficient SUI for gas.
{% endhint %}

---

**`nexus scheduler periodic set --task-id <id> --first-start-ms <ms> --period-ms <ms> [--deadline-offset-ms <ms>] [--max-iterations <count>] [--priority-fee-per-gas-unit <mist>]`**

Configures or updates a periodic schedule for the task. `--first-start-ms` pins the next execution to an absolute timestamp (milliseconds since epoch), `--period-ms` defines the spacing between subsequent occurrences, `--deadline-offset-ms` applies the same completion window after every generated start, `--max-iterations` limits how many future occurrences may be emitted automatically (omit for infinite), and `--priority-fee-per-gas-unit` sets the priority fee charged for each periodic occurrence.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI and holds sufficient SUI for gas.
{% endhint %}

---

**`nexus scheduler periodic disable --task-id <id>`**

Removes the periodic schedule while leaving any existing sporadic occurrences untouched.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI and holds sufficient SUI for gas.
{% endhint %}

---

### `nexus gas`

Set of commands to manage Nexus gas ticket extensions (expiry tickets and limited-invocations tickets) for off-chain tools.

{% hint style="info" %}
Talus agent execution payments are managed through `nexus dag execute --payment-coin`, `nexus tap execute --payment-*` flags, `nexus dag execution-cost`, and `nexus tap payments`. The commands below are for the tool-side gas ticket extensions that off-chain tool owners can enable for their tools.
{% endhint %}

---

**`nexus gas expiry enable --tool-fqn <FQN> --cost-per-minute <MIST> [--owner-cap <OBJECT_ID>]`**

The tool owners can enable the expiry gas extension for their tools specified by the FQN. This allows users to buy expiry gas tickets that can be used to pay for the tool usage for a limited amount of time.

If the OwnerCap object ID is not passed, the CLI will attempt to use the one saved in the configuration file.

Calling this command again with a different `cost-per-minute` value will update the cost of the tickets.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus gas expiry disable --tool-fqn <fqn> --owner-cap [object_id]`**

Disables the expiry gas extension for the tool specified by the FQN.

If the OwnerCap object ID is not passed, the CLI will attempt to use the one saved in the configuration file.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus gas expiry buy-ticket --tool-fqn <fqn> --minutes <minutes> --coin <object_id>`**

Buy an expiry gas ticket for the tool specified by the FQN. This ticket can then be used to pay for the tool usage for the specified amount of `minutes` if a DAG is executed from the same address that was used to buy this ticket. The ticket is paid for with the provided `coin` object.

This transaction fails if the tool does not have the expiry gas extension enabled.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus gas limited-invocations enable --tool-fqn <fqn> --owner-cap [object_id] --cost-per-invocation <mist> --min-invocations <count> --max-invocations <count>`**

The tool owners can enable the limited invocations gas extension for their tools specified by the FQN. This allows users to buy limited invocations gas tickets that can be used to pay for a specific number of tool invocations.

The `cost-per-invocation` parameter sets the price in MIST for each invocation. The `min-invocations` and `max-invocations` parameters define the allowed range for ticket purchases.

If the OwnerCap object ID is not passed, the CLI will attempt to use the one saved in the configuration file.

Calling this command again with different parameters will update the cost and limits for new tickets.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus gas limited-invocations disable --tool-fqn <fqn> --owner-cap [object_id]`**

Disables the limited invocations gas extension for the tool specified by the FQN.

If the OwnerCap object ID is not passed, the CLI will attempt to use the one saved in the configuration file.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus gas limited-invocations buy-ticket --tool-fqn <fqn> --invocations <count> --coin <object_id>`**

Buy a limited invocations gas ticket for the tool specified by the FQN. This ticket can then be used to pay for the specified number of tool `invocations` if a DAG is executed from the same address that was used to buy this ticket. The ticket is paid for with the provided `coin` object.

The number of invocations must be within the min/max range configured by the tool owner when enabling the extension.

This transaction fails if the tool does not have the limited invocations gas extension enabled.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

### `nexus tap`

Commands for authoring, publishing, registering, executing, scheduling, and inspecting custom TAP packages and Talus agent skills. The TAP package surface covers the full lifecycle from scaffolding a new skill locally through publishing it on-chain, binding it to an agent, updating the current skill revision, and inspecting registry/payment/scheduled-task state.

A typical lifecycle looks like:

1. `tap scaffold` — generate a TAP package + DAG + skill config skeleton.
1. `tap validate-skill` / `tap dry-run` — verify the local artifacts and report the validated skill name plus interface revision.
1. `tap publish-skill` — publish the Move package, publish the DAG, derive the current skill artifact, and write a portable publish artifact.
1. `tap create-skill-artifact` — build the same portable skill artifact locally when the DAG and TAP package were published by another flow.
1. `tap create-agent` (or `tap bind` to do create+register in one PTB) — get an on-chain agent identity.
1. `tap register-skill` and `tap update-skill` — bind the skill to the agent and move future executions to a new current skill revision when the DAG or policies change.
1. `tap execute` / `tap schedule` — run the skill once or schedule recurring/queued executions.
1. `tap registry show`, `tap default-agent show`, `tap requirements`, `tap payments show`/`wait`, `tap vault balance`, and `tap payments list` — inspect on-chain state and drive settlement.

All commands accept `--json` for stable machine-readable output.

#### Authoring (local-only)

---

**`nexus tap scaffold --name <NAME> [--target <PATH>]`**

Generates a TAP package skeleton in `<target>/<name-kebab-cased>/`, containing a `tap/` Move package, a `dag.json` DAG, and a `skill.tap.json` skill config that points at both. The package name is snake-cased from the supplied `<name>`, and the module name matches the package name. The JSON output contains the resolved path to the generated directory. `--target` defaults to the current directory.

---

**`nexus tap validate-skill --config <PATH>`**

Statically validates a TAP skill config JSON and the local TAP package it references — package manifest, named-address aliases, module declarations, and the bundled DAG JSON. The TAP package is resolved from the config's `tap_package_path` (interpreted relative to the config file's directory). No network is required.

---

**`nexus tap dry-run --config <PATH>`**

Runs `validate-skill` and checks the local TAP package, DAG, and simplified skill requirements before any chain write. The JSON output reports `valid`, the skill name, and the requested interface revision.

#### Publishing (on-chain authoring)

---

**`nexus tap publish-skill --config <PATH> [--out <PATH>]`**

Publishes the TAP Move package, publishes the DAG, and constructs a `TapPublishArtifact` carrying the `dag_id`, requested `interface_revision`, and simplified requirements needed to bind or update a skill. The JSON output includes the TAP `package_id`, the `dag_id`, the per-step transaction digests and checkpoints, and the full `artifact`. When `--out` is supplied, the artifact is also written to disk as JSON for `register-skill`, `bind`, or `update-skill`.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap create-skill-artifact --skill-name <NAME> --dag-id <OBJECT_ID> --interface-revision <U64> --payment-mode <user-funded|agent-funded> [--agent-funded-max-budget <AMOUNT>] [--recurrence-kind <KIND>] [--min-interval-ms <MS>] [--max-occurrences <COUNT>] [--allow-recursive] [--fixed-tool <REGISTRY_ID=FQN>] --out <PATH>`**

Creates a skill `TapPublishArtifact` file from explicit skill inputs and a read-only fetch of the published DAG. Use it when a script has already published or otherwise knows the DAG id, but still wants the canonical skill artifact consumed by `tap register-skill`, `tap bind`, and `tap update-skill`. The command derives `requirements.input_schema_commitment` from the fetched DAG input-port data and aborts without writing if the DAG cannot be fetched or decoded. The artifact file and `--json` output contain only the active TAP artifact fields: `skill_name`, `dag_id`, `interface_revision`, and `requirements`.

`--payment-mode agent-funded` requires a positive `--agent-funded-max-budget`; `--payment-mode user-funded` rejects that budget flag. `--fixed-tool` is repeatable and maps directly to `requirements.fixed_tools[]` as `<tool_registry_id>=<tool_fqn>`.

#### Agent setup

---

**`nexus tap create-agent`**

Creates a Talus agent through the configured Agent Registry and shares the agent object. Mutable custody of the `Agent` object is the lifecycle authorization handle. JSON output includes the new `agent_id` and the transaction digest/checkpoint.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap bind --artifact <ARTIFACT_JSON>`**

Composes `tap::create_agent` and `tap::register_skill` into a single PTB, returning the new agent and skill bound together in one transaction. Use this when an agent has not been created yet and the operator wants the "create + register first skill" flow in one round-trip.

The artifact JSON is the one produced by `nexus tap publish-skill` — it carries DAG id, TAP package id, interface revision, and simplified requirements.

The JSON output exposes the transaction digest and checkpoint, the new `agent_id` and `skill_id`, the agent object ref, and structured skill evidence for external bookkeeping.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap agent save --name <NAME> --agent-id <OBJECT_ID>`**

Saves a Talus agent object ID under a local alias in the CLI configuration. Commands that accept `--alias` (e.g. `tap vault balance`, `tap payments list`) use this mapping to resolve agent ids without re-typing them.

---

**`nexus tap agent list`**

Lists locally saved Talus agent aliases.

---

**`nexus tap agent remove --name <NAME>`**

Removes a locally saved Talus agent alias.

#### Skill Registration and Current Revisions

---

**`nexus tap register-skill --artifact <PATH> --agent-id <OBJECT_ID>`**

Registers a TAP skill against an existing agent using the publish artifact. The artifact supplies the DAG id, TAP package id, interface revision, and simplified requirements. JSON output includes the new `skill_id`, the `agent_id`, the DAG and TAP package ids, and the transaction digest/checkpoint.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap update-skill --artifact <PATH> --agent-id <OBJECT_ID> --skill-id <U64>`**

Updates an existing skill from a publish artifact. The current CLI path refreshes the skill DAG/policy contract and reports the resulting `current_interface_revision`; there is no endpoint-revision table or separate announce step in the active TAP model. JSON output includes the `agent_id`, `skill_id`, artifact evidence, `current_interface_revision`, and transaction digest/checkpoint.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

#### Registry and default-agent inspection

---

**`nexus tap registry show`**

Reads the configured Agent Registry and prints its full contents as JSON: a `standard_tap` flag, the registry `id`, the configured `default_executor`, all agent records, and each skill's active flag, DAG binding, current interface revision, requirements, and scheduled-task count. This replaces ad-hoc `sui client object --json` walks of the registry's dynamic fields.

---

**`nexus tap default-agent show`**

Resolves the configured default agent through the registry and prints a flat JSON containing a `standard_tap` flag, the default `agent_id` and `skill_id`, the DAG binding, active interface revision, and published skill requirements. Useful for scripts that want to drive the network's default agent without hard-coding ids.

---

**`nexus tap requirements --agent-id <OBJECT_ID> --skill-id <U64>`**

Fetches the live skill requirements from the TAP registry for a given agent/skill pair: the active skill revision key `(agent_id, skill_id, current_interface_revision)` plus the registered requirements (`input_schema_commitment`, payment policy, schedule policy, and fixed tools). Use this before `tap execute` or `tap schedule` to confirm the active revision and verify the runtime inputs match the on-chain commitments.

#### Vaults and payments

---

**`nexus tap vault balance [--alias <NAME> | --agent-id <OBJECT_ID>]`**

Reads the Talus agent payment vault (a dynamic-object child of the agent object) and reports its current SUI balance. The agent can be supplied either as a saved alias or as an explicit object id; the two flags are mutually exclusive.

---

**`nexus tap vault deposit --amount <AMOUNT> [--alias <NAME> | --agent-id <OBJECT_ID>]`**

Deposits MIST into a Talus agent payment vault by splitting `--amount` MIST from the signer's gas coin and submitting `tap::deposit_agent_payment_vault`. The agent can be supplied either as a saved alias or as an explicit object id; the two flags are mutually exclusive. JSON output includes the agent id, deposited amount, transaction digest, and tx checkpoint.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap payments show --payment-id <OBJECT_ID>`**

Reads a `TapExecutionPayment` object and emits a flat JSON of its fields: payment/execution/agent/skill ids, interface revision, payer, mode/source kind/source identity, locked budget, consumed amount, `accomplished`/`refunded` booleans, raw `final_state`, computed `terminal` flag, and the list of currently-locked vertices. Replaces shell-side BCS decoding of payment object internals.

---

**`nexus tap payments wait --payment-id <OBJECT_ID> [--timeout-secs <SECS>] [--poll-secs <SECS>]`**

Polls the same `TapExecutionPayment` object on a fixed interval until `accomplished`, `refunded`, or any non-`Pending` `final_state` is observed, or until the timeout elapses. Emits the same JSON shape as `payments show` plus `elapsed_ms` and `timed_out` fields. Defaults to a 120-second total timeout and a 2-second poll interval; both are configurable.

Use this in CI pipelines or demos to drive payment settlement instead of hand-rolled retry loops over raw Sui object reads.

---

**`nexus tap payments list [--alias <NAME> | --agent-id <OBJECT_ID>] [--completed | --pending | --all]`**

Lists wallet-owned `ExecutionPaymentReceipt` objects and, when an agent is supplied (by alias or id), the agent-vault payment-receipt history. Filter to completed-only, pending-only, or both. JSON output includes the owner, optional agent id, wallet receipts, vault receipts, and the unresolved/resolved execution-id lists.

---

**`nexus tap payments resolve --execution-id <OBJECT_ID> [--alias <NAME> | --agent-id <OBJECT_ID>]`**

Settles the Talus agent payment linked to a shared `DAGExecution` so it moves to its `Accomplished` final state. Useful when the off-chain leader has not (yet) submitted the settlement transaction itself but the execution has reached a state that the on-chain assertions accept (`assert_execution_can_accomplish_tap_payment` + `assert_matches_tap_payment`).

Two on-chain entrypoints are wrapped depending on the funding source:

- Without `--alias`/`--agent-id`, the SDK builds a one-call PTB targeting `nexus_workflow::dag::accomplish_tap_execution_payment` — the invoker-funded path that settles out of the `TapExecutionPayment` object.
- With `--alias` (resolved against the local agent alias map) or `--agent-id`, the SDK additionally fetches the agent's shared object and routes through `nexus_workflow::dag::accomplish_tap_execution_payment_from_agent_vault` so the payment settles out of the agent's payment vault.

JSON output includes a `function` marker (`accomplish_tap_execution_payment` or `accomplish_tap_execution_payment_from_agent_vault`), the resolved `execution_id`, the resolved `agent_id` (or `null` on the invoker-funded path), and the transaction `digest`/`tx_checkpoint`. Pair with `nexus tap payments show` or `nexus tap payments wait` to confirm the linked `TapExecutionPayment` flipped to `accomplished: true`.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

#### Execution and scheduling

---

**`nexus tap execute --agent-id <OBJECT_ID> --skill-id <U64> --input-json <DATA> [--entry-group <NAME>] [--remote vertex.port,...] [--priority-fee-per-gas-unit <MIST>] [--payment-source-hex <HEX>] [--payment-max-budget <AMOUNT>] [--authorization-plan-hash-hex <HEX>]`**

Executes a Talus agent skill through its current registry skill revision and DAG binding. Input JSON follows the same `{vertex: {port: data}}` shape as `nexus dag execute`. `--remote` forces named ports to be uploaded to the configured remote storage instead of being inlined on-chain. Payment options select the payment source for the Talus agent execution payment:

- `--payment-source-hex` provides typed payment-source bytes (invoker-funded vs agent-vault-funded). Empty defaults to the invoker.
- `--payment-max-budget` caps the Talus agent execution payment.
- `--authorization-plan-hash-hex` optionally supplies an authorization-plan commitment for cap-gated tools.

JSON output includes the new `DAGExecution` object id, the agent and skill ids, the active skill revision key, the submitted authorization plan, and the transaction digest/checkpoint. Pair with `nexus dag inspect-execution`, `nexus tap payments wait`, and (where relevant) `nexus dag execution-cost`.

Cap-gated skills (tools registered with `--workflow-authorization-cap-first`) need a `WorkflowVertexAuthorizationGrant` minted and recorded in the tap package's shared state before the leader dispatches the walk. The CLI does **not** drive that wiring — its shape is skill-specific. Build a single PTB with `sui client ptb` (or the `nexus_sdk::transactions::dag::create_vertex_authorization_grant` builder) that calls `nexus_workflow::dag::create_vertex_authorization_grant`, hands the result to your tap package's bind hook, and only then invokes the workflow's begin / request-walk entrypoints. See the [TAP development guide](guides/1-tap-development.md) for a worked example.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap schedule --agent-id <OBJECT_ID> --skill-id <U64> --long-term-gas-coin-id <OBJECT_ID> [--refill-policy-hex <HEX>] [--schedule-entries-commitment-hex <HEX>] [--recurrence-kind <KIND>] [--min-interval-ms <MS>] [--max-occurrences <COUNT>] [--allow-recursive] [--first-after-ms <MS>]`**

Schedules a Talus agent skill execution by attaching a durable, long-term gas coin to the configured Agent Registry scheduler. The `--recurrence-kind` (default `once`), `--min-interval-ms`, `--max-occurrences` (default `1`), and `--first-after-ms` parameters define the schedule shape; `--refill-policy-hex` and `--schedule-entries-commitment-hex` supply the on-chain policy commitments. JSON output includes the `scheduled_task_id`, agent and skill ids, and the transaction digest/checkpoint.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap schedule-address-funded --scheduler-task-id <OBJECT_ID> --agent-id <OBJECT_ID> --skill-id <U64> --prepay-amount <AMOUNT> --occurrence-budget <AMOUNT> [--refund-recipient <ADDRESS>] [--recurrence-kind <KIND>] [--min-interval-ms <MS>] [--max-occurrences <COUNT>] [--allow-recursive] [--refill-policy-hex <HEX>] [--schedule-entries-commitment-hex <HEX>] [--first-after-ms <MS>]`**

Creates a durable address-funded `ScheduledSkillTask` for a specific agent + skill, attaches it to the existing scheduler task via `TapScheduledTaskLink`, and shares the scheduled TAP task — all in one transaction. `--prepay-amount` MIST are split from the signer's gas coin to prepay the schedule; `--refund-recipient` defaults to the signer. JSON output includes the `scheduled_task_id`, `scheduler_task_id`, agent and skill ids, prepay amount, occurrence budget, and transaction digest/checkpoint.

Replaces hand-rolled scheduler PTBs that combine `agent_registry::schedule_skill_execution_address_funded` with `scheduler::attach_tap_scheduled_task_link` and a `public_share_object` move call.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap schedule-from-vault --scheduler-task-id <OBJECT_ID> --agent-id <OBJECT_ID> --skill-id <U64> --prepay-amount <AMOUNT> --occurrence-budget <AMOUNT> [--recurrence-kind <KIND>] [--min-interval-ms <MS>] [--max-occurrences <COUNT>] [--allow-recursive] [--refill-policy-hex <HEX>] [--schedule-entries-commitment-hex <HEX>] [--first-after-ms <MS>]`**

Creates a durable agent-vault-funded `ScheduledSkillTask` for a specific agent + skill, attaches it to the existing scheduler task, and shares the scheduled TAP task — all in one transaction. `--prepay-amount` MIST are drawn from the agent's payment vault; pair with `nexus tap vault deposit` when the vault needs to be funded first. JSON output mirrors `tap schedule-address-funded` minus `refund_recipient`.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus tap schedule-default-address-funded --scheduler-task-id <OBJECT_ID> --prepay-amount <AMOUNT> --occurrence-budget <AMOUNT> [--refund-recipient <ADDRESS>] [--recurrence-kind <KIND>] [--min-interval-ms <MS>] [--max-occurrences <COUNT>] [--allow-recursive] [--refill-policy-hex <HEX>] [--schedule-entries-commitment-hex <HEX>] [--first-after-ms <MS>]`**

Creates a durable address-funded `ScheduledSkillTask` tied to the registry-owned default agent, attaches it to the existing scheduler task, and shares the scheduled TAP task — all in one transaction. Unlike `tap schedule-address-funded`, no `--agent-id`/`--skill-id` flags are required: the configured default agent is used. JSON output mirrors `tap schedule-address-funded`.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

### `nexus conf`

Manage the Nexus CLI configuration stored at `~/.nexus/conf.toml`. The CLI reads the configured Sui RPC URL, the Sui private key, the Nexus deployment objects (package IDs and shared registry/service/gas-service/leader-registry refs), and optional data-storage settings from this file.

---

**`nexus conf get`**

Print the current Nexus CLI configuration. JSON mode emits the full configuration as a JSON document; otherwise a human-readable summary is printed.

---

**`nexus conf set [--sui.pk <BASE64>] [--sui.rpc-url <URL>] [--nexus.objects <PATH>] [--data-storage.walrus-aggregator-url <URL>] [--data-storage.walrus-publisher-url <URL>] [--data-storage.walrus-save-for-epochs <EPOCHS>] [--data-storage.preferred-remote-storage <KIND>] [--data-storage.testnet]`**

Update the Nexus CLI configuration. Each flag updates the corresponding setting in `~/.nexus/conf.toml`; only the flags supplied are modified.

- `--sui.pk` sets the Sui private key as base64-encoded bytes (matches the `base64WithFlag` format from `sui keytool convert`).
- `--sui.rpc-url` sets the Sui node RPC URL the CLI talks to.
- `--nexus.objects <PATH>` loads the Nexus package ids and shared object refs from a TOML file (as produced by `publish.sh`). This replaces the `[nexus.*]` sections wholesale.
- `--data-storage.walrus-aggregator-url` / `--data-storage.walrus-publisher-url` configure the Walrus endpoints used for remote DAG input storage.
- `--data-storage.walrus-save-for-epochs` sets how many epochs uploaded data is preserved for.
- `--data-storage.preferred-remote-storage` chooses the default remote storage backend (currently `walrus`).
- `--data-storage.testnet` is a preset that fills in the data-storage block for Sui testnet defaults and overrides any conflicting flags.

---

### `nexus completion`

**`nexus completion <SHELL>`**

Prints shell completion scripts to stdout. Supported shells: `bash`, `elvish`, `fish`, `powershell`, `zsh`. Source the output into your shell's completion directory or `eval` it directly.

<!-- List of References -->

[nexus-cli-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/cli
