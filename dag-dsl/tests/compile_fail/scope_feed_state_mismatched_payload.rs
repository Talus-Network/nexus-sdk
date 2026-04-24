//! `TypedDoWhileScope::feed_state(StatePort<T>, InPort<U>)` where T != U
//! must fail to compile.
//!
//! What breaks if this file starts compiling: the typed do-while scope's
//! seed-edge type guarantee has regressed. A loop seeded with an i64
//! state could be connected to an InPort<String> state input, misrouting
//! the loop's initial value.

use nexus_dag_dsl::{InPort, Ok as OkVariant, OutPort, ToolDescriptor, ToolFqn, TypedDagBuilder};
use std::str::FromStr;

struct SeedTool;
struct SeedInputs;
struct SeedOutputs {
    ok: SeedOk,
}
struct SeedOk {
    result: OutPort<OkVariant, i64>,
}

impl ToolDescriptor for SeedTool {
    type Inputs = SeedInputs;
    type Outputs = SeedOutputs;
    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.fixtures.seed@1").unwrap()
    }
    fn inputs_for(_: &str) -> Self::Inputs {
        SeedInputs
    }
    fn outputs_for(n: &str) -> Self::Outputs {
        SeedOutputs {
            ok: SeedOk {
                result: OutPort::new(n, "ok", "result"),
            },
        }
    }
}

struct StepTool;
struct StepInputs {
    label: InPort<String>,
}
struct StepOutputs {
    ok: StepOk,
}
struct StepOk {
    label: OutPort<OkVariant, String>,
}

impl ToolDescriptor for StepTool {
    type Inputs = StepInputs;
    type Outputs = StepOutputs;
    fn fqn() -> ToolFqn {
        ToolFqn::from_str("xyz.fixtures.step@1").unwrap()
    }
    fn inputs_for(n: &str) -> Self::Inputs {
        StepInputs {
            label: InPort::new(n, "label"),
        }
    }
    fn outputs_for(n: &str) -> Self::Outputs {
        StepOutputs {
            ok: StepOk {
                label: OutPort::new(n, "ok", "label"),
            },
        }
    }
}

fn main() {
    let mut dag = TypedDagBuilder::new();
    let seed = dag.add::<SeedTool>("seed");
    // StatePort<i64> fed into InPort<String>: must not compile.
    dag.do_while(seed.out.ok.result, |scope, state| {
        let step = scope.add::<StepTool>("step");
        scope.feed_state(state, step.inp.label);
    });
}
