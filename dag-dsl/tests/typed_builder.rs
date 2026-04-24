//! Integration tests for the strict, flat [`TypedDagBuilder`].
//!
//! A hand-written [`ToolDescriptor`] exercises the typed authoring surface
//! end-to-end without requiring the derive macro or codegen.

use {
    nexus_dag_dsl::{InPort, Ok as OkVariant, OutPort, ToolDescriptor, ToolFqn, TypedDagBuilder},
    std::str::FromStr,
};

// ---------------------------------------------------------------------------
// Hand-written descriptor for `xyz.taluslabs.math.i64.add@1`.
//
// Inputs:  a: i64, b: i64
// Outputs: { ok: { result: i64 } }
// ---------------------------------------------------------------------------

struct AddTool;

struct AddInputs {
    a: InPort<i64>,
    b: InPort<i64>,
}

struct AddOutputs {
    ok: AddOkVariant,
}

struct AddOkVariant {
    result: OutPort<OkVariant, i64>,
}

impl ToolDescriptor for AddTool {
    type Inputs = AddInputs;
    type Outputs = AddOutputs;

    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.taluslabs.math.i64.add@1").unwrap()
    }

    fn inputs_for(vertex_name: &str) -> Self::Inputs {
        AddInputs {
            a: InPort::new(vertex_name, "a"),
            b: InPort::new(vertex_name, "b"),
        }
    }

    fn outputs_for(vertex_name: &str) -> Self::Outputs {
        AddOutputs {
            ok: AddOkVariant {
                result: OutPort::new(vertex_name, "ok", "result"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Hand-written descriptor for a fan-out tool:
// `xyz.taluslabs.util.make_vec@1`
//
// Inputs:  n: i64
// Outputs: { ok: { items: Vec<i64> } }
// Used to exercise connect_for_each / connect_collect arity.
// ---------------------------------------------------------------------------

struct MakeVec;

struct MakeVecInputs {
    n: InPort<i64>,
}

struct MakeVecOutputs {
    ok: MakeVecOkVariant,
}

struct MakeVecOkVariant {
    items: OutPort<OkVariant, Vec<i64>>,
}

impl ToolDescriptor for MakeVec {
    type Inputs = MakeVecInputs;
    type Outputs = MakeVecOutputs;

    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.taluslabs.util.make_vec@1").unwrap()
    }

    fn inputs_for(vertex_name: &str) -> Self::Inputs {
        MakeVecInputs {
            n: InPort::new(vertex_name, "n"),
        }
    }

    fn outputs_for(vertex_name: &str) -> Self::Outputs {
        MakeVecOutputs {
            ok: MakeVecOkVariant {
                items: OutPort::new(vertex_name, "ok", "items"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Descriptor for a sum-of-vec tool:
// Inputs: vec: Vec<i64>
// Outputs: { ok: { result: i64 } }
// ---------------------------------------------------------------------------

struct SumVec;

struct SumVecInputs {
    vec: InPort<Vec<i64>>,
}

struct SumVecOutputs {
    ok: SumVecOkVariant,
}

struct SumVecOkVariant {
    result: OutPort<OkVariant, i64>,
}

impl ToolDescriptor for SumVec {
    type Inputs = SumVecInputs;
    type Outputs = SumVecOutputs;

    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.taluslabs.math.i64.sum@1").unwrap()
    }

    fn inputs_for(vertex_name: &str) -> Self::Inputs {
        SumVecInputs {
            vec: InPort::new(vertex_name, "vec"),
        }
    }

    fn outputs_for(vertex_name: &str) -> Self::Outputs {
        SumVecOutputs {
            ok: SumVecOkVariant {
                result: OutPort::new(vertex_name, "ok", "result"),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Construct a simple two-vertex typed DAG with a single `connect` and an
/// output; verify `.build()` succeeds and produces the expected wire
/// structure.
///
/// What breaks if this test is deleted: the strict happy path could silently
/// regress — e.g., `add::<T>` not recording the vertex, `connect` not
/// lowering to the relaxed builder's `edge`, or `output` not being wired.
#[test]
fn builds_minimal_typed_dag() {
    let mut dag = TypedDagBuilder::new();
    let a = dag.add::<AddTool>("a");
    let b = dag.add::<AddTool>("b");

    dag.entry_port(&a, "a");
    dag.inline_default(a.inp.b, 1i64);
    dag.inline_default(b.inp.b, 2i64);
    dag.connect(a.out.ok.result, b.inp.a);
    dag.output(b.out.ok.result);

    let built = dag.build().expect("valid typed DAG");
    assert_eq!(built.vertices.len(), 2);
    assert_eq!(built.edges.len(), 1);
    assert_eq!(built.outputs.as_ref().unwrap().len(), 1);
}

/// `connect_for_each` accepts an `OutPort<_, Vec<T>>` source and an
/// `InPort<T>` destination; `connect_collect` is the inverse. Both are
/// verified against the existing wire-level validator's for-each / collect
/// pairing rule.
///
/// What breaks if this test is deleted: the for-each / collect arity — the
/// whole point of separate methods instead of a single `connect` — could
/// silently regress to passing mismatched types.
#[test]
fn for_each_and_collect_arity_round_trip() {
    let mut dag = TypedDagBuilder::new();
    let make = dag.add::<MakeVec>("make");
    let add = dag.add::<AddTool>("add");
    let sum = dag.add::<SumVec>("sum");

    dag.entry_port(&make, "n");
    dag.inline_default(add.inp.b, 1i64);

    // Vec<i64> source, i64 destination → ForEach.
    dag.connect_for_each(make.out.ok.items, add.inp.a);
    // i64 source, Vec<i64> destination → Collect.
    dag.connect_collect(add.out.ok.result, sum.inp.vec);
    dag.output(sum.out.ok.result);

    let built = dag.build().expect("valid for-each / collect DAG");
    let edge_kinds: Vec<_> = built.edges.iter().map(|e| e.kind.clone()).collect();
    use nexus_dag_dsl::EdgeKind;
    assert_eq!(edge_kinds, vec![EdgeKind::ForEach, EdgeKind::Collect]);
}

/// Typed and untyped vertices coexist in the same DAG: an untyped vertex
/// can be connected via `raw().edge(...)` (the escape hatch) and still
/// participates in topology validation.
///
/// What breaks if this test is deleted: the strict-is-adoptive property
/// could silently regress — users would be forced to make every vertex
/// typed or give up type safety entirely.
#[test]
fn typed_and_untyped_coexist() {
    let mut dag = TypedDagBuilder::new();
    let a = dag.add::<AddTool>("a");
    let legacy = dag.add_untyped("legacy", ToolFqn::from_str("xyz.legacy.tool@1").unwrap());

    dag.entry_port(&a, "a");
    dag.inline_default(a.inp.b, 7i64);

    // Typed output, untyped input — lower typed to relaxed at the boundary.
    dag.raw()
        .edge(a.out.ok.result.untyped(), legacy.inp("payload"));
    dag.raw().output(legacy.out("ok", "result"));

    let built = dag.build().expect("valid mixed DAG");
    assert_eq!(built.vertices.len(), 2);
    assert_eq!(built.edges.len(), 1);
}
