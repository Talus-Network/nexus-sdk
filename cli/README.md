# nexus-cli

The **Nexus CLI** provides easy-to-use command-line tools to manage and interact with Nexus, the Agentic Workflow Engine.

## Installation

You can install Nexus CLI using several convenient methods:

### Using Homebrew (macOS/Linux)

```sh
brew tap talus-network/tap
brew install nexus-cli
```

### Arch Linux

The [nexus-cli](https://aur.archlinux.org/packages/nexus-cli) is also available in the AUR (Arch User Repository). You can install it using your preferred [AUR helper](https://wiki.archlinux.org/title/AUR_helpers):

```bash
yay -S nexus-cli
```

### Using cargo-binstall (recommended for faster binaries)

If you prefer quicker binary installation, use [cargo-binstall]:

```bash
cargo binstall --git https://github.com/talus-network/nexus-sdk nexus-cli
```

### Using Cargo

To install directly from the source using `cargo`, run:

```bash
cargo install nexus-cli \
  --git https://github.com/talus-network/nexus-sdk \
  --tag v2.1.0 \
  --locked
```

## Usage

Run the `nexus` command to see all the available commands and options:

```console
$ nexus help
Nexus CLI

Usage: nexus [OPTIONS] <COMMAND>

Commands:
  tool        Manage Nexus Tools
  conf        Manage Nexus Configuration
  dag         Validate and publish Nexus DAGs
  task        Create and operate scheduled Tasks
  gas         Manage Nexus gas budgets and tickets
  tap         Prepare, test and operate TAP applications
  completion  Provide shell completions
  help        Print this message or the help of the given subcommand(s)

Options:
      --json        Emit machine readable JSON
  -v, --verbose...  More output per occurrence
  -q, --quiet...    Less output per occurrence
  -h, --help        Print help
  -V, --version     Print version

```

Scheduled work follows `Task -> Schedule -> Occurrence`.

`nexus task create` creates an empty Task for later composition.
`nexus task schedule` creates a Task and applies a complete nonempty Schedule
atomically. Task and occurrence inspection read durable object state. Run
`nexus task occurrence list --task-id <OBJECT_ID> --json` to page through
retained occurrence records, and `nexus task --help` for complete examples.

For expired work, `nexus task occurrence abort-expired --task-id <OBJECT_ID> --occurrence-id <U64>` discovers eligible invocations automatically. It settles available results, refunds expired payment locks, and settles the occurrence when execution finishes. The output reports the resulting occurrence state and each confirmed resolution; work still awaiting its timeout remains pending. Supplying `--invocation-id` limits the command to that invocation. `nexus task occurrence cost` includes outstanding invocation IDs in both human and JSON output.

For an Agent skill whose onchain Tool requires workflow authorization, bind
each vertex that requires a grant to its recipient when creating the Task:

```sh
nexus task schedule \
  --agent-id 0xAGENT --skill-id 0 \
  --authorization-binding check_message=0xRECIPIENT \
  --input-json "$INPUT_JSON" \
  --prepay-amount-mist 500000000 \
  --occurrence-budget-mist 500000000 \
  --now
```

Repeat `--authorization-binding` when more than one vertex requires a grant.
The CLI rejects malformed or duplicate vertex bindings before submitting a
transaction.

## TAP unit tests

Run a TAP Move suite against the Nexus bytecode published on Sui without a
wallet, private key, gas, or local Nexus implementation source:

```sh
nexus tap test --path tap
```

The command resolves MVR dependencies for Testnet by default, fetches the exact
published Nexus modules, adds developer `#[test_only]` extension functions in
memory, verifies the resulting modules, and runs the TAP tests in a local Sui
VM. Existing Nexus functions keep their published definitions.

The command works for both TAP forms. `nexus tap scaffold` creates a standard
CLI managed skill with JSON DAG and skill files. The canonical
[embedded application example] instead owns its Agent in Move state, builds
the DAG in Move and exposes an onchain Tool callback.

Use focused commands while developing:

```sh
nexus tap test --path tap --list
nexus tap test --path tap execute_accepts
nexus tap test --path tap --threads 1
nexus tap test --path tap --build-env mainnet
```

Local tests cover TAP behavior and reachable Nexus behavior with state that
the suite constructs. Use Testnet for live shared objects, package
publication, transaction effects, gas, network routing, and external Tools.

The canonical [TAP development and testing guide] documents both TAP forms,
focused fixtures, unit test limits and a complete embedded application. The
application owns its Agent, creates its DAG and skill, schedules a Task,
authenticates the onchain Tool callback and finalizes a Nexus result before the
Testnet integration path begins.

## Walrus

Walrus uses the same wallet and Sui RPC as Nexus. Configure the wallet with the existing Sui commands. Fund that address with SUI for gas and WAL for storage on the selected network. No publisher URL or separate wallet is required.

```sh
nexus walrus status
nexus walrus configure --epochs 5
nexus walrus upload input.json --estimate
# input.json contains the data to upload; walrus_reference.json stores the blob ID, hash, owner and expiry.
nexus walrus upload input.json --out walrus_reference.json
nexus walrus inspect walrus_reference.json
nexus walrus download walrus_reference.json --out downloaded.json
nexus walrus list
nexus walrus extend walrus_reference.json --epochs 2 --max-storage-cost-frost 100000000
```

`upload` treats a JSON document as one value. Use `--many` to upload each array item as a separate value. Data is permanent until expiry by default; `--deletable` permits the owner to delete it with `nexus walrus delete REFERENCE --yes`. Each upload is limited to 8 MiB of execution data under the shared SDK [execution limits](../sdk/README.md#execution-limits). The combined inputs of each invocation must also fit that budget. `--max-storage-cost-frost` caps WAL cost per blob, and `--storage-gas-budget` caps SUI gas per transaction. Without an explicit WAL limit, each transaction is capped at its current quote.

The network follows the active Sui RPC. Standard aggregators follow network changes automatically. To select a custom reader, use `nexus walrus configure --aggregator URL --network testnet` (or `mainnet`). `--reset-aggregator` restores the network default. Inspect and download need no signing key. Old publisher configuration is ignored and removed when the configuration is next saved.

Use the same input options for task creation and scheduling:

```sh
nexus task create --dag-id DAG_ID --prepay-amount-mist 100000000   --occurrence-budget-mist 10000000   --input-file inputs.json --remote analyze.document --remote-receipts ./references

nexus task create --dag-id DAG_ID --prepay-amount-mist 100000000   --occurrence-budget-mist 10000000   --input-ref analyze.document=walrus_reference.json
```

`--input-file` reads the same vertex and port object accepted by `--input-json`. Repeat `--input-ref VERTEX.PORT=FILE` to reuse references without buying storage. Duplicate or conflicting input sources are rejected. Local validation and preflight against the published DAG complete before any new storage payment.

Uploads save signed registrations and progress beside the destination reference in a `.uploads` directory. If a standalone upload is interrupted, run the same command and options with `--resume`. It reuses the paid registration. For task uploads, reuse the same `--remote-receipts` directory to resume matching work. References remain available if task submission fails. Successful task JSON output includes their paths in `walrus_references`. Keep these files until completion; an uncertain submission must be reconciled using its original transaction, since registering again may buy duplicate storage.

Scheduled tasks do not renew storage automatically. Choose a retention period covering their occurrences, and extend the owned Blob objects before expiry. Large resolved inputs are supported for HTTP tools; Sui tools retain their transaction size limits.

For more detailed instructions, visit the [Nexus CLI documentation][nexus-cli-docs].

<!-- List of references -->

[nexus-cli-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/cli
[embedded application example]: https://github.com/Talus-Network/nexus-move-packages/tree/main/examples/local_testing
[TAP development and testing guide]: https://github.com/Talus-Network/nexus-move-packages/blob/main/docs/tap_development.md
