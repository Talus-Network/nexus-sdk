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

**`nexus dag execute --dag-id <id> --input-json <data> --entry-group [group] [--inspect]`**

Execute a DAG with the provided `<id>`. This command also accepts an entry `<group>` of vertices to be invoked. Find out more about entry groups in [[Package: Workflow]]. Entry `<group>` defaults to a starndardized `_default_group` string.

The input `<data>` is a JSON string with the following structure:

- The top-level object keys refer to the _entry vertex names_
- Each top-level value is an object and its keys refer to the _input port names_ of each vertex (this object can be empty if the vertex has no input ports)
- Values of the second-level object are the data that should be passed to each input port

Data for encrypted ports are automatically encrypted before being sent on-chain.

The `--inspect` argument automatically triggers `nexus dag inspect-execution` upon submitting the execution transaction.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI...
{% endhint %}

---

**`nexus dag inspect-execution --dag-execution-id <id> --execution-digest <digest>`**

Inspects a DAG execution process based on the provided `DAGExecution` object ID and the transaction digest from submitting the execution transaction.

---

### `nexus scheduler`

Manage scheduler tasks, one-off occurrences, and periodic schedules.

---

**`nexus scheduler task create --dag-id <id> [--entry-group <group>] [--input-json <json>] [--metadata key=value ...] [--schedule-*]`**

Creates a new scheduler task tied to the specified DAG. You can optionally attach metadata, supply initial DAG inputs, and immediately schedule the first occurrence. When `--schedule-start-offset-ms` is used, the optional `--schedule-deadline-offset-ms` is interpreted as milliseconds after that start time (e.g. `--schedule-start-offset-ms 60000 --schedule-deadline-offset-ms 30000` opens a 30 s window beginning in one minute).

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

**`nexus scheduler occurrence add --task-id <id> [--start-ms <ms> | --start-offset-ms <ms>] [--deadline-ms <ms> | --deadline-offset-ms <ms>] [--gas-price <mist>]`**

Schedules a one-off occurrence for the task. Absolute timestamps are interpreted as milliseconds since epoch. Offsets are measured from the current Sui clock, and deadline offsets are expressed relative to the chosen start time.

{% hint style="info" %}
This command requires that a wallet is connected to the CLI and holds sufficient SUI for gas.
{% endhint %}

---

**`nexus scheduler periodic set --task-id <id> --period-ms <ms> [--deadline-offset-ms <ms>] [--max-iterations <count>] [--gas-price <mist>]`**

Configures or updates a periodic schedule for the task. The deadline offset is applied after each generated start time, and `max-iterations` limits how many future occurrences can be produced automatically.

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
