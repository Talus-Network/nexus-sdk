//! `connect(out: OutPort<_, T>, inp: InPort<U>)` where T != U must fail.
//!
//! What breaks if this file starts compiling: the payload-type guarantee
//! of `TypedDagBuilder::connect` — the entire point of the strict layer —
//! has regressed. Mismatched port types would silently produce a wire DAG
//! whose runtime execution fails the way it would in the relaxed builder.

use nexus_dag_dsl::{
    InPort,
    Ok as OkVariant,
    OutPort,
    ToolDescriptor,
    ToolFqn,
    TypedDagBuilder,
};
use std::str::FromStr;

struct Src;
struct SrcInputs;
struct SrcOutputs {
    ok: SrcOk,
}
struct SrcOk {
    result: OutPort<OkVariant, i64>,
}

impl ToolDescriptor for Src {
    type Inputs = SrcInputs;
    type Outputs = SrcOutputs;
    fn fqn() -> ToolFqn { ToolFqn::from_str("xyz.fixtures.src@1").unwrap() }
    fn inputs_for(_: &str) -> Self::Inputs { SrcInputs }
    fn outputs_for(n: &str) -> Self::Outputs {
        SrcOutputs { ok: SrcOk { result: OutPort::new(n, "ok", "result") } }
    }
}

struct Dst;
struct DstInputs {
    value: InPort<String>,
}
struct DstOutputs;

impl ToolDescriptor for Dst {
    type Inputs = DstInputs;
    type Outputs = DstOutputs;
    fn fqn() -> ToolFqn { ToolFqn::from_str("xyz.fixtures.dst@1").unwrap() }
    fn inputs_for(n: &str) -> Self::Inputs {
        DstInputs { value: InPort::new(n, "value") }
    }
    fn outputs_for(_: &str) -> Self::Outputs { DstOutputs }
}

fn main() {
    let mut dag = TypedDagBuilder::new();
    let s = dag.add::<Src>("s");
    let d = dag.add::<Dst>("d");
    // i64 → String: must not compile.
    dag.connect(s.out.ok.result, d.inp.value);
}
