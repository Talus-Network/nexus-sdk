//! Integration tests for the `tool_descriptor!` declarative macro.
//!
//! The macro is the no-codegen, no-fetch path to a [`ToolDescriptor`] impl.
//! These tests exercise it end-to-end: macro expands, typed ports are
//! usable in [`TypedDagBuilder`], and the resulting DAG matches the
//! wire-format expectations.

use nexus_dag_dsl::{ToolDescriptor, TypedDagBuilder};

// Simple single-variant tool.
nexus_dag_dsl::tool_descriptor! {
    pub struct AddTool;
    fqn = "xyz.taluslabs.math.i64.add@1";
    inputs {
        a: i64,
        b: i64,
    }
    outputs {
        ok {
            result: i64,
        }
    }
}

// Multi-variant tool: cmp has ok / lt / gt / eq variants.
nexus_dag_dsl::tool_descriptor! {
    pub struct CmpTool;
    fqn = "xyz.taluslabs.math.i64.cmp@1";
    inputs {
        a: i64,
        b: i64,
    }
    outputs {
        lt { a: i64, b: i64 }
        gt { a: i64, b: i64 }
        eq { a: i64, b: i64 }
    }
}

/// The macro emits a [`ToolDescriptor`] impl usable in [`TypedDagBuilder`]
/// end-to-end: `add::<AddTool>("a")` yields typed port handles, `connect`
/// accepts them, `.build()` succeeds.
///
/// What breaks if this test is deleted: the declarative macro could
/// silently regress — the expanded code could stop producing the right
/// associated types, `inputs_for`/`outputs_for` could miswire port names,
/// or the generated structs could become private and unusable by callers.
#[test]
fn macro_descriptor_is_usable_in_typed_builder() {
    let mut dag = TypedDagBuilder::new();
    let a = dag.add::<AddTool>("a");
    let b = dag.add::<AddTool>("b");
    dag.entry_port(&a, "a");
    dag.inline_default(a.inp.b, 1i64);
    dag.inline_default(b.inp.b, 2i64);
    dag.connect(a.out.ok.result, b.inp.a);
    dag.output(b.out.ok.result);

    let built = dag
        .build()
        .expect("macro-generated descriptor builds a valid DAG");
    assert_eq!(built.vertices.len(), 2);
    assert_eq!(built.edges.len(), 1);

    // Sanity-check: fqn() returns what was declared in the macro.
    assert_eq!(AddTool::fqn().to_string(), "xyz.taluslabs.math.i64.add@1");
}

/// Tools with multiple output variants work — each variant is reachable as
/// a field on the emitted `<Tool>Outputs` struct, and each variant's ports
/// are typed `OutPort<Ok, T>`.
///
/// What breaks if this test is deleted: multi-variant emission could
/// silently regress to single-variant-only, breaking descriptors for
/// branching tools like `cmp` (which has lt / gt / eq branches).
#[test]
fn multi_variant_outputs_all_reachable() {
    let mut dag = TypedDagBuilder::new();
    let cmp = dag.add::<CmpTool>("cmp");
    let lt_sink = dag.add::<AddTool>("lt_sink");
    let gt_sink = dag.add::<AddTool>("gt_sink");
    let eq_sink = dag.add::<AddTool>("eq_sink");

    dag.entry_port(&cmp, "a");
    dag.inline_default(cmp.inp.b, 0i64);
    dag.inline_default(lt_sink.inp.b, 0i64);
    dag.inline_default(gt_sink.inp.b, 0i64);
    dag.inline_default(eq_sink.inp.b, 0i64);

    dag.connect(cmp.out.lt.a, lt_sink.inp.a);
    dag.connect(cmp.out.gt.a, gt_sink.inp.a);
    dag.connect(cmp.out.eq.a, eq_sink.inp.a);
    dag.output(lt_sink.out.ok.result);
    dag.output(gt_sink.out.ok.result);
    dag.output(eq_sink.out.ok.result);

    let built = dag.build().expect("multi-variant DAG builds");
    assert_eq!(built.edges.len(), 3);
    assert_eq!(built.outputs.as_ref().unwrap().len(), 3);
}
