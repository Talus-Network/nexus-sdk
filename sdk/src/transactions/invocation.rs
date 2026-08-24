//! Exact Tool Invocation authorization and settlement transactions.

use {
    crate::{
        move_bindings::{
            interface::graph::RuntimeVertex,
            move_std::type_name::TypeName,
            tool::invocation::Invocation,
            workflow::invocation_adapter as invocation_adapter_binding,
        },
        move_boundary,
        sui,
        transactions::dag::OnchainToolArgument,
        types::NexusObjects,
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

impl InvocationPolicyCall {
    /// Creates an arbitrary policy call from its witness [TypeName].
    pub fn new(policy: TypeName, arguments: Vec<OnchainToolArgument>) -> Self {
        Self { policy, arguments }
    }

    /// Selects the canonical fixed price policy for this Nexus deployment.
    pub fn fixed_price(objects: &NexusObjects) -> Self {
        let origin = objects.packages.tool.type_origin("fixed_price", "Policy");
        Self::new(
            TypeName::new(&format!("{origin}::fixed_price::Policy")),
            Vec::new(),
        )
    }

    /// Selects the canonical sponsored free policy for this Nexus deployment.
    pub fn free_invocation(objects: &NexusObjects) -> Self {
        let origin = objects
            .packages
            .tool
            .type_origin("free_invocation", "Policy");
        Self::new(
            TypeName::new(&format!("{origin}::free_invocation::Policy")),
            Vec::new(),
        )
    }

    /// Selects the canonical finite credit policy with one mutable account.
    pub fn finite_credits(
        objects: &NexusObjects,
        credits: sui::types::Address,
        initial_shared_version: sui::types::Version,
    ) -> Self {
        Self::new(
            Self::finite_credits_policy(objects),
            vec![OnchainToolArgument::SharedObject {
                object_id: credits,
                initial_shared_version,
                mutable: true,
            }],
        )
    }

    /// Returns the canonical finite credits witness [TypeName].
    pub fn finite_credits_policy(objects: &NexusObjects) -> TypeName {
        let origin = objects
            .packages
            .tool
            .type_origin("finite_credits", "Policy");
        TypeName::new(&format!("{origin}::finite_credits::Policy"))
    }

    /// Selects the canonical time pass policy with one read only shared account.
    pub fn time_pass(
        objects: &NexusObjects,
        pass: sui::types::Address,
        initial_shared_version: sui::types::Version,
    ) -> Self {
        let origin = objects.packages.tool.type_origin("time_pass", "Policy");
        Self::new(
            TypeName::new(&format!("{origin}::time_pass::Policy")),
            vec![OnchainToolArgument::SharedObject {
                object_id: pass,
                initial_shared_version,
                mutable: false,
            }],
        )
    }
}

pub(crate) fn policy_target(
    objects: &NexusObjects,
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
    let call_package = if objects.is_tool_package(defining_package) {
        objects.tool_pkg_id()
    } else {
        defining_package
    };
    Ok((call_package, module.to_owned()))
}

/// Appends exact Invocation authorization to an existing PTB.
pub fn authorize(
    transaction: &mut move_boundary::NexusPtbBuilder,
    cashier: sui::types::Argument,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    leader_registry: sui::types::Argument,
    target: InvocationTarget<'_>,
    policy: &InvocationPolicyCall,
    submission_gas_charge: u64,
) -> anyhow::Result<sui::types::Argument> {
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
    let (package, module) = policy_target(transaction.objects(), &policy.policy)?;
    let authorized =
        transaction.call_function(package, module, "get_invocation", policy_arguments)?;
    let walk_index = transaction.arg(&target.walk_index)?;
    let submission_gas_charge = transaction.arg(&submission_gas_charge)?;
    transaction.call_target(
        invocation_adapter_binding::lock_and_request_target,
        vec![
            dag,
            execution,
            leader_registry,
            walk_index,
            vertex,
            authorized,
            submission_gas_charge,
            clock,
        ],
    )
}

/// Builds a PTB that authorizes one exact Tool Invocation.
///
/// `submission_gas_charge` reimburses the transaction sender from the
/// execution payment. A user submitting for itself should pass zero.
pub fn authorize_ptb(
    objects: &NexusObjects,
    cashier: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    leader_registry: &sui::types::ObjectReference,
    target: InvocationTarget<'_>,
    policy: &InvocationPolicyCall,
    submission_gas_charge: u64,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(cashier, false)?;
        let dag = transaction.shared_object(dag, false)?;
        let execution = transaction.shared_object(execution, true)?;
        let leader_registry = transaction.shared_object(leader_registry, false)?;
        authorize(
            transaction,
            cashier,
            dag,
            execution,
            leader_registry,
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
    transaction.call_target(
        invocation_adapter_binding::settle_target,
        vec![dag, execution, vertex, receiving],
    )
}

/// Builds a permissionless timeout refund for one exact [Invocation].
///
/// When `task_settlement` is supplied, the owning Task is settled only after
/// the Invocation refund has removed its accounting lock.
pub fn abort_expired_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    vertex: &RuntimeVertex,
    invocation: &sui::types::ObjectReference,
    task_settlement: Option<&sui::types::ObjectReference>,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let dag = transaction.shared_object(dag, false)?;
        let execution = transaction.shared_object(execution, true)?;
        let leader_registry = transaction.shared_object(&objects.leader_registry, false)?;
        let vertex = super::dag::runtime_vertex_arg(transaction, vertex)?;
        let receiving = transaction.receiving_object::<Invocation>(invocation)?;
        let clock = transaction.clock()?;
        transaction.call_target(
            invocation_adapter_binding::abort_expired_target,
            vec![dag, execution, vertex, receiving, clock],
        )?;
        if let Some(task) = task_settlement {
            super::scheduler::append_settle_occurrence(
                transaction,
                task,
                execution,
                leader_registry,
                clock,
            )?;
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
            test_utils::sui_mocks::{mock_nexus_objects, object_ref_for_id},
        },
        sui_sdk_types::{Argument, Command, Input},
    };

    fn vertex() -> RuntimeVertex {
        RuntimeVertex::plain("tool")
    }

    #[test]
    fn fixed_price_authorization_calls_policy_before_lock_and_request() {
        let objects = mock_nexus_objects();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0x81"));
        let dag = object_ref_for_id(sui::types::Address::from_static("0x82"));
        let execution = object_ref_for_id(sui::types::Address::from_static("0x83"));
        let leader_registry = object_ref_for_id(sui::types::Address::from_static("0x84"));
        let ptb = authorize_ptb(
            &objects,
            &cashier,
            &dag,
            &execution,
            &leader_registry,
            InvocationTarget {
                walk_index: 0,
                vertex: &vertex(),
            },
            &InvocationPolicyCall::fixed_price(&objects),
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
        assert_eq!(lock_and_request.arguments.len(), 8);
        let Argument::Input(gas_charge) = lock_and_request.arguments[6] else {
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
        let objects = mock_nexus_objects();
        let policy = TypeName::new("0x71::custom_terms::Policy");
        assert_eq!(
            policy_target(&objects, &policy).unwrap(),
            (
                sui::types::Address::from_static("0x71"),
                "custom_terms".to_owned()
            )
        );
    }

    #[test]
    fn finite_credits_selects_one_mutable_shared_object() {
        let objects = mock_nexus_objects();
        let credits = sui::types::Address::from_static("0x91");
        let initial_shared_version = 7;

        let policy =
            InvocationPolicyCall::finite_credits(&objects, credits, initial_shared_version);

        assert!(policy.policy.as_str().ends_with("::finite_credits::Policy"));
        assert_eq!(
            policy.arguments,
            vec![OnchainToolArgument::SharedObject {
                object_id: credits,
                initial_shared_version,
                mutable: true,
            }]
        );
    }

    #[test]
    fn time_pass_selects_one_read_only_shared_object() {
        let objects = mock_nexus_objects();
        let pass = sui::types::Address::from_static("0x92");

        let policy = InvocationPolicyCall::time_pass(&objects, pass, 7);

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
        let objects = mock_nexus_objects();
        let dag = object_ref_for_id(sui::types::Address::from_static("0x82"));
        let execution = object_ref_for_id(sui::types::Address::from_static("0x83"));
        let invocation = object_ref_for_id(sui::types::Address::from_static("0x84"));
        let task = object_ref_for_id(sui::types::Address::from_static("0x85"));
        let ptb = abort_expired_ptb(
            &objects,
            &dag,
            &execution,
            &vertex(),
            &invocation,
            Some(&task),
        )
        .expect("ptb should build");
        let calls = ptb
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::MoveCall(call) => Some((call.module.as_str(), call.function.as_str())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let refund = calls
            .iter()
            .position(|call| *call == ("invocation_adapter", "abort_expired"))
            .expect("Invocation timeout refund should be present");
        let task = calls
            .iter()
            .position(|call| *call == ("scheduler", "settle"))
            .expect("Task settlement should be present");

        assert!(refund < task);
    }
}
