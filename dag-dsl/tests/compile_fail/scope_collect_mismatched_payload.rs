//! `TypedForEachScope::collect(OutPort<V, T>, InPort<Vec<U>>)` where
//! T != U must fail to compile.
//!
//! What breaks if this file starts compiling: the scoped collect's
//! per-iteration-to-aggregate type guarantee has regressed. A loop body
//! emitting `i64` per iteration could be mistakenly piped into a
//! `Vec<String>` destination, producing a wire DAG the runtime would
//! reject only at execution time.

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

struct PerItem;
struct PerItemInputs {
    n: InPort<i64>,
}
struct PerItemOutputs {
    ok: PerItemOk,
}
struct PerItemOk {
    name: OutPort<OkVariant, i64>,
}

impl ToolDescriptor for PerItem {
    type Inputs = PerItemInputs;
    type Outputs = PerItemOutputs;
    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.fixtures.per_item@1").unwrap()
    }
    fn inputs_for(n: &str) -> Self::Inputs {
        PerItemInputs {
            n: InPort::new(n, "n"),
        }
    }
    fn outputs_for(n: &str) -> Self::Outputs {
        PerItemOutputs {
            ok: PerItemOk {
                name: OutPort::new(n, "ok", "name"),
            },
        }
    }
}

struct Aggregator;
struct AggregatorInputs {
    vec: InPort<Vec<String>>,
}
struct AggregatorOutputs;

impl ToolDescriptor for Aggregator {
    type Inputs = AggregatorInputs;
    type Outputs = AggregatorOutputs;
    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.fixtures.aggregator@1").unwrap()
    }
    fn inputs_for(n: &str) -> Self::Inputs {
        AggregatorInputs {
            vec: InPort::new(n, "vec"),
        }
    }
    fn outputs_for(_: &str) -> Self::Outputs {
        AggregatorOutputs
    }
}

fn main() {
    let mut dag = TypedDagBuilder::new();
    let src = dag.add::<Source>("src");
    let agg = dag.add::<Aggregator>("agg");
    dag.for_each(src.out.ok.items, |scope, item| {
        let per = scope.add::<PerItem>("per");
        scope.consume(item, per.inp.n);
        // per.out.ok.name is OutPort<_, i64>; agg.inp.vec is
        // InPort<Vec<String>>. i64 != String — must not compile.
        scope.collect(per.out.ok.name, agg.inp.vec);
    });
}
