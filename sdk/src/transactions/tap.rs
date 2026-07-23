use {
    crate::{
        move_bindings::{
            interface::{
                agent as agent_binding,
                agent::{FixedTool, SkillSchedulePolicy},
                payment::{self as payment_binding, SkillPaymentPolicy},
            },
            registry::agent_registry as agent_registry_binding,
        },
        move_boundary,
        sui,
        transactions::agent_input::AgentInput,
        types::{NexusObjects, SkillId, TapPublishArtifact},
    },
    sui::types::ProgrammableTransaction,
};

pub(crate) fn agent_registry_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    mutability: bool,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let registry = &objects.agent_registry;

    Ok(tx.shared_object(registry, mutability)?)
}

fn tool_registry_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    mutability: bool,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let registry = &objects.tool_registry;

    Ok(tx.shared_object(registry, mutability)?)
}

pub(crate) fn create_agent(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    registry: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(agent_registry_binding::create_agent_target, vec![registry])
}

/// Build a PTB that publishes a TAP Move package and transfers the upgrade cap to `recipient`.
#[cfg(feature = "move_publish")]
pub(crate) fn publish_package_ptb(
    objects: &NexusObjects,
    modules: Vec<Vec<u8>>,
    dependencies: Vec<sui::types::Address>,
    recipient: sui::types::Address,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let upgrade_cap = tx.publish(modules, dependencies)?;
        let recipient = tx.arg(&recipient)?;
        tx.transfer_objects(vec![upgrade_cap], recipient)?;
        Ok(())
    })
}

/// Build a PTB that creates a standard TAP agent and transfers it to `address`.
pub(crate) fn create_agent_for_self_ptb(
    objects: &NexusObjects,
    address: sui::types::Address,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let registry = tx.shared_object(&objects.agent_registry, true)?;
        let agent = tx.call_target(agent_registry_binding::create_agent_target, vec![registry])?;
        let recipient = tx.arg(&address)?;
        tx.transfer_objects(vec![agent], recipient)?;
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn register_skill(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag: sui::types::Argument,
    description: Vec<u8>,
    input_commitment: Vec<u8>,
    payment_policy: SkillPaymentPolicy,
    schedule_policy: SkillSchedulePolicy,
    fixed_tools: Vec<FixedTool>,
) -> anyhow::Result<sui::types::Argument> {
    let tool_registry = tool_registry_arg(tx, false)?;
    let payment_policy = payment_policy_arg(tx, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, &schedule_policy)?;
    let fixed_tools = fixed_tools_arg(tx, &fixed_tools)?;
    let args = vec![
        registry,
        agent,
        tool_registry,
        dag,
        tx.arg(&description)?,
        tx.arg(&input_commitment)?,
        payment_policy,
        schedule_policy,
        fixed_tools,
    ];

    tx.call_target(agent_registry_binding::register_skill_target, args)
}

/// Build a PTB that registers a skill on an existing TAP agent.
pub(crate) fn register_skill_ptb(
    objects: &NexusObjects,
    agent: AgentInput,
    dag: &sui::types::ObjectReference,
    artifact: &TapPublishArtifact,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let registry = agent_registry_arg(tx, true)?;
        let agent = agent.mutable_ptb_argument(tx)?;
        let dag = tx.shared_object(dag, false)?;

        register_skill(
            tx,
            registry,
            agent,
            dag,
            artifact.skill_name.as_bytes().to_vec(),
            artifact.requirements.input_commitment.clone(),
            artifact.requirements.payment_policy,
            artifact.requirements.schedule_policy.clone(),
            artifact.requirements.fixed_tools.clone(),
        )?;
        Ok(())
    })
}

pub(crate) fn update_dag(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag: sui::types::Argument,
    skill_id: SkillId,
) -> anyhow::Result<sui::types::Argument> {
    let tool_registry = tool_registry_arg(tx, false)?;
    let args = vec![registry, agent, tool_registry, dag, tx.arg(&skill_id)?];

    tx.call_target(agent_registry_binding::update_dag_target, args)
}

pub(crate) fn update_skill_policies(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    payment_policy: SkillPaymentPolicy,
    schedule_policy: SkillSchedulePolicy,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.arg(&skill_id)?,
        payment_policy,
        schedule_policy,
    ];

    tx.call_target(agent_registry_binding::update_skill_policies_target, args)
}

/// Build a PTB that updates a skill's current DAG and policy contract from an artifact.
pub(crate) fn update_skill_from_artifact_ptb(
    objects: &NexusObjects,
    agent: AgentInput,
    dag: &sui::types::ObjectReference,
    skill_id: SkillId,
    artifact: &TapPublishArtifact,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let registry_for_dag = agent_registry_arg(tx, true)?;
        let registry_for_policies = agent_registry_arg(tx, true)?;
        let agent_for_dag = agent.clone().mutable_ptb_argument(tx)?;
        let agent_for_policies = agent.mutable_ptb_argument(tx)?;
        let dag = tx.shared_object(dag, false)?;

        update_dag(tx, registry_for_dag, agent_for_dag, dag, skill_id)?;

        update_skill_policies(
            tx,
            registry_for_policies,
            agent_for_policies,
            skill_id,
            artifact.requirements.payment_policy,
            artifact.requirements.schedule_policy.clone(),
        )?;
        Ok(())
    })
}

/// Builds a [`ProgrammableTransaction`] that deposits MIST from the sender's
/// address balance into an agent vault.
pub(crate) fn deposit_agent_payment_vault_for_self_ptb(
    objects: &NexusObjects,
    agent: AgentInput,
    amount: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let agent = agent.mutable_ptb_argument(tx)?;
        let coin = tx.withdraw_sui_coin(amount)?;
        tx.call_target(
            agent_binding::deposit_agent_payment_vault_target,
            vec![agent, coin],
        )?;
        Ok(())
    })
}

/// Build a PTB that creates an agent and registers its first skill atomically.
pub(crate) fn bind_agent_skill_ptb(
    objects: &NexusObjects,
    dag: &sui::types::ObjectReference,
    artifact: &TapPublishArtifact,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let registry = agent_registry_arg(tx, true)?;
        let agent = create_agent(tx, registry)?;
        let dag = tx.shared_object(dag, false)?;

        register_skill(
            tx,
            registry,
            agent,
            dag,
            artifact.skill_name.as_bytes().to_vec(),
            artifact.requirements.input_commitment.clone(),
            artifact.requirements.payment_policy,
            artifact.requirements.schedule_policy.clone(),
            artifact.requirements.fixed_tools.clone(),
        )?;
        Ok(())
    })
}

fn schedule_policy_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    schedule_policy: &SkillSchedulePolicy,
) -> anyhow::Result<sui::types::Argument> {
    match schedule_policy {
        SkillSchedulePolicy::Once => tx.call_target(agent_binding::schedule_once_target, vec![]),
        SkillSchedulePolicy::Recurring {
            min_interval_ms,
            max_occurrences,
        } => {
            let min_interval_ms = tx.arg(min_interval_ms)?;
            let max_occurrences = option_u64_arg(tx, max_occurrences.as_option())?;
            tx.call_target(
                agent_binding::schedule_recurring_target,
                vec![min_interval_ms, max_occurrences],
            )
        }
    }
}

fn option_u64_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    value: Option<&u64>,
) -> anyhow::Result<sui::types::Argument> {
    match value {
        Some(value) => {
            let value = tx.arg(value)?;
            Ok(tx.option::<u64>(Some(value))?)
        }
        None => Ok(tx.option::<u64>(None)?),
    }
}

fn payment_policy_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    payment_policy: &SkillPaymentPolicy,
) -> anyhow::Result<sui::types::Argument> {
    Ok(match payment_policy {
        SkillPaymentPolicy::UserFunded => {
            tx.call_target(payment_binding::payment_policy_user_funded_target, vec![])?
        }
        SkillPaymentPolicy::AgentFunded { max_budget_mist } => {
            let max_budget_mist = tx.arg(max_budget_mist)?;
            tx.call_target(
                payment_binding::payment_policy_agent_funded_target,
                vec![max_budget_mist],
            )?
        }
    })
}

fn fixed_tool_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    fixed_tool: &FixedTool,
) -> anyhow::Result<sui::types::Argument> {
    let tool_registry_id = tx.object_id(fixed_tool.tool_registry_address())?;
    let tool_fqn = tx.ascii_string(fixed_tool.tool_fqn_string())?;

    tx.call_target(
        agent_binding::fixed_tool_target,
        vec![tool_registry_id, tool_fqn],
    )
}

fn fixed_tools_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    fixed_tools: &[FixedTool],
) -> anyhow::Result<sui::types::Argument> {
    let fixed_tools = fixed_tools
        .iter()
        .map(|fixed_tool| fixed_tool_arg(tx, fixed_tool))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(tx.move_vector::<FixedTool>(fixed_tools)?)
}
