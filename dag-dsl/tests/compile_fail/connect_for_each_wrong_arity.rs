//! `connect_for_each(out: OutPort<_, Vec<T>>, inp: InPort<T>)` must reject
//! a scalar source — i.e., passing `OutPort<_, T>` instead of
//! `OutPort<_, Vec<T>>` is a compile error.
//!
//! What breaks if this file starts compiling: the for-each kind-specific
//! arity guarantee has regressed and users could silently build DAGs where
//! a non-list source is being iterated over, which the wire-level validator
//! would catch only later with a less-localized error.

use nexus_dag_dsl::{
    InPort,
    Ok as OkVariant,
    OutPort,
    ToolDescriptor,
    ToolFqn,
    TypedDagBuilder,
};
use std::str::FromStr;

struct Scalar;
struct ScalarInputs;
struct ScalarOutputs {
    ok: ScalarOk,
}
struct ScalarOk {
    // NB: scalar i64, not Vec<i64>.
    result: OutPort<OkVariant, i64>,
}

impl ToolDescriptor for Scalar {
    type Inputs = ScalarInputs;
    type Outputs = ScalarOutputs;
    fn fqn() -> ToolFqn { ToolFqn::from_str("xyz.fixtures.scalar@1").unwrap() }
    fn inputs_for(_: &str) -> Self::Inputs { ScalarInputs }
    fn outputs_for(n: &str) -> Self::Outputs {
        ScalarOutputs { ok: ScalarOk { result: OutPort::new(n, "ok", "result") } }
    }
}

struct Consumer;
struct ConsumerInputs {
    item: InPort<i64>,
}
struct ConsumerOutputs;

impl ToolDescriptor for Consumer {
    type Inputs = ConsumerInputs;
    type Outputs = ConsumerOutputs;
    fn fqn() -> ToolFqn { ToolFqn::from_str("xyz.fixtures.consumer@1").unwrap() }
    fn inputs_for(n: &str) -> Self::Inputs {
        ConsumerInputs { item: InPort::new(n, "item") }
    }
    fn outputs_for(_: &str) -> Self::Outputs { ConsumerOutputs }
}

fn main() {
    let mut dag = TypedDagBuilder::new();
    let s = dag.add::<Scalar>("s");
    let c = dag.add::<Consumer>("c");
    // Scalar i64 → for-each into i64: must not compile
    // (source must be Vec<i64>).
    dag.connect_for_each(s.out.ok.result, c.inp.item);
}
