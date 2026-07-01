# Working on `nexus-sdk` — Agent context

This file documents the conventions and workflow expected of any AI agent
contributing to `nexus-sdk`. Read it once before touching code; refer back to
it when a pattern is unclear.

## Workspace layout

```text
nexus-sdk/                  cargo workspace root
├── sdk/                    `nexus-sdk` crate — Rust SDK
│   └── src/
│       ├── nexus/          high-level action types (TapActions, ToolActions,
│       │                   WorkflowActions, SchedulerActions, GasActions,
│       │                   NetworkAuthActions) + NexusClient, Crawler,
│       │                   Signer, gas pool, EventPoller, errors
│       ├── transactions/   PTB builders: tap.rs, tool.rs, workflow.rs,
│       │                   scheduler.rs, gas.rs, network_auth.rs
│       ├── types/          typed on-chain object/event models
│       │                   (TapRegistry, TapExecutionPayment, Tool, ToolRef,
│       │                   DagExecution, NexusObjects, derive helpers, …)
│       │                   plus Move-JSON / BCS serde parsers
│       ├── idents/         Move package/module/function/struct identifiers
│       │                   (primitives, sui_framework, workflow, tap, move_std)
│       ├── events/         event parsing
│       ├── test_utils/     sui_mocks (gRPC mocks), nexus_mocks (client mocks),
│       │                   containers (sui/redis testcontainers), faucet
│       ├── onchain_schema_gen/  Move-introspection schema generation
│       ├── signed_http/    application-layer Ed25519 signatures (feature)
│       ├── walrus/         Walrus storage client
│       ├── tool_fqn.rs     ToolFqn + `fqn!` macro
│       └── lib.rs          top-level re-exports
├── cli/                    `nexus-cli` crate — `nexus` binary
│   └── src/
│       ├── main.rs         top-level Cli + Command dispatch
│       ├── prelude.rs      common imports (GasArgs, JSON_MODE, sui)
│       ├── display.rs      command_title!, notify_success!, item!, loading!,
│       │                   json_output(), JSON_MODE: AtomicBool
│       ├── sui.rs          get_nexus_client(), gRPC client helpers
│       ├── cli_conf.rs     CliConf (~/.nexus/conf.toml)
│       └── {tool,conf,dag,scheduler,gas,tap,completion}/mod.rs
│                           subcommand groups with their handlers
├── toolkit-rust/           Rust toolkit (`nexus-toolkit`) for tool authors
├── helpers/                workspace helper crates / just recipes
├── docs/                   gitbook-synced docs (cli.md is the CLI reference)
├── target/                 cargo build output
├── Cargo.toml              workspace manifest
├── rustfmt.toml            unstable nightly-only options used
├── .nightly-version        pinned nightly toolchain for fmt
├── rust-toolchain.toml     stable toolchain for everything else
├── STYLE_GUIDE.md          markdown style rules (markdownlint-enforced)
├── CONTRIBUTING.md         contributor guide (pre-commit, commits, PRs)
└── CHANGELOG.md            keep-a-changelog, per-crate sections
```

Sibling repos checked out next to this one (paths depend on local layout):

- `nexus-next/` — on-chain Move packages (`sui/primitives`, `sui/interface`,
  `sui/registry`, `sui/workflow`), example TAPs (`sui/examples/demo_tap`),
  and the off-chain leader (`be/leader/`). Its `sui/bin/publish.sh` and
  `sui/bin/test_demo.sh` are the canonical localnet bring-up and demo driver.
- `nexus-tools/`, `nexus-workbench/`, `nexus-api/` — sibling repos consumed
  by docker-compose workbenches.

## SDK conventions

- **High-level actions** live in `sdk/src/nexus/<area>.rs`. Each action takes
  `&self` on the `*Actions` struct (held by `NexusClient`), submits via the
  shared signer/gas/crawler, and returns a typed `*Result` struct. Free-
  function `fetch_*` helpers (e.g. `fetch_registry`) live in the same
  file when they're useful without a full client.
- **PTB builders** live in `sdk/src/transactions/<area>.rs` and take
  `&mut TransactionBuilder` plus `&NexusObjects`. They never read from the
  network; they only emit move calls/inputs. Pair an action with one PTB
  builder per logical transaction.
- **Errors** flow through `NexusError` (`Wallet`, `Configuration`,
  `TransactionBuilding`, `Rpc`, `Parsing`, `Timeout`, `Channel`, `Storage`).
  Pick the variant that matches the root cause; only `Configuration` is
  string-typed.
- **On-chain decoding**: prefer `crawler.get_object::<T>` (JSON) for SDK
  types that implement `Deserialize` against Sui's Move-JSON, and
  `crawler.get_object_contents_bcs::<T>` for objects whose layout is best
  decoded from raw BCS. Dynamic field readers
  (`get_dynamic_fields_bcs`, `get_dynamic_object_fields`) sit on top of both.
- **ID derivation** uses `derive_tool_id`, `derive_tool_gas_id`,
  `derive_walk_execution_event_task_id`, etc. Never reimplement the
  ascii-string / BCS-blake2b derivation in shell or Python.
- **Struct reuse comes before new structs.** Before adding any new Rust struct, inspect the existing structs in the related SDK/CLI/type module and confirm that none of them, and no reasonable modification of them, can satisfy the new purpose. If a new struct is still necessary, add a short doc comment or nearby comment that states exactly what is missing from the closest existing struct and why modifying that existing struct would be wrong.

## Move identifier bindings (`idents`)

The `ModuleAndNameIdent` constants in `sdk/src/idents/<package>.rs` are **not
hand-written** — they are generated from on-chain Move package metadata using
the sibling `move-binding` crate (`sui-move-codegen`). This gives a single,
regeneratable source of truth for module/function/datatype names and catches
drift: if a Move function is renamed or removed on-chain, regeneration drops the
constant and the call site fails to compile.

How the pipeline fits together:

- **Network half (manual, on demand)**: `sdk/src/bin/generate_binding.rs` (gated
  behind the `binding_codegen` feature) fetches each package's normalized IR
  (intermediate representation — `NormalizedPackage`) over gRPC via
  `sui_move_codegen::fetch_package` and writes it as committed JSON under
  `sdk/src/idents/generated/ir/<package>.json`. One file per package:
  `primitives`, `interface`, `registry`, `workflow`, `scheduler`, plus the
  framework packages `move_std` (`0x1`) and `sui_framework` (`0x2`).
- **Offline half (every build)**: `sdk/build.rs` reads the committed IR and
  renders one `$OUT_DIR/idents_<package>.rs` per file — a `pub struct` per Move
  module with a SCREAMING_SNAKE `ModuleAndNameIdent` const per function and
  datatype. No network access; the rendered `.rs` is never committed.
- **Wiring**: each `sdk/src/idents/<package>.rs` `include!`s its generated file
  and adds the hand-written `TypeTag`/argument helpers (`vertex_from_str`,
  `into_type_tag`, enum mappers, etc.) on top. Module-to-file mapping is by
  package: `tap.rs` includes the `interface` package (its generated structs are
  `Agent`, `Authorization`, `Payment`, `Verifier`, `Version`, …); `move_std.rs`
  and `sui_framework.rs` keep the fixed framework addresses (`PACKAGE_ID`,
  `CLOCK_OBJECT_ID`).

Key invariants:

- **Generated constants are address-free** (module + name only). The deployed
  package id is supplied at call time from the runtime-injected `NexusObjects`,
  so the same constant works across localnet/testnet/mainnet. Never bake a
  package address into the generated output.
- **Generated structs follow the real on-chain layout**, which does not always
  match the old hand-written grouping. For example `tool_registry` lives in the
  `registry` package (`registry::ToolRegistry`, not `workflow`), and the
  verifier identifiers live in the `interface` package
  (`tap::Verifier`, not `workflow::Dag`). `tap::TapStandard` is the one
  deliberately hand-maintained facade — it is a runtime-resolved view whose
  constants span the interface and registry packages, so it cannot map onto a
  single generated struct.
- The constants only carry names; **correctness of which package a call targets
  lives in the PTB builder**, which passes the right `*_pkg_id` from
  `NexusObjects`.

### Step-by-step: regenerating the bindings

Run this after the on-chain Move in `nexus-next` changes (renamed/added/removed
functions or types). It republishes the contracts to a fresh localnet and
refetches the IR.

1. **Match the Sui toolchain to `nexus-next`.** The Move sources are built and
   published with the host `sui` client, so its version must satisfy
   `nexus-next`'s framework requirement (otherwise the build fails, e.g. with
   `Unbound function 'exists' in module 'sui::dynamic_field'`). Install/select a
   matching toolchain with `suiup` (the testcontainer used by tests pins
   `testnet-v1.73.1`):

   ```bash
   suiup install sui@testnet-1.73.1
   suiup default set sui@testnet-1.73.1
   sui --version
   ```

1. **Start a fresh localnet and fund the active address.**

   ```bash
   sui start --with-faucet --force-regenesis   # leave running in another shell
   sui client switch --env localnet
   sui client faucet
   ```

1. **Publish the Nexus packages** with the `nexus-next` script. It writes the
   package ids to `nexus-next/sui/bin/target/objects.localnet.toml`.

   ```bash
   cd ../nexus-next/sui
   NEXUS_PUBLISH_OVERWRITE=1 SUI_ENV=localnet ./bin/publish.sh publish
   ```

1. **Regenerate the committed IR.** The simplest path is the wrapper recipe,
   which collects the package ids from the objects TOML, adds the `0x1`/`0x2`
   framework packages, and runs the generator binary:

   ```bash
   just sdk rebind
   ```

   Or invoke the binary directly:

   ```bash
   NEXUS_BINDING_GRPC_URL=http://127.0.0.1:9000 \
   NEXUS_BINDING_PACKAGES="primitives=0x..,interface=0x..,registry=0x..,workflow=0x..,scheduler=0x..,move_std=0x1,sui_framework=0x2" \
     cargo run -p nexus-sdk --features binding_codegen --bin generate_binding
   ```

1. **Rebuild and review.** `build.rs` re-renders the constants automatically.
   Review the diff under `sdk/src/idents/generated/ir/` and fix any call sites
   the compiler flags (a dropped or renamed constant means the on-chain API
   moved):

   ```bash
   cargo +stable check --all-features -p nexus-sdk
   ```

1. **Tear down** the localnet (Ctrl-C the `sui start` shell) and run the usual
   verification (`just pre-commit cargo-check`, `cargo-clippy`,
   `cargo-nightly-fmt`).

## CLI conventions

- **Module layout per command group** (`cli/src/<group>/mod.rs`):
  1. `mod tap_xxx;` for each subcommand handler file.
  1. `use { … }` block bringing handler functions and SDK result types into
     the module scope.
  1. `#[derive(Subcommand)] enum <Group>Command { … }` with one variant per
     subcommand; each `#[command(about = …)]` annotation feeds `--help`.
  1. Optional nested `enum <Sub>Command { … }` for two-level groupings (e.g.
     `EndpointCommand`, `PaymentsCommand`).
  1. `pub(crate) async fn handle(command: <Group>Command) -> AnyResult<(), NexusCliError>`
     dispatcher that destructures each variant into a flat handler call.
- **Per-subcommand handler** (`cli/src/<group>/<group>_<command>.rs`):
  1. `command_title!("…")` for human progress.
  1. `let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;`
     (omit gas args for read-only commands).
  1. Drive the SDK action and unwrap the result.
  1. `notify_success!` for human feedback.
  1. `json_output(&<group>_<command>_result_json(&result))?;` — JSON output
     is stable and consumed by scripts; keep keys flat and snake_case.
- **JSON-shape helpers** (`<group>_<command>_result_json`) live next to
  their handler. They take typed SDK results and emit `serde_json::Value`
  with the keys the CLI documents. Always cover them with a unit test that
  asserts each top-level key — this catches accidental field renames.
- **The global `--json` flag** is stored in `JSON_MODE: AtomicBool` (see
  `cli/src/prelude.rs`). Interactive prompts and progress spinners must
  check `JSON_MODE.load(Ordering::Relaxed)` and short-circuit when set.

## Testing patterns

- **Unit tests** sit next to their implementation in `#[cfg(test)] mod tests`.
  - Bring the parent module in with `use super::*;` and supplement with
    `crate::{fqn, sui::traits::*, test_utils::{nexus_mocks, sui_mocks}}`
    plus `tonic::Status` when you need to fake gRPC errors.
  - Use `sui_mocks::grpc::MockLedgerService`, `MockTransactionExecutionService`,
    `MockSubscriptionService` to expect gRPC calls.
  - For full transaction flows use
    `mock_execute_transaction_and_wait_for_checkpoint` and pass the response
    objects your handler depends on.
  - Use `sui_mocks::grpc::mock_server` + `nexus_mocks::mock_nexus_client` to
    materialise an end-to-end client against the mocked server.
  - `mock_get_object_metadata` / `mock_get_object_json` /
    `mock_get_object_bcs_for` cover the three read paths; return a tonic
    `Status::not_found` directly for "object missing" cases.
- **CLI dispatch tests** in `cli/src/tap/mod.rs::tests` and similar verify
  every clap variant reaches a local boundary (e.g. missing-RPC error)
  before any network call. Add a new arm whenever you add a subcommand.
- **JSON-shape tests** for `*_result_json` helpers assert each documented
  top-level key with `assert_eq!(json["x"], …)`.

## Comment patterns

- Avoid unnecessary and extraneous comments around self explanatory code. Code
  should be written in such a way that it doesn't require a sea of comments in
  the first place.
- Add `//!` brief module descriptions to each module. Highlighting what the
  responsibility and purpose of that module is. Update this comment after any
  changes made to each module.
- Prefer doc-comments `///` to inline comments `//` where possible
- Only use inline comments to clarify potentially confusing logic within
  functions. These comments should be concise (1-2 lines maximum).
- `///` doc comments can be more verbose if the struct or the function require it.

## Step-by-step: adding a new feature

1. **Add the SDK primitive** under `sdk/src/nexus/<area>.rs`:
   - Define the params and result structs near the existing ones.
   - Add the `impl <Area>Actions { pub async fn … }` method.
   - If a new PTB shape is required, extend `sdk/src/transactions/<area>.rs`
     with a builder that takes `&mut TransactionBuilder` and the relevant
     `NexusObjects` refs/idents.
   - Reuse existing typed models (`sdk/src/types/<area>.rs`) and identifier
     constants (`sdk/src/idents/<package>.rs`) — only add new types when an
     on-chain shape genuinely changed.
1. **Wire the SDK helper into the CLI**:
   - Create `cli/src/<group>/<group>_<command>.rs` with a handler that
     calls the SDK and a `*_result_json` helper.
   - Add a `mod` line and import in `cli/src/<group>/mod.rs`.
   - Add a `#[derive(Subcommand)]` variant (or a nested `enum`) with clap
     attributes that match the SDK's parameter types.
   - Route the variant in `handle(command: <Group>Command)`.
1. **Add tests**:
   - SDK: at minimum one happy-path mock test and one failure-mode test
     per branch in the new function. Cover the gRPC error path with
     `Status::not_found` and the parse-failure path with a constructed bad
     response.
   - CLI: dispatch test that the new variant reaches the missing-RPC
     boundary, plus a JSON-shape unit test for each new `*_result_json`.
1. **Update documentation**:
   - `docs/cli.md` — add a command block in the existing `### nexus <group>`
     section. Follow the established `**\`nexus … [--flag <VALUE>]\`\*\*`header
style and the`{% hint style="info" %}…{% endhint %}` callouts.
   - `CHANGELOG.md` — append bullets to `## [Unreleased]` under the matching
     `### nexus-cli` / `### nexus-sdk` / `### nexus-toolkit` / `### docs`
     sections; `#### Added`, `#### Changed`, `#### Fixed`, `#### Removed`
     are the allowed verbs.
1. **Verify** (in this order):

   ```bash
   just pre-commit cargo-check          # cargo check --locked --workspace --bins --examples
   just pre-commit cargo-nextest-run    # cargo nextest run --locked --fail-fast … (needs docker)
   just pre-commit cargo-clippy         # cargo clippy --locked --all-targets --all-features
   just pre-commit cargo-nightly-fmt    # cargo +<nightly> fmt --all --check
   ```

   The fmt step is **required**: `rustfmt.toml` uses several unstable
   options (`imports_granularity`, `group_imports`, `reorder_impl_items`,
   …) that the stable rustfmt rejects. `cargo-nightly-fmt` resolves the
   pinned nightly from `.nightly-version` (currently `nightly-2025-01-06`)
   automatically; install it with
   `rustup toolchain install "$(cat .nightly-version)" --component rustfmt`
   if you don't have it yet.

1. **Run the equivalent `just` recipes** when in doubt — they wrap the
   above with the right toolchain selection:

   ```bash
   just sdk check && just sdk test && just sdk fmt-check
   just cli check && just cli test && just cli fmt-check
   just pre-commit cargo-nightly-fmt
   ```

## Definition of done

A change is "done" only when **all** of the following pass:

- `cargo +stable check` and `cargo +stable test` for every touched
  crate (`-p nexus-sdk -p nexus-cli` at minimum).
- `cargo +nightly fmt --all --check` (use the pinned `.nightly-version`).
- New public SDK items have rustdoc that explains the _why_, not only the
  _what_; non-obvious branches (timeouts, idempotency, error mapping) get
  a one-liner.
- New CLI subcommands are listed in `docs/cli.md` with the same flag
  ordering and naming as `--help`, and have a corresponding bullet in
  `CHANGELOG.md` under `[Unreleased]`.
- Pre-existing tests still pass — keep an eye on flaky CLI tests that
  share `HOME` / `$SUI_RPC_URL` / `$SUI_PK`; they use `serial_test::serial`
  for a reason, never run them concurrently without that attribute.

## When in doubt

- Read [STYLE_GUIDE.md](STYLE_GUIDE.md) for markdown rules
  (markdownlint-enforced) — ordered list items use `1.` for every entry
  (MD029 style `1/1/1`); emphasis uses `*asterisks*`, not underscores
  (MD049).
- Read [CONTRIBUTING.md](CONTRIBUTING.md) for commit-message conventions
  (Conventional Commits, imperative tense) and the pre-commit hook setup
  (`./.pre-commit/pre-commit --install`).
- The on-chain Move source lives in the sibling `nexus-next` repo —
  cross-check struct layouts, function signatures, and idents there
  before guessing.
