//! `TypedForEachScope::consume(ItemPort<T>, InPort<U>)` where T != U must
//! fail to compile.
//!
//! What breaks if this file starts compiling: the typed scoped layer's
//! per-iteration type guarantee has regressed — an `ItemPort<i64>` could
//! be connected to an `InPort<String>`, meaning the destination vertex
//! receives values of a different type per iteration than what the
//! source emits.

use nexus_dag_dsl::{InPort, Ok as OkVariant, OutPort, ToolDescriptor, ToolFqn, TypedDagBuilder};
use std::str::FromStr;

struct Source;
struct SourceInputs;
struct SourceOutputs {
    ok: SourceOk,
}
struct SourceOk {
    items: OutPort<OkVariant, Vec<i64>>,
}

impl ToolDescriptor for Source {
    type Inputs = SourceInputs;
    type Outputs = SourceOutputs;
    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.fixtures.source@1").unwrap()
    }
    fn inputs_for(_: &str) -> Self::Inputs {
        SourceInputs
    }
    fn outputs_for(n: &str) -> Self::Outputs {
        SourceOutputs {
            ok: SourceOk {
                items: OutPort::new(n, "ok", "items"),
            },
        }
    }
}

struct Consumer;
struct ConsumerInputs {
    name: InPort<String>,
}
struct ConsumerOutputs;

impl ToolDescriptor for Consumer {
    type Inputs = ConsumerInputs;
    type Outputs = ConsumerOutputs;
    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.fixtures.consumer@1").unwrap()
    }
    fn inputs_for(n: &str) -> Self::Inputs {
        ConsumerInputs {
            name: InPort::new(n, "name"),
        }
    }
    fn outputs_for(_: &str) -> Self::Outputs {
        ConsumerOutputs
    }
}

fn main() {
    let mut dag = TypedDagBuilder::new();
    let src = dag.add::<Source>("src");
    dag.for_each(src.out.ok.items, |scope, item| {
        let c = scope.add::<Consumer>("c");
        // ItemPort<i64> into InPort<String>: must not compile.
        scope.consume(item, c.inp.name);
    });
}
