# nexus-dag-dsl

A type-safe Rust domain-specific language for constructing Nexus DAGs
programmatically. Emits the canonical [`nexus_sdk::types::Dag`] wire
struct — the same JSON format the CLI's `dag validate` / `dag publish` /
`dag execute` commands consume.

## Two variants, two surfaces, one core

- **Relaxed** ([`DagBuilder`]) — string-keyed ports, validation at
  `.build()`. Works with any tool without needing a descriptor.
- **Strict** ([`TypedDagBuilder`]) — typed port handles via
  [`ToolDescriptor`]. Port-payload type mismatches are compile errors
  (guarded by `trybuild` compile-fail tests).
- **Flat** — one method per edge kind; 1:1 with the JSON wire format.
  Example (relaxed): `dag.edge_for_each(out, inp)`. Example (strict):
  `dag.connect_for_each(out, inp)` — bounds enforce `Vec<T>` source
  and `T` destination.
- **Scoped** — `for_each` / `do_while` closures auto-tag edge kinds
  and auto-prefix scope-local vertex names with `.` (e.g.
  `per_org.per_member.notify`). Example: `dag.for_each(src, |scope,
  item| { ... })`.

All four combinations (relaxed/strict × flat/scoped) use the same
internal graph model and the same validator, which delegates
wire-level rules to `nexus_sdk::dag::validator::validate`.

## Quick start — relaxed flat

```rust
use nexus_dag_dsl::{DagBuilder, ToolFqn};
use std::str::FromStr;

let mut dag = DagBuilder::new();
let a = dag.offchain("a", ToolFqn::from_str("xyz.taluslabs.math.i64.add@1").unwrap());
let b = dag.offchain("b", ToolFqn::from_str("xyz.taluslabs.math.i64.add@1").unwrap());
dag.entry_port(&a, "a")
    .inline_default(&a, "b", 1)
    .edge(a.out("ok", "result"), b.inp("a"))
    .inline_default(&b, "b", 2)
    .output(b.out("ok", "result"));
let built = dag.build().expect("valid DAG");
```

## Quick start — strict with `tool_descriptor!` macro

```rust
nexus_dag_dsl::tool_descriptor! {
    pub struct AddTool;
    fqn = "xyz.taluslabs.math.i64.add@1";
    inputs { a: i64, b: i64 }
    outputs { ok { result: i64 } }
}

use nexus_dag_dsl::TypedDagBuilder;
let mut dag = TypedDagBuilder::new();
let a = dag.add::<AddTool>("a");
let b = dag.add::<AddTool>("b");
dag.entry_port(&a, "a");
dag.inline_default(a.inp.b, 1i64);
dag.inline_default(b.inp.b, 2i64);
dag.connect(a.out.ok.result, b.inp.a);  // compile error if types mismatch
dag.output(b.out.ok.result);
let built = dag.build().unwrap();
```

## Three ways to produce a `ToolDescriptor`

1. **Hand-written** — implement the trait directly. Zero dependencies.
1. **`tool_descriptor!` declarative macro** (this crate) — inline spec,
   no proc-macro, no codegen.
1. **`nexus-dsl-codegen`** — reads `tool-meta.json` artifacts (from
   `nexus-toolkit`'s `--meta` flag) and emits descriptor modules for
   Rust *and* TypeScript from the same source of truth.

## TypeScript parity

A matching TypeScript package — [`@nexus/dag-dsl`](../ts-dsl/) in this
workspace — mirrors the Rust API and produces byte-identical wire JSON.
Cross-language round-trip tests run in CI against every committed DAG
fixture in `sdk/src/dag/_dags/`.

[`DagBuilder`]: crate::DagBuilder
[`TypedDagBuilder`]: crate::TypedDagBuilder
[`ToolDescriptor`]: crate::ToolDescriptor
