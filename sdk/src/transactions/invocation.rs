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
}

fn policy_target(
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
    vertex: &RuntimeVertex,
    policy: &InvocationPolicyCall,
) -> anyhow::Result<sui::types::Argument> {
    let vertex = super::dag::runtime_vertex_arg(transaction, vertex)?;
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
    transaction.call_target(
        invocation_adapter_binding::lock_target,
        vec![dag, execution, vertex, authorized],
    )
}

/// Builds a PTB that authorizes one exact Tool Invocation.
pub fn authorize_ptb(
    objects: &NexusObjects,
    cashier: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    vertex: &RuntimeVertex,
    policy: &InvocationPolicyCall,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let cashier = transaction.shared_object(cashier, false)?;
        let dag = transaction.shared_object(dag, false)?;
        let execution = transaction.shared_object(execution, true)?;
        authorize(transaction, cashier, dag, execution, vertex, policy)?;
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

/// Builds a permissionless timeout refund for one exact Invocation.
pub fn abort_expired_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
    vertex: &RuntimeVertex,
    invocation: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let dag = transaction.shared_object(dag, false)?;
        let execution = transaction.shared_object(execution, true)?;
        let vertex = super::dag::runtime_vertex_arg(transaction, vertex)?;
        let receiving = transaction.receiving_object::<Invocation>(invocation)?;
        let clock = transaction.clock()?;
        transaction.call_target(
            invocation_adapter_binding::abort_expired_target,
            vec![dag, execution, vertex, receiving, clock],
        )?;
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
        sui_sdk_types::{Command, Input},
    };

    fn vertex() -> RuntimeVertex {
        RuntimeVertex::plain("tool")
    }

    #[test]
    fn fixed_price_authorization_reads_cashier_and_calls_policy_before_lock() {
        let objects = mock_nexus_objects();
        let cashier = object_ref_for_id(sui::types::Address::from_static("0x81"));
        let dag = object_ref_for_id(sui::types::Address::from_static("0x82"));
        let execution = object_ref_for_id(sui::types::Address::from_static("0x83"));
        let ptb = authorize_ptb(
            &objects,
            &cashier,
            &dag,
            &execution,
            &vertex(),
            &InvocationPolicyCall::fixed_price(&objects),
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
        let lock = calls
            .iter()
            .position(|call| *call == ("invocation_adapter", "lock"))
            .unwrap();
        assert!(request < policy && policy < lock);
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
}
