//! Exact Tool Invocation authorization and settlement transactions.

use {
    crate::{
        move_bindings::{
            interface::graph::RuntimeVertex,
            move_std::type_name::TypeName,
            scheduler::invocation_adapter as scheduler_invocation_adapter_binding,
            tool::invocation::Invocation,
            workflow::invocation_adapter as invocation_adapter_binding,
        },
        move_boundary,
        sui,
        transactions::dag::OnchainToolArgument,
        types::{NexusContext, PackageRole},
    },
    std::str::FromStr,
};

/// One transient call to a Tool owner's Invocation policy.
///
/// Generated call targets cannot name owner policy modules selected at runtime,
/// so this value carries the exact policy type and its additional arguments.
/// The witness [TypeName] identifies the package and module. Every policy
/// exposes `get_invocation`. Arguments contain only the policy specific values;
/// the canonical cashier and request are inserted by the composer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationPolicyCall {
    pub policy: TypeName,
    pub arguments: Vec<OnchainToolArgument>,
}

/// Exact active walk position whose Tool Invocation is being authorized.
#[derive(Clone, Copy, Debug)]
pub struct InvocationTarget<'a> {
    pub walk_index: u64,
    pub vertex: &'a RuntimeVertex,
}

fn policy_type_name(context: &NexusContext, module: &str) -> anyhow::Result<TypeName> {
    let origin = context.type_origin(PackageRole::Tool, module, "Policy")?;
    Ok(TypeName::new(&format!(
        "{}::{module}::Policy",
        hex::encode(origin.as_bytes()),
    )))
}

impl InvocationPolicyCall {
    /// Creates an arbitrary policy call from its witness [TypeName].
    pub fn new(policy: TypeName, arguments: Vec<OnchainToolArgument>) -> Self {
        Self { policy, arguments }
    }

    /// Selects the canonical fixed price policy for this Nexus deployment.
    pub fn fixed_price(context: &NexusContext) -> anyhow::Result<Self> {
        Ok(Self::new(
            policy_type_name(context, "fixed_price")?,
            Vec::new(),
        ))
    }

    /// Selects the canonical finite credit policy with one mutable account.
    pub fn finite_credits(
        context: &NexusContext,
        credits: sui::types::Address,
        initial_shared_version: sui::types::Version,
    ) -> anyhow::Result<Self> {
        Ok(Self::new(
            Self::finite_credits_policy(context)?,
            vec![OnchainToolArgument::SharedObject {
                object_id: credits,
                initial_shared_version,
                mutable: true,
            }],
        ))
    }

    /// Returns the canonical finite credits witness [TypeName].
    pub fn finite_credits_policy(context: &NexusContext) -> anyhow::Result<TypeName> {
        policy_type_name(context, "finite_credits")
    }

    /// Selects the canonical time pass policy with one read only shared account.
    pub fn time_pass(
        context: &NexusContext,
        pass: sui::types::Address,
        initial_shared_version: sui::types::Version,
    ) -> anyhow::Result<Self> {
        Ok(Self::new(
            policy_type_name(context, "time_pass")?,
            vec![OnchainToolArgument::SharedObject {
                object_id: pass,
                initial_shared_version,
                mutable: false,
            }],
        ))
    }
}

pub(crate) fn policy_target(
    context: &NexusContext,
    policy: &TypeName,
) -> anyhow::Result<(sui::types::Address, String)> {
    let mut parts = policy.as_str().split("::");
    let package = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invocation policy TypeName has no package"))?;
    let module = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invocation policy TypeName has no module"))?;
    let witness = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invocation policy TypeName has no witness"))?;
    anyhow::ensure!(
        parts.next().is_none() && witness == "Policy",
        "Invocation policy TypeName must be '<package>::<module>::Policy'"
    );
    let defining_package = sui::types::Address::from_str(package).map_err(|error| {
        anyhow::anyhow!("Invalid Invocation policy package '{package}': {error}")
    })?;
    let tool_package = context.require_package(PackageRole::Tool)?;
    let call_package = if tool_package.contains_package(defining_package) {
        tool_package.storage_id
    } else {
        defining_package
    };
    Ok((call_package, module.to_owned()))
}

fn prepare_authorization(
    transaction: &mut move_boundary::NexusPtbBuilder,
    cashier: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    target: InvocationTarget<'_>,
    policy: &InvocationPolicyCall,
) -> anyhow::Result<(
    sui::types::Argument,
    sui::types::Argument,
    sui::types::Argument,
    sui::types::Argument,
)> {
    let vertex = super::dag::runtime_vertex_arg(transaction, target.vertex)?;
    let clock = transaction.clock()?;
    let request = transaction.call_target(
        invocation_adapter_binding::new_request_target,
        vec![cashier, dag, execution, vertex, clock],
    )?;
    let mut policy_arguments = Vec::with_capacity(policy.arguments.len() + 2);
    policy_arguments.push(cashier);
    policy_arguments.extend(
        policy
            .arguments
            .iter()
            .map(|argument| {
                super::dag::prepare_onchain_tool_argument(
                    transaction,
                    argument,
                    &Default::default(),
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()?,
    );
    policy_arguments.push(request);
    let (package, module) = policy_target(transaction.context(), &policy.policy)?;
    let authorized =
        transaction.call_function(package, module, "get_invocation", policy_arguments)?;
    let walk_index = transaction.arg(&target.walk_index)?;
    Ok((walk_index, vertex, authorized, clock))
}

/// Appends one [Invocation] admission authorized by a leader.
///
/// The capability guards admission. The verified gas charge reimburses that leader.
#[allow(clippy::too_many_arguments)]
pub fn authorize(
    transaction: &mut move_boundary::NexusPtbBuilder,
    cashier: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    leader_registry: sui::types::Argument,
    leader_cap: sui::types::Argument,
    target: InvocationTarget<'_>,
    policy: &InvocationPolicyCall,
    submission_gas_charge: u64,
) -> anyhow::Result<sui::types::Argument> {
    let (walk_index, vertex, authorized, clock) =
        prepare_authorization(transaction, cashier, dag, execution, target, policy)?;
    let submission_gas_charge = transaction.arg(&submission_gas_charge)?;
    let runtime_authority = transaction.runtime_authority(false)?;
    transaction.call_target(
        scheduler_invocation_adapter_binding::lock_and_request_target,
        vec![
            runtime_authority,
            dag,
            execution,
            leader_registry,
            leader_cap,
            walk_index,
            vertex,
            authorized,
            submission_gas_charge,
            clock,
        ],
    )
}

/// Builds one Tool [Invocation] admission authorized by a leader.
///
/// Sui checks capability control. The gas charge must be verified by the leader.
#[allow(clippy::too_many_arguments)]
pub fn authorize_ptb(
    context: &NexusContext,
    cashier: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    target: InvocationTarget<'_>,
    policy: &InvocationPolicyCall,
    submission_gas_charge: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(context, |transaction| {
        let cashier = transaction.shared_object(cashier, false)?;
        let dag = transaction.immutable_object(dag)?;
        let execution = transaction.shared_object(execution, true)?;
        let leader_registry = transaction.shared_root(&context.leader_registry, false)?;
        let leader_cap = transaction.shared_object(leader_cap, false)?;
        authorize(
            transaction,
            cashier,
            dag,
            execution,
            leader_registry,
            leader_cap,
            target,
            policy,
            submission_gas_charge,
        )?;
        Ok(())
    })
}

/// Appends settlement of one exact Invocation to an existing PTB.
pub(crate) fn settle(
    transaction: &mut move_boundary::NexusPtbBuilder,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    vertex: &RuntimeVertex,
    invocation: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = super::dag::runtime_vertex_arg(transaction, vertex)?;
    let receiving = transaction.receiving_object::<Invocation>(invocation)?;
    let runtime_authority = transaction.runtime_authority(false)?;
    transaction.call_target(
        scheduler_invocation_adapter_binding::settle_target,
        vec![runtime_authority, dag, execution, vertex, receiving],
    )
}

/// Builds a permissionless timeout refund for one exact [Invocation].
///
/// When `task_settlement` is supplied, the owning Task is settled only after
/// the Invocation refund has removed its accounting lock.
pub fn abort_expired_ptb(
    context: &NexusContext,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    vertex: &RuntimeVertex,
    invocation: &sui::types::ObjectReference,
    task_settlement: Option<&sui::types::ObjectReference>,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(context, |transaction| {
        let dag = transaction.immutable_object(dag)?;
        let execution = transaction.shared_object(execution, true)?;
        let vertex = super::dag::runtime_vertex_arg(transaction, vertex)?;
        let receiving = transaction.receiving_object::<Invocation>(invocation)?;
        let clock = transaction.clock()?;
        let runtime_authority = transaction.runtime_authority(false)?;
        transaction.call_target(
            scheduler_invocation_adapter_binding::abort_expired_target,
            vec![runtime_authority, dag, execution, vertex, receiving, clock],
        )?;
        if let Some(task) = task_settlement {
            super::scheduler::append_settle_occurrence(transaction, task, execution, clock)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::interface::graph::RuntimeVertex,
            test_utils::sui_mocks::{mock_nexus_context, object_ref_for_id},
        },
        sui_sdk_types::{Argument, Command, Input},
    };

    fn vertex() -> RuntimeVertex {
        RuntimeVertex::plain("tool")
    }

    #[test]
    fn leader_authorization_calls_policy_before_lock_and_request() {
        let context = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0x81"));
        let dag = object_ref_for_id(sui::types::Address::from_static("0x82"));
        let execution = object_ref_for_id(sui::types::Address::from_static("0x83"));
        let leader_cap = object_ref_for_id(sui::types::Address::from_static("0x84"));
        let policy = InvocationPolicyCall::fixed_price(&context).unwrap();
        let ptb = authorize_ptb(
            &context,
            &cashier,
            &dag,
            &execution,
            &leader_cap,
            InvocationTarget {
                walk_index: 0,
                vertex: &vertex(),
            },
            &policy,
            42,
        )
        .unwrap();

        let Input::Shared(shared) = &ptb.inputs[0] else {
            panic!("cashier must be a shared input")
        };
        assert!(
            !shared.mutability().is_mutable(),
            "Invocation admission must not mutate ToolCashier"
        );
        let Input::ImmutableOrOwned(dag_input) = &ptb.inputs[1] else {
            panic!("finalized DAG must be an immutable input")
        };
        assert_eq!(dag_input.object_id(), dag.object_id());
        let calls = ptb
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::MoveCall(call) => Some((call.module.as_str(), call.function.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let request = calls
            .iter()
            .position(|call| *call == ("invocation_adapter", "new_request"))
            .unwrap();
        let policy = calls
            .iter()
            .position(|call| *call == ("fixed_price", "get_invocation"))
            .unwrap();
        let lock_and_request = calls
            .iter()
            .position(|call| *call == ("invocation_adapter", "lock_and_request"))
            .unwrap();
        assert!(request < policy && policy < lock_and_request);

        let lock_and_request = ptb
            .commands
            .iter()
            .find_map(|command| match command {
                Command::MoveCall(call)
                    if call.module.as_str() == "invocation_adapter"
                        && call.function.as_str() == "lock_and_request" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("lock and request call");
        assert_eq!(lock_and_request.arguments.len(), 10);
    }

    #[test]
    fn leader_authorization_requires_capability_before_reimbursement_charge() {
        let context = mock_nexus_context();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0x81"));
        let dag = object_ref_for_id(sui::types::Address::from_static("0x82"));
        let execution = object_ref_for_id(sui::types::Address::from_static("0x83"));
        let leader_cap = object_ref_for_id(sui::types::Address::from_static("0x84"));
        let policy = InvocationPolicyCall::fixed_price(&context).unwrap();
        let ptb = authorize_ptb(
            &context,
            &cashier,
            &dag,
            &execution,
            &leader_cap,
            InvocationTarget {
                walk_index: 0,
                vertex: &vertex(),
            },
            &policy,
            42,
        )
        .unwrap();

        let call = ptb
            .commands
            .iter()
            .find_map(|command| match command {
                Command::MoveCall(call)
                    if call.module.as_str() == "invocation_adapter"
                        && call.function.as_str() == "lock_and_request" =>
                {
                    Some(call)
                }
                _ => None,
            })
            .expect("leader reimbursement call");
        assert_eq!(call.arguments.len(), 10);

        let Argument::Input(leader_cap_input) = call.arguments[4] else {
            panic!("leader capability must be an object input")
        };
        let Input::Shared(shared) = &ptb.inputs[usize::from(leader_cap_input)] else {
            panic!("leader capability must preserve its consensus owner")
        };
        assert_eq!(shared.object_id(), *leader_cap.object_id());

        let Argument::Input(gas_charge) = call.arguments[8] else {
            panic!("gas charge must be a pure input")
        };
        let Input::Pure(gas_charge) = &ptb.inputs[usize::from(gas_charge)] else {
            panic!("gas charge must be a pure input")
        };
        assert_eq!(
            u64::from_le_bytes(gas_charge.as_slice().try_into().unwrap()),
            42,
        );
    }

    #[test]
    fn policy_witness_selects_runtime_module() {
        let context = mock_nexus_context();
        let policy = TypeName::new("0x71::custom_terms::Policy");
        assert_eq!(
            policy_target(&context, &policy).unwrap(),
            (
                sui::types::Address::from_static("0x71"),
                "custom_terms".to_owned()
            )
        );
    }

    #[test]
    fn finite_credits_selects_one_mutable_shared_object() {
        let context = mock_nexus_context();
        let credits = sui::types::Address::from_static("0x91");
        let initial_shared_version = 7;

        let policy =
            InvocationPolicyCall::finite_credits(&context, credits, initial_shared_version)
                .unwrap();

        assert!(policy.policy.as_str().ends_with("::finite_credits::Policy"));
        assert_eq!(
            policy.arguments,
            vec![OnchainToolArgument::SharedObject {
                object_id: credits,
                initial_shared_version,
                mutable: true,
            }]
        );
        let origin = context
            .type_origin(PackageRole::Tool, "finite_credits", "Policy")
            .unwrap();
        assert_eq!(
            policy.policy.as_str(),
            format!("{}::finite_credits::Policy", hex::encode(origin.as_bytes()))
        );
    }

    #[test]
    fn time_pass_selects_one_read_only_shared_object() {
        let context = mock_nexus_context();
        let pass = sui::types::Address::from_static("0x92");

        let policy = InvocationPolicyCall::time_pass(&context, pass, 7).unwrap();

        assert!(policy.policy.as_str().ends_with("::time_pass::Policy"));
        assert_eq!(
            policy.arguments,
            vec![OnchainToolArgument::SharedObject {
                object_id: pass,
                initial_shared_version: 7,
                mutable: false,
            }]
        );
    }

    #[test]
    fn timeout_refund_settles_the_task_after_the_invocation() {
        let context = mock_nexus_context();
        let dag = object_ref_for_id(sui::types::Address::from_static("0x82"));
        let execution = object_ref_for_id(sui::types::Address::from_static("0x83"));
        let invocation = object_ref_for_id(sui::types::Address::from_static("0x84"));
        let task = object_ref_for_id(sui::types::Address::from_static("0x85"));
        let ptb = abort_expired_ptb(
            &context,
            &dag,
            &execution,
            &vertex(),
            &invocation,
            Some(&task),
        )
        .expect("ptb should build");
        let Input::ImmutableOrOwned(dag_input) = &ptb.inputs[0] else {
            panic!("finalized DAG must be an immutable input")
        };
        assert_eq!(dag_input.object_id(), dag.object_id());
        let refund = ptb
            .commands
            .iter()
            .position(|command| match command {
                Command::MoveCall(call) => {
                    call.module.as_str() == "invocation_adapter"
                        && call.function.as_str() == "abort_expired"
                }
                _ => false,
            })
            .expect("Invocation timeout refund should be present");
        let task = ptb
            .commands
            .iter()
            .position(|command| match command {
                Command::MoveCall(call) => {
                    call.module.as_str() == "scheduler" && call.function.as_str() == "settle"
                }
                _ => false,
            })
            .expect("Task settlement should be present");

        assert!(refund < task);

        let Command::MoveCall(task_settlement) = &ptb.commands[task] else {
            panic!("expected task settlement Move call");
        };
        let Argument::Input(clock_index) = task_settlement.arguments[3] else {
            panic!("expected Clock input argument");
        };
        let Input::Shared(clock) = &ptb.inputs[usize::from(clock_index)] else {
            panic!("expected Clock shared object input");
        };
        assert_eq!(clock.object_id(), move_boundary::CLOCK_OBJECT_ID);
        assert!(!clock.mutability().is_mutable());
    }
}
