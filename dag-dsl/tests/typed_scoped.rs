//! Integration tests for the typed scoped layer — the strict counterpart
//! of `scoped_builder.rs`. `ItemPort<T>` and `StatePort<T>` carry the
//! per-iteration payload type through the closure so port-payload
//! mismatches at scope boundaries are compile errors.

use nexus_dag_dsl::{EdgeKind, TypedDagBuilder};

// Descriptor for `xyz.taluslabs.util.make_vec@1` — emits a Vec<i64>.
nexus_dag_dsl::tool_descriptor! {
    pub struct MakeVec;
    fqn = "xyz.taluslabs.util.make_vec@1";
    inputs { n: i64 }
    outputs { ok { items: Vec<i64> } }
}

// Descriptor for `xyz.taluslabs.math.i64.add@1` — a + b -> result.
nexus_dag_dsl::tool_descriptor! {
    pub struct AddTool;
    fqn = "xyz.taluslabs.math.i64.add@1";
    inputs { a: i64, b: i64 }
    outputs { ok { result: i64 } }
}

// Descriptor for a vec-sum tool.
nexus_dag_dsl::tool_descriptor! {
    pub struct SumVec;
    fqn = "xyz.taluslabs.math.i64.sum@1";
    inputs { vec: Vec<i64> }
    outputs { ok { result: i64 } }
}

// Descriptor for a vec-length tool (used to exercise do_while
// integration around the fixture-friendly cmp shape).
nexus_dag_dsl::tool_descriptor! {
    pub struct CmpTool;
    fqn = "xyz.taluslabs.math.i64.cmp@1";
    inputs { a: i64, b: i64 }
    outputs {
        lt { a: i64, b: i64 }
        gt { a: i64, b: i64 }
        eq { a: i64, b: i64 }
    }
}

/// A typed for-each scope lowers to a `ForEach` edge from the scope's
/// source to the consuming vertex's typed input and a `Collect` edge
/// from the loop-body output to an outer `Vec<T>` input.
///
/// What breaks if this test is deleted: the typed scoped surface could
/// silently regress to bypassing the `ItemPort<T>` → `InPort<T>` type
/// check on `consume`, or the `T` → `Vec<T>` type check on `collect`,
/// losing the one-iteration compile-time guarantee the scoped typed
/// layer exists to provide.
#[test]
fn typed_for_each_scope_emits_for_each_and_collect() {
    let mut dag = TypedDagBuilder::new();
    let splitter = dag.add::<MakeVec>("splitter");
    let aggregator = dag.add::<SumVec>("aggregator");
    dag.entry_port(&splitter, "n");

    dag.for_each_named("per_item", splitter.out.ok.items, |scope, item| {
        let add = scope.add::<AddTool>("add");
        scope.inline_default(add.inp.b, 1i64);
        scope.consume(item, add.inp.a);
        scope.collect(add.out.ok.result, aggregator.inp.vec);
    });
    dag.output(aggregator.out.ok.result);

    let built = dag.build().expect("typed for-each DAG builds");
    let kinds: Vec<_> = built.edges.iter().map(|e| e.kind.clone()).collect();
    assert_eq!(kinds, vec![EdgeKind::ForEach, EdgeKind::Collect]);

    // Scope-local vertex is auto-prefixed with the scope's name using `.`.
    let names: Vec<_> = built.vertices.iter().map(|v| v.name.clone()).collect();
    assert!(names.contains(&"per_item.add".to_string()));
}

/// A typed do-while scope emits the full loop-control edge-kind set:
/// a seeding edge (Normal) from `feed_state`, a DoWhile back-edge from
/// `continue_with`, a Break edge from `break_to`, and a Static edge
/// from `static_input`. Each is type-checked on both endpoints.
///
/// What breaks if this test is deleted: the typed do-while scope's
/// edge-kind promotion could silently degrade (e.g. `continue_with`
/// emitting a Normal edge instead of DoWhile), mangling loop
/// semantics despite the DSL's superficial appearance of correctness.
#[test]
fn typed_do_while_scope_emits_all_loop_edge_kinds() {
    let mut dag = TypedDagBuilder::new();
    let init = dag.add::<AddTool>("init");
    let limit_src = dag.add::<AddTool>("limit_src");
    let sink = dag.add::<AddTool>("sink");
    dag.entry_port(&init, "a");
    dag.inline_default(init.inp.b, 0i64);
    dag.entry_port(&limit_src, "a");
    dag.inline_default(limit_src.inp.b, 0i64);

    dag.do_while_named("loop", init.out.ok.result, |scope, state| {
        let step = scope.add::<AddTool>("step");
        let cmp = scope.add::<CmpTool>("cmp");
        scope.inline_default(step.inp.b, 1i64);
        scope.feed_state(state, step.inp.a.clone());
        scope.connect(step.out.ok.result.clone(), cmp.inp.a);
        scope.static_input(limit_src.out.ok.result, cmp.inp.b);
        // Back-edge from cmp.lt → step.a (re-using the same step input
        // port that the seed was feeding; InPort<T> is Clone to allow
        // this multi-feed pattern required by real loops).
        scope.continue_with(cmp.out.lt.a, step.inp.a);
        scope.break_to(cmp.out.eq.a, sink.inp.a);
    });
    dag.inline_default(sink.inp.b, 0i64);
    dag.output(sink.out.ok.result);

    let built = dag.build().expect("typed do-while DAG builds");
    let kinds: Vec<_> = built.edges.iter().map(|e| e.kind.clone()).collect();
    assert!(kinds.contains(&EdgeKind::DoWhile));
    assert!(kinds.contains(&EdgeKind::Break));
    assert!(kinds.contains(&EdgeKind::Static));

    // Scope-local vertices prefixed with the scope's name.
    let names: Vec<_> = built.vertices.iter().map(|v| v.name.clone()).collect();
    assert!(names.contains(&"loop.step".to_string()));
    assert!(names.contains(&"loop.cmp".to_string()));
}

/// Nested typed scopes compose prefixes with `.` (outer.inner.vertex)
/// and preserve typed handles across nesting. Intentionally checked via
/// the builder's `raw().vertices()` accessor rather than `.build()`:
/// the Nexus wire validator rejects nested ForEach by design (see
/// `sdk/src/dag/_dags/nested_for_each_invalid.json`), but DSL-level
/// name composition is a distinct invariant worth guarding.
///
/// What breaks if this test is deleted: nested-scope name composition
/// could silently flatten to innermost-only, producing vertex-name
/// collisions across sibling inner scopes that aren't caught until
/// actual execution.
#[test]
fn nested_typed_for_each_scopes_compose_dotted_prefixes() {
    let mut dag = TypedDagBuilder::new();
    let outer_src = dag.add::<MakeVec>("outer_src");
    let sink = dag.add::<SumVec>("sink");
    dag.entry_port(&outer_src, "n");

    dag.for_each_named(
        "outer",
        outer_src.out.ok.items,
        |outer_scope, _outer_item| {
            let inner_src = outer_scope.add::<MakeVec>("inner_src");
            outer_scope.inline_default(inner_src.inp.n, 1i64);

            outer_scope.for_each_named(
                "inner",
                inner_src.out.ok.items,
                |inner_scope, inner_item| {
                    let add = inner_scope.add::<AddTool>("add");
                    inner_scope.inline_default(add.inp.b, 1i64);
                    inner_scope.consume(inner_item, add.inp.a);
                    inner_scope.collect(add.out.ok.result, sink.inp.vec);
                },
            );
        },
    );

    let names: Vec<_> = dag
        .raw()
        .vertices()
        .iter()
        .map(|v| v.name.clone())
        .collect();
    assert!(names.contains(&"outer.inner_src".to_string()));
    assert!(names.contains(&"outer.inner.add".to_string()));
}

/// `TypedForEachScope::raw()` exposes the underlying relaxed builder as
/// an escape hatch — typed-scope-local descriptors and relaxed-scope
/// free-form edges coexist.
///
/// What breaks if this test is deleted: users who need to connect a
/// typed scope-local vertex to an untyped vertex outside the typed
/// system could have no clean path — forcing them to duplicate every
/// tool as a typed descriptor or give up the scoped typing entirely.
#[test]
fn typed_for_each_scope_raw_escape_hatch_works() {
    let mut dag = TypedDagBuilder::new();
    let make = dag.add::<MakeVec>("make");
    let aggregator = dag.add::<SumVec>("aggregator");
    dag.entry_port(&make, "n");

    dag.for_each_named("per_item", make.out.ok.items, |scope, item| {
        let add = scope.add::<AddTool>("add");
        scope.inline_default(add.inp.b, 1i64);
        scope.consume(item, add.inp.a);
        // Use raw() to add a Collect edge via the relaxed API — matches
        // what happens when the user needs to bridge typed scope-local
        // work with untyped neighbours.
        scope.raw().edge_collect(
            add.out.ok.result.untyped(),
            aggregator.inp.vec.clone().untyped(),
        );
    });
    dag.output(aggregator.out.ok.result);

    let built = dag.build().expect("escape-hatch DAG builds");
    let kinds: Vec<_> = built.edges.iter().map(|e| e.kind.clone()).collect();
    assert_eq!(kinds, vec![EdgeKind::ForEach, EdgeKind::Collect]);
}
