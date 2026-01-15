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

**`nexus tool new <name> --template <template>`**

Create a new Tool scaffolding in a folder called `<name>`. Which files are generated is determined by the `--template` flag. I propose having `templates/tools/<template>.template` files that contain the Tool skeleton files. For example for `rust` it'd be a `Cargo.toml` with the `nexus-toolkit` dependency, and a `src/main.rs` file that shows a basic use case of the crate.

---

**`nexus tool validate --off-chain <url>`**

Validate an off-chain Nexus Tool on the provided URL. This command checks whether the URL hosts a valid Nexus Tool interface:

1. `GET /meta` contains Tool metadata that is later stored in our Tool Registry, this contains the `fqn`, the `url` which should match the one in the command and the Tool input and output schemas. Output schema is also validated to contain a top-level `oneOf` to adhere to Nexus output variant concept.
1. `GET /health` simple health check endpoint that needs to return a `200 OK` in order for the validation to pass.
1. `POST /invoke` the CLI can check that the endpoint exists.

{% hint style="success" %}
As an improvement, the command could take a `[data]` parameter that invokes the Tool and checks the response against the output schema.
{% endhint %}

This command should also check that the URL is accessible by the Leader node. It should, however, be usable with `localhost` Tools for development purposes, printing a warning.

---

**`nexus tool validate --on-chain <ident>`**

{% hint style="warning" %}
The specific design for onchain tools is still in progress and as a result the implementation is not yet present. When running the command, it will panic.
{% endhint %}

---

**`nexus tool register --off-chain <url> --invocation-cost [mist] --collateral-coin [object_id] [--batch] [--no-save]`**

Command that makes a request to `GET <url>/meta` to fetch the Tool definition and then submits a TX to our Tool Registry. It also locks the collateral and sets the single invocation cost of the Tool which defaults to 0 MIST.

This returns 2 OwnerCap object IDs that can be used to manage the Tool and its Gas settlement methods.

If the `--batch` flag is passed, the command accepts a URL of a webserver hosting multiple tools and register all of them at once. `nexus-toolkit` automatically generates a `GET /tools` endpoint that returns a list of URLs of all tools registered on that server. The CLI will then iterate over the list and register each tool.

Upon successful registration, both OwnerCap object IDs are saved to the CLI configuration file and automatically used for subsequent commands. This happens unless the `--no-save` flag is passed, in which case the OwnerCaps are not saved.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

{% hint style="info" %}
Tool registration is currently restricted during the beta phase. To register your tool, please contact the team to be added to the allow list.
{% endhint %}

---

**`nexus tool register --on-chain <ident>`**

{% hint style="warning" %}
The specific design for onchain tools is still in progress and as a result the implementation is not yet present. When running the command, it will panic.
{% endhint %}

---

**`nexus tool unregister --tool-fqn <fqn> --owner-cap [object_id]`**

Command that sends a TX to our Tool Registry and unregisters a Tool with the provided `<fqn>`. This command requires confirmation as unregistering a Tool will render all DAGs using it unusable.

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

**`nexus tool list`**

List all Nexus Tools available in the Tool Registry. This reads the dynamic object directly from Sui.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

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

**`nexus dag execute --dag-id <id> --input-json <data> --entry-group [group] --remote [field1,field2,...] [--priority-fee-per-gas-unit <mist>] [--inspect]`**

Execute a DAG with the provided `<id>`. This command also accepts an entry `<group>` of vertices to be invoked. Find out more about entry groups in [[Package: Workflow]]. Entry `<group>` defaults to a starndardized `_default_group` string.

The input `<data>` is a JSON string with the following structure:

- The top-level object keys refer to the _entry vertex names_
- Each top-level value is an object and its keys refer to the _input port names_ of each vertex (this object can be empty if the vertex has no input ports)
- Values of the second-level object are the data that should be passed to each input port

Data for encrypted ports are automatically encrypted before being sent on-chain.

The `--inspect` argument automatically triggers `nexus dag inspect-execution` upon submitting the execution transaction.

The `--remote` argument accepts a list of `{vertex}.{port}` strings that refer to entry ports and their vertices. The data associated with these ports is stored remotely based on the configured preferred remote storage provider. Note that it is required that the user configure these remote storage providers via the `$ nexus conf set --help` command.

Supported remote storage providers are:

- Walrus

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus dag inspect-execution --dag-execution-id <id> --execution-digest <digest>`**

Inspects a DAG execution process based on the provided `DAGExecution` object ID and the transaction digest from submitting the execution transaction.

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

**`nexus scheduler task create --dag-id <id> [--entry-group <group>] [--input-json <json>] [--remote vertex.port,...] [--metadata key=value ...] [--execution-priority-fee-per-gas-unit <mist>] [--schedule-start-ms <ms> | --schedule-start-offset-ms <ms>] [--schedule-deadline-offset-ms <ms>] [--schedule-priority-fee-per-gas-unit <mist>] [--generator queue|periodic]`**

Creates a new scheduler task tied to the specified DAG. Key options:

- `--entry-group` points to the DAG entry function and defaults to `_default_group`.
- `--input-json` provides inline input data; `--remote vertex.port,...` forces specific inputs to be uploaded to the configured remote storage instead of inlining them on-chain.
- `--metadata key=value` attaches arbitrary metadata entries and replaces any existing entries if the command is re-run.
- `--execution-priority-fee-per-gas-unit` sets the priority fee for future DAG executions launched by the task.
- `--schedule-start-ms` supplies an absolute first-occurrence timestamp (milliseconds since epoch) while `--schedule-start-offset-ms` uses the current Sui clock as the base; the two switches are mutually exclusive.
- `--schedule-deadline-offset-ms` sets the completion window relative to whichever start time was selected, and `--schedule-priority-fee-per-gas-unit` sets the priority fee for that initial occurrence.
- `--generator` chooses the generator responsible for future occurrences (`queue` by default, `periodic` to enable recurring schedules).

Initial schedule arguments (`--schedule-*`) are only valid for queue-based tasks. Selecting `--generator periodic` prepares the task for periodic execution, but you must configure the recurring schedule separately via `nexus scheduler periodic set`.

Data for encrypted entry ports is automatically encrypted when a session is available.

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

### `nexus crypto`

Set of commands for managing the CLI’s encrypted secrets (master key, passphrase, identity key) and establishing secure sessions that power DAG data encryption.

---

**`nexus crypto auth [--sui-gas-coin <object_id>] [--sui-gas-budget <mist>]`**

Runs the two-step handshake with the Nexus network to claim a pre-key, perform X3DH with your local identity key, and store a fresh Double Ratchet session on disk. The claimed pre-key bundle is what enables the CLI to complete a Signal-style secure session with the network: X3DH bootstraps shared secrets, and the Double Ratchet derived from that bundle encrypts every DAG payload going forward. The command returns both claim/associate transaction digests and prints the initial message in JSON format, enabling you to audit the handshake.

Before sending the associate transaction, the CLI automatically generates an identity key if one is missing and persists the session in `~/.nexus/crypto.toml`. All subsequent `nexus dag` commands load that session to encrypt entry-port payloads or decrypt remote results, so run `auth` whenever you rotate keys or see “No active sessions found.”

{% hint style="info" %}
This command requires that a wallet is connected to the CLI and spends gas for **two** programmable transactions. Use `--sui-gas-coin` / `--sui-gas-budget` if you need explicit control.
{% endhint %}

---

**`nexus crypto generate-identity-key`**

Creates a brand-new long-term identity key and stores it (encrypted) inside `~/.nexus/crypto.toml`. Because peers can no longer trust sessions tied to the previous identity, the CLI makes it clear that all stored sessions become invalid. Run `nexus crypto auth` immediately after to populate a replacement session.

---

**`nexus crypto init-key [--force]`**

Generates a random 32‑byte master key with [`OsRng`](https://docs.rs/rand/latest/rand/rngs/struct.OsRng.html) and writes it to the OS keyring under the `nexus-cli-store/master-key` entry. The master key controls access to every encrypted field (`Secret<T>`) in the CLI configuration. Rotating it without also wiping the encrypted data would leave the ciphertext inaccessible, so this command automatically truncates the cryptographic configuration after a successful write.

Use `--force` to overwrite an existing raw key or stored passphrase; doing so deletes all saved sessions and identity material because it can no longer be decrypted.

---

**`nexus crypto set-passphrase [--stdin] [--force]`**

Stores a user-provided passphrase in the OS keyring (`nexus-cli-store/passphrase`) and derives the same 32‑byte master key via Argon2id whenever secrets need to be decrypted. By default the command prompts interactively; `--stdin` allows piping from scripts or CI.

Like `init-key`, it refuses to overwrite an existing persistent key unless `--force`. Empty or whitespace-only passphrases are rejected to avoid unusable configs.

---

**`nexus crypto key-status`**

Reports where the current master key will be loaded from, following the same priority order as the runtime resolver: `NEXUS_CLI_STORE_PASSPHRASE` environment variable, keyring passphrase entry, or raw key entry. If a raw key is in use the CLI prints the first 8 hex characters so you can distinguish multiple installations; otherwise it notes the source or that no persistent key exists yet.

---

### `nexus gas`

Set of commands to manage Nexus gas budgets and tickets.

---

**`nexus gas add-budget --coin <object_id>`**

Upload the coin object to the Nexus gas service as budget in the "invoker address" scope. That means that if a DAG execution is started from the address that the coin was uploaded from, the coin can be used to pay for the gas.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus gas expiry enable --tool-fqn <fqn> --owner-cap [object_id] --cost-per-minute <mist>`**

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

### `nexus network`

Set of commands for managing Nexus networks.

---

**`nexus network create --addresses [addresses] --count-leader-caps [count-leader-caps]`**

Create a new Nexus network and assign `count-leader-caps` (default: 5) leader caps to the TX sender and the addresses listed in `addresses` (default: []).

The network object ID is returned.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

### `nexus completion`

Provides completion for some well-known shells.

<!-- List of References -->

[nexus-cli-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/cli
