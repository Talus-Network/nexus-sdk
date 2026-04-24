//! `TypedDagBuilder::inline_default(port: InPort<T>, value: T)` must reject
//! a value whose Rust type differs from the port's declared payload type.
//!
//! What breaks if this file starts compiling: users could set a default
//! value whose type does not match the port, which would only surface as
//! a runtime JSON schema failure during DAG execution — exactly the kind
//! of late error the strict layer exists to prevent.

use nexus_dag_dsl::{
    InPort,
    Ok as OkVariant,
    OutPort,
    ToolDescriptor,
    ToolFqn,
    TypedDagBuilder,
};
use std::str::FromStr;

struct NumberTool;
struct NumberInputs {
    n: InPort<i64>,
}
struct NumberOutputs {
    ok: NumberOk,
}
struct NumberOk {
    result: OutPort<OkVariant, i64>,
}

impl ToolDescriptor for NumberTool {
    type Inputs = NumberInputs;
    type Outputs = NumberOutputs;
    fn fqn() -> ToolFqn { ToolFqn::from_str("xyz.fixtures.number@1").unwrap() }
    fn inputs_for(n: &str) -> Self::Inputs {
        NumberInputs { n: InPort::new(n, "n") }
    }
    fn outputs_for(n: &str) -> Self::Outputs {
        NumberOutputs { ok: NumberOk { result: OutPort::new(n, "ok", "result") } }
    }
}

fn main() {
    let mut dag = TypedDagBuilder::new();
    let t = dag.add::<NumberTool>("t");
    // InPort<i64> with a &str default: must not compile.
    dag.inline_default(t.inp.n, "not an integer");
}
