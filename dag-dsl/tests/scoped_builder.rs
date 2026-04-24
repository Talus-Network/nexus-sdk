//! Integration tests for the scoped authoring surface.
//!
//! These tests verify that `for_each` / `do_while` closures lower to the
//! correct `EdgeKind`s in the wire format and that scope-local vertices
//! are auto-prefixed with `.` per the plan's naming decision.

use {
    nexus_dag_dsl::{DagBuilder, EdgeKind, ToolFqn},
    std::str::FromStr,
};

fn fqn(s: &str) -> ToolFqn {
    ToolFqn::from_str(s).expect("fixture FQN must parse")
}

/// A single for-each scope with one internal vertex and a collect back to
/// an external aggregator emits the expected edge kinds — a `ForEach` edge
/// from the outer source into the loop body, and a `Collect` edge out to
/// the aggregator.
///
/// What breaks if this test is deleted: the scope's auto-edge-kind
/// promotion (consume → ForEach, collect → Collect) could silently regress
/// to Normal edges, producing a DAG that the wire validator would reject
/// as "collect without for-each" or accept but execute incorrectly.
#[test]
fn for_each_scope_emits_for_each_and_collect_edges() {
    let mut dag = DagBuilder::new();
    let splitter = dag.offchain("splitter", fqn("xyz.taluslabs.util.make_vec@1"));
    let aggregator = dag.offchain("aggregator", fqn("xyz.taluslabs.math.i64.sum@1"));
    dag.entry_port(&splitter, "n");

    dag.for_each_named("per_item", splitter.out("ok", "items"), |scope, item| {
        let add = scope.offchain("add", fqn("xyz.taluslabs.math.i64.add@1"));
        scope.inline_default(&add, "b", 1);
        scope.consume(item, add.inp("a"));
        scope.collect(add.out("ok", "result"), aggregator.inp("vec"));
    });
    dag.output(aggregator.out("ok", "result"));

    let built = dag.build().expect("valid for-each DAG");
    let kinds: Vec<_> = built.edges.iter().map(|e| e.kind.clone()).collect();
    assert_eq!(kinds, vec![EdgeKind::ForEach, EdgeKind::Collect]);

    // Scope-local vertex was prefixed with `.`.
    let names: Vec<_> = built.vertices.iter().map(|v| v.name.clone()).collect();
    assert!(names.contains(&"per_item.add".to_string()));
}

/// Anonymous for-each scopes auto-name themselves `foreach_N`; the counter
/// is monotonic per-builder so two anonymous scopes at the same level do
/// not collide on vertex names.
///
/// What breaks if this test is deleted: anonymous scope name collisions
/// could silently reappear, producing a `DagError::DuplicateVertex` or
/// worse, a validator-accepted DAG that aliases two iterations' vertices.
///
/// This test intentionally does not call `.build()`: two independent
/// for-each loops feeding into a common sink would be rejected by the
/// wire validator as a race condition on the sink's input port (which is
/// a real-DAG rule, unrelated to DSL naming). The DSL-level invariant —
/// "sibling anonymous scopes get distinct auto-names" — can be checked
/// directly from the builder's `vertices()` accessor before `.build()`.
#[test]
fn anonymous_for_each_scopes_get_distinct_auto_names() {
    let mut dag = DagBuilder::new();
    let a_src = dag.offchain("src_a", fqn("xyz.taluslabs.util.make_vec@1"));
    let b_src = dag.offchain("src_b", fqn("xyz.taluslabs.util.make_vec@1"));
    let sink_a = dag.offchain("sink_a", fqn("xyz.taluslabs.math.i64.sum@1"));
    let sink_b = dag.offchain("sink_b", fqn("xyz.taluslabs.math.i64.sum@1"));
    dag.entry_port(&a_src, "n");
    dag.entry_port(&b_src, "n");

    dag.for_each(a_src.out("ok", "items"), |scope, item| {
        let t = scope.offchain("t", fqn("xyz.taluslabs.math.i64.add@1"));
        scope.inline_default(&t, "b", 1);
        scope.consume(item, t.inp("a"));
        scope.collect(t.out("ok", "result"), sink_a.inp("vec"));
    });
    dag.for_each(b_src.out("ok", "items"), |scope, item| {
        let t = scope.offchain("t", fqn("xyz.taluslabs.math.i64.add@1"));
        scope.inline_default(&t, "b", 2);
        scope.consume(item, t.inp("a"));
        scope.collect(t.out("ok", "result"), sink_b.inp("vec"));
    });

    let names: Vec<_> = dag.vertices().iter().map(|v| v.name.clone()).collect();
    assert!(names.contains(&"foreach_0.t".to_string()));
    assert!(names.contains(&"foreach_1.t".to_string()));
}

/// A do-while scope emits `DoWhile` back-edges and `Break` exit edges with
/// the correct kinds, and `static_input` produces `Static`-kinded edges.
///
/// What breaks if this test is deleted: the scope's auto-edge-kind
/// promotion for do-while / break / static could silently degrade to
/// Normal edges, mangling loop semantics.
#[test]
fn do_while_scope_emits_do_while_break_and_static_edges() {
    let mut dag = DagBuilder::new();
    let init = dag.offchain("init", fqn("xyz.taluslabs.math.i64.add@1"));
    let limit_src = dag.offchain("limit_src", fqn("xyz.taluslabs.math.i64.add@1"));
    let sink = dag.offchain("sink", fqn("xyz.taluslabs.math.i64.add@1"));
    dag.entry_port(&init, "a");
    dag.inline_default(&init, "b", 0);
    dag.entry_port(&limit_src, "a");
    dag.inline_default(&limit_src, "b", 0);

    dag.do_while_named("loop", init.out("ok", "result"), |scope, state| {
        let step = scope.offchain("step", fqn("xyz.taluslabs.math.i64.add@1"));
        let cmp = scope.offchain("cmp", fqn("xyz.taluslabs.math.i64.cmp@1"));
        scope.inline_default(&step, "b", 1);
        scope.feed_state(state, step.inp("a"));
        scope.edge(step.out("ok", "result"), cmp.inp("a"));
        scope.static_input(limit_src.out("ok", "result"), cmp.inp("b"));
        scope.continue_with(cmp.out("lt", "a"), step.inp("a"));
        scope.break_to(cmp.out("eq", "a"), sink.inp("a"));
    });
    dag.inline_default(&sink, "b", 0);
    dag.output(sink.out("ok", "result"));

    let built = dag.build().expect("do-while DAG builds");
    let kinds: Vec<_> = built.edges.iter().map(|e| e.kind.clone()).collect();
    assert!(kinds.contains(&EdgeKind::DoWhile));
    assert!(kinds.contains(&EdgeKind::Break));
    assert!(kinds.contains(&EdgeKind::Static));
}

/// Nested for-each scopes compose — vertex names join outer and inner
/// prefixes with `.`.
///
/// What breaks if this test is deleted: nested-scope prefix composition
/// could silently break (e.g., collapsing to the innermost name only),
/// producing collisions across sibling inner scopes.
///
/// This test intentionally does not call `.build()`: the Nexus wire
/// validator rejects nested `ForEach` edges (see
/// `sdk/src/dag/_dags/nested_for_each_invalid.json`), which is an
/// execution-semantics constraint independent of the DSL's name
/// composition. The DSL-level invariant — "inner-scope vertex names
/// compose with `.`" — is checked directly against the builder's
/// `vertices()` accessor. When the user actually needs nested iteration,
/// a different Nexus construct (e.g., collect-then-for-each) is required;
/// the DSL intentionally does not forbid the nested DSL syntax, so that
/// authoring helpers can compose, but the wire validator is the final
/// gate on executable shape.
#[test]
fn nested_for_each_scopes_compose_dotted_prefixes() {
    let mut dag = DagBuilder::new();
    let outer_src = dag.offchain("outer_src", fqn("xyz.taluslabs.util.make_vec@1"));
    let sink = dag.offchain("sink", fqn("xyz.taluslabs.math.i64.sum@1"));
    dag.entry_port(&outer_src, "n");

    dag.for_each_named(
        "outer",
        outer_src.out("ok", "items"),
        |outer_scope, outer_item| {
            let inner_src = outer_scope.offchain("inner_src", fqn("xyz.taluslabs.util.make_vec@1"));
            outer_scope.consume(outer_item, inner_src.inp("n"));

            outer_scope.for_each_named(
                "inner",
                inner_src.out("ok", "items"),
                |inner_scope, inner_item| {
                    let add = inner_scope.offchain("add", fqn("xyz.taluslabs.math.i64.add@1"));
                    inner_scope.inline_default(&add, "b", 1);
                    inner_scope.consume(inner_item, add.inp("a"));
                    inner_scope.collect(add.out("ok", "result"), sink.inp("vec"));
                },
            );
        },
    );

    let names: Vec<_> = dag.vertices().iter().map(|v| v.name.clone()).collect();
    assert!(names.contains(&"outer.inner_src".to_string()));
    assert!(names.contains(&"outer.inner.add".to_string()));
}
