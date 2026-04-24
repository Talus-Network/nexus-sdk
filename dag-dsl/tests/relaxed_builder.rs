//! Integration tests for the relaxed, flat [`DagBuilder`].
//!
//! Every test covers one named rule or one edge case. When a test fails, its
//! doc comment tells the reader exactly which property has regressed.

use {
    assert_matches::assert_matches,
    nexus_dag_dsl::{DagBuilder, DagError, EdgeKind, ToolFqn},
    std::str::FromStr,
};

fn fqn(s: &str) -> ToolFqn {
    ToolFqn::from_str(s).expect("fixture FQN must parse")
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

/// A minimally-valid two-vertex DAG with one normal edge and one output
/// builds successfully.
///
/// What breaks if this test is deleted: the whole happy-path authoring
/// surface could silently regress (new constructors mis-wiring the internal
/// collections, for example) without any narrower test catching it.
#[test]
fn builds_minimal_two_vertex_dag() {
    let mut dag = DagBuilder::new();
    let a = dag.offchain("a", fqn("xyz.taluslabs.math.i64.add@1"));
    let b = dag.offchain("b", fqn("xyz.taluslabs.math.i64.add@1"));
    dag.entry_port(&a, "a")
        .inline_default(&a, "b", 1)
        .edge(a.out("ok", "result"), b.inp("a"))
        .inline_default(&b, "b", 2)
        .output(b.out("ok", "result"));

    let built = dag.build().expect("valid DAG builds");
    assert_eq!(built.vertices.len(), 2);
    assert_eq!(built.edges.len(), 1);
    assert_eq!(built.edges[0].kind, EdgeKind::Normal);
    assert_eq!(built.outputs.as_ref().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// Local validation (reference integrity)
// ---------------------------------------------------------------------------

/// An edge that references an unknown vertex in `from` surfaces
/// [`DagError::EdgeFromUnknownVertex`] with the offending vertex name.
///
/// What breaks if this test is deleted: typos in edge `from` fields would
/// propagate silently into the wire-level validator, which reports only a
/// generic "undefined connection" — losing the user-actionable locator.
#[test]
fn detects_edge_from_unknown_vertex() {
    let mut dag = DagBuilder::new();
    let b = dag.offchain("b", fqn("xyz.taluslabs.math.i64.add@1"));
    // `ghost` was never added.
    dag.edge(
        nexus_dag_dsl::VertexRef::named("ghost").out("ok", "result"),
        b.inp("a"),
    );

    let errors = dag.build().expect_err("should fail");
    assert!(errors.iter().any(|e| matches!(
        e,
        DagError::EdgeFromUnknownVertex { vertex, .. } if vertex == "ghost"
    )));
}

/// An edge that references an unknown vertex in `to` surfaces
/// [`DagError::EdgeToUnknownVertex`] with the offending vertex name.
///
/// What breaks if this test is deleted: same as above, but for the
/// destination side of the edge.
#[test]
fn detects_edge_to_unknown_vertex() {
    let mut dag = DagBuilder::new();
    let a = dag.offchain("a", fqn("xyz.taluslabs.math.i64.add@1"));
    dag.edge(
        a.out("ok", "result"),
        nexus_dag_dsl::VertexRef::named("phantom").inp("x"),
    );

    let errors = dag.build().expect_err("should fail");
    assert!(errors.iter().any(|e| matches!(
        e,
        DagError::EdgeToUnknownVertex { vertex, .. } if vertex == "phantom"
    )));
}

/// Entry groups that list unknown vertices surface
/// [`DagError::EntryGroupUnknownVertex`] with both the group name and the
/// offending vertex name.
///
/// What breaks if this test is deleted: the wire-level validator would
/// accept an entry group listing a nonexistent vertex as structurally
/// well-formed until much later in the pipeline (DAG execution).
#[test]
fn detects_entry_group_unknown_vertex() {
    let mut dag = DagBuilder::new();
    let _a = dag.offchain("a", fqn("xyz.taluslabs.math.i64.add@1"));
    dag.entry_group("main", ["a", "never_added"]);

    let errors = dag.build().expect_err("should fail");
    assert!(errors.iter().any(|e| matches!(
        e,
        DagError::EntryGroupUnknownVertex { group, vertex } if group == "main" && vertex == "never_added"
    )));
}

/// Default values targeting unknown vertices surface
/// [`DagError::DefaultValueUnknownVertex`] with the vertex name and port.
///
/// What breaks if this test is deleted: defaults attached to nonexistent
/// vertices would be serialized into the wire DAG and only detected at
/// runtime.
#[test]
fn detects_default_value_unknown_vertex() {
    let mut dag = DagBuilder::new();
    dag.offchain("a", fqn("xyz.taluslabs.math.i64.add@1"));
    dag.inline_default(&nexus_dag_dsl::VertexRef::named("missing"), "b", 99);

    let errors = dag.build().expect_err("should fail");
    assert!(errors.iter().any(|e| matches!(
        e,
        DagError::DefaultValueUnknownVertex { vertex, port } if vertex == "missing" && port == "b"
    )));
}

/// Outputs referencing unknown vertices surface
/// [`DagError::OutputUnknownVertex`] with the vertex name, variant, and
/// port.
///
/// What breaks if this test is deleted: typos in the `output()` vertex
/// reference would go undetected at build time, producing DAGs whose
/// outputs can never resolve.
#[test]
fn detects_output_unknown_vertex() {
    let mut dag = DagBuilder::new();
    dag.offchain("a", fqn("xyz.taluslabs.math.i64.add@1"));
    dag.output(nexus_dag_dsl::VertexRef::named("nope").out("ok", "result"));

    let errors = dag.build().expect_err("should fail");
    assert!(errors.iter().any(|e| matches!(
        e,
        DagError::OutputUnknownVertex { vertex, variant, port }
            if vertex == "nope" && variant == "ok" && port == "result"
    )));
}

/// Multiple local errors are aggregated and returned together in a single
/// `Err(Vec<DagError>)`, not just the first.
///
/// What breaks if this test is deleted: `.build()` could silently regress
/// to fail-fast behavior, forcing users to fix errors one at a time.
#[test]
fn aggregates_multiple_local_errors() {
    let mut dag = DagBuilder::new();
    let _a = dag.offchain("a", fqn("xyz.taluslabs.math.i64.add@1"));
    dag.entry_group("main", ["ghost"]);
    dag.output(nexus_dag_dsl::VertexRef::named("nope").out("ok", "result"));
    dag.inline_default(&nexus_dag_dsl::VertexRef::named("missing"), "b", 1);

    let errors = dag.build().expect_err("should fail");
    assert!(
        errors.len() >= 3,
        "expected at least 3 errors, got {errors:?}"
    );
}

// ---------------------------------------------------------------------------
// Wire-level validator delegation
// ---------------------------------------------------------------------------

/// A topology with a cycle surfaces [`DagError::WireValidation`] from the
/// sdk validator rather than being silently accepted.
///
/// What breaks if this test is deleted: cycles could make it past the DSL
/// layer if the delegation to `nexus_sdk::dag::validator::validate` were
/// accidentally removed or bypassed.
#[test]
fn wire_validator_rejects_cycles() {
    let mut dag = DagBuilder::new();
    let a = dag.offchain("a", fqn("xyz.taluslabs.math.i64.add@1"));
    let b = dag.offchain("b", fqn("xyz.taluslabs.math.i64.add@1"));

    dag.entry_port(&a, "a");
    dag.inline_default(&a, "b", 1);
    dag.inline_default(&b, "b", 1);
    dag.edge(a.out("ok", "result"), b.inp("a"));
    dag.edge(b.out("ok", "result"), a.inp("a"));
    dag.output(a.out("ok", "result"));

    let errors = dag.build().expect_err("cyclic DAG must fail");
    assert_matches!(errors.as_slice(), [DagError::WireValidation { .. }]);
}
