use {
    crate::{
        move_bindings::{
            interface::{
                agent as agent_binding,
                agent::{FixedTool, SkillRecurrenceKind, SkillSchedulePolicy},
                authorization::{self as authorization_binding, AgentVertexAuthorizationTemplate},
                payment::{self as payment_binding, SkillPaymentPolicy},
            },
            primitives::data::NexusData,
            registry::agent_registry as agent_registry_binding,
            scheduler::scheduler as scheduler_binding,
            sui_framework::{object::ID as MoveObjectId, transfer as transfer_binding},
        },
        move_boundary,
        sui,
        transactions::{agent_input::AgentInput, scheduler::OccurrenceGenerator},
        types::{
            effective_priority_fee_percentage,
            AgentId,
            NexusObjects,
            SkillId,
            TapPublishArtifact,
        },
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
            artifact.requirements.payment_policy.clone(),
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
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
        )?;
        Ok(())
    })
}

/// Build a PTB that deposits MIST from the transaction gas coin into an agent vault.
pub(crate) fn deposit_agent_payment_vault_for_self_ptb(
    objects: &NexusObjects,
    agent: AgentInput,
    amount: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let agent = agent.mutable_ptb_argument(tx)?;
        let amount = tx.arg(&amount)?;
        let gas = tx.gas();
        let coin = tx.split_coins(gas, vec![amount])?;
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
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
            artifact.requirements.fixed_tools.clone(),
        )?;
        Ok(())
    })
}

#[derive(Clone, Debug)]
pub(crate) enum AgentTaskPaymentPtbInput {
    UserFunded {
        prepay_amount_mist: u64,
        refund_recipient: Option<sui::types::Address>,
        occurrence_budget_mist: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
    AgentVault {
        prepay_amount_mist: u64,
        occurrence_budget_mist: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_agent_task_ptb(
    objects: &NexusObjects,
    sender: sui::types::Address,
    metadata: &[(String, String)],
    generator: OccurrenceGenerator,
    priority_fee_percentage: Option<u64>,
    entry_group: &str,
    input_data: &std::collections::HashMap<String, std::collections::HashMap<String, NexusData>>,
    agent_id: AgentId,
    agent: AgentInput,
    skill_id: SkillId,
    payment: &AgentTaskPaymentPtbInput,
) -> anyhow::Result<ProgrammableTransaction> {
    let execution_selected_dag = match payment {
        AgentTaskPaymentPtbInput::UserFunded { selected_dag, .. }
        | AgentTaskPaymentPtbInput::AgentVault { selected_dag, .. } => *selected_dag,
    };

    move_boundary::ptb(objects, |tx| {
        let metadata = crate::transactions::scheduler::new_metadata(tx, metadata.iter().cloned())?;
        let constraints = crate::transactions::scheduler::new_constraints_policy(tx, generator)?;
        let execution = crate::transactions::scheduler::new_agent_execution_policy(
            tx,
            priority_fee_percentage,
            entry_group,
            input_data,
            agent_id,
            skill_id,
            execution_selected_dag,
        )?;
        let registry = tx.shared_object(&objects.agent_registry, true)?;

        let task = match payment {
            AgentTaskPaymentPtbInput::UserFunded {
                prepay_amount_mist,
                refund_recipient,
                occurrence_budget_mist,
                selected_dag,
                authorization_templates,
            } => {
                let agent = agent.clone().immutable_ptb_argument(tx)?;
                let prepay_amount_mist = tx.arg(prepay_amount_mist)?;
                let gas = tx.gas();
                let prepayment_coin = tx.split_coins(gas, vec![prepay_amount_mist])?;
                new_invoker_funded_agent_task(
                    tx,
                    metadata,
                    constraints,
                    execution,
                    registry,
                    agent,
                    agent_id,
                    priority_fee_percentage,
                    entry_group,
                    input_data,
                    skill_id,
                    *selected_dag,
                    prepayment_coin,
                    refund_recipient.unwrap_or(sender),
                    *occurrence_budget_mist,
                    authorization_templates.clone(),
                )?
            }
            AgentTaskPaymentPtbInput::AgentVault {
                prepay_amount_mist,
                occurrence_budget_mist,
                selected_dag,
                authorization_templates,
            } => {
                let agent = agent.clone().mutable_ptb_argument(tx)?;
                new_agent_funded_task(
                    tx,
                    metadata,
                    constraints,
                    execution,
                    registry,
                    agent,
                    agent_id,
                    priority_fee_percentage,
                    entry_group,
                    input_data,
                    skill_id,
                    *selected_dag,
                    *prepay_amount_mist,
                    *occurrence_budget_mist,
                    authorization_templates.clone(),
                )?
            }
        };

        tx.call_target(
            transfer_binding::public_share_object_target::<scheduler_binding::Task>,
            vec![task],
        )?;
        Ok(())
    })
}

/// PTB template to create a sender-owned invoker-funded scheduled task for an explicit agent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn new_invoker_funded_agent_task(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    metadata: sui::types::Argument,
    constraints: sui::types::Argument,
    execution: sui::types::Argument,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    agent_id: AgentId,
    priority_fee_percentage: Option<u64>,
    entry_group: &str,
    input_data: &std::collections::HashMap<String, std::collections::HashMap<String, NexusData>>,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    prepayment_coin: sui::types::Argument,
    refund_recipient: sui::types::Address,
    occurrence_budget_mist: u64,
    authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
) -> anyhow::Result<sui::types::Argument> {
    let agent_config = scheduled_agent_execution_config_arg(
        tx,
        agent_id,
        priority_fee_percentage,
        entry_group,
        input_data,
        skill_id,
        selected_dag,
        &authorization_templates,
    )?;
    let refund_recipient = tx.arg(&refund_recipient)?;
    let occurrence_budget_mist = tx.arg(&occurrence_budget_mist)?;
    tx.call_target(
        scheduler_binding::new_invoker_funded_agent_task_target,
        vec![
            metadata,
            constraints,
            execution,
            registry,
            agent,
            agent_config,
            prepayment_coin,
            refund_recipient,
            occurrence_budget_mist,
        ],
    )
}

/// PTB template to create an agent-owned scheduled task with agent-vault reserve components.
#[allow(clippy::too_many_arguments)]
pub(crate) fn new_agent_funded_task(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    metadata: sui::types::Argument,
    constraints: sui::types::Argument,
    execution: sui::types::Argument,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    agent_id: AgentId,
    priority_fee_percentage: Option<u64>,
    entry_group: &str,
    input_data: &std::collections::HashMap<String, std::collections::HashMap<String, NexusData>>,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    prepay_amount_mist: u64,
    occurrence_budget_mist: u64,
    authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
) -> anyhow::Result<sui::types::Argument> {
    let agent_config = scheduled_agent_execution_config_arg(
        tx,
        agent_id,
        priority_fee_percentage,
        entry_group,
        input_data,
        skill_id,
        selected_dag,
        &authorization_templates,
    )?;
    let prepay_amount_mist = tx.arg(&prepay_amount_mist)?;
    let occurrence_budget_mist = tx.arg(&occurrence_budget_mist)?;
    tx.call_target(
        scheduler_binding::new_agent_funded_task_target,
        vec![
            metadata,
            constraints,
            execution,
            registry,
            agent,
            agent_config,
            prepay_amount_mist,
            occurrence_budget_mist,
        ],
    )
}

fn schedule_policy_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    schedule_policy: &SkillSchedulePolicy,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let recurrence = match &schedule_policy.recurrence {
        SkillRecurrenceKind::Once => {
            tx.call_target(agent_binding::recurrence_once_target, vec![])?
        }
        SkillRecurrenceKind::Recursive {
            min_interval_ms,
            max_occurrences,
        } => {
            let min_interval_ms = tx.arg(min_interval_ms)?;
            let max_occurrences = option_u64_arg(tx, objects, max_occurrences.as_option())?;
            tx.call_target(
                agent_binding::recurrence_recursive_target,
                vec![min_interval_ms, max_occurrences],
            )?
        }
    };
    let allow_recursive = tx.arg(&schedule_policy.allow_recursive)?;

    tx.call_target(
        agent_binding::schedule_policy_target,
        vec![recurrence, allow_recursive],
    )
}

fn option_u64_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    _objects: &NexusObjects,
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

fn option_id_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    _objects: &NexusObjects,
    value: Option<sui::types::Address>,
) -> anyhow::Result<sui::types::Argument> {
    match value {
        Some(value) => {
            let value = tx.object_id(value)?;
            Ok(tx.option::<MoveObjectId>(Some(value))?)
        }
        None => Ok(tx.option::<MoveObjectId>(None)?),
    }
}

pub(crate) fn default_agent_execution_config_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag_id: sui::types::Argument,
    network: sui::types::Argument,
    entry_group: sui::types::Argument,
    inputs: sui::types::Argument,
    priority_fee_percentage: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(
        agent_binding::new_default_agent_execution_config_target,
        vec![
            dag_id,
            network,
            entry_group,
            inputs,
            priority_fee_percentage,
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn agent_execution_config_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    agent_id: sui::types::Argument,
    network: sui::types::Argument,
    entry_group: sui::types::Argument,
    inputs: sui::types::Argument,
    priority_fee_percentage: sui::types::Argument,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    authorization_templates: &[AgentVertexAuthorizationTemplate],
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let skill_id = tx.arg(&skill_id)?;
    let selected_dag = option_id_arg(tx, objects, selected_dag)?;
    let authorization_templates =
        scheduled_vertex_authorization_templates_arg(tx, authorization_templates)?;
    tx.call_target(
        agent_binding::new_agent_execution_config_target,
        vec![
            agent_id,
            network,
            entry_group,
            inputs,
            priority_fee_percentage,
            skill_id,
            selected_dag,
            authorization_templates,
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn scheduled_agent_execution_config_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    agent_id: AgentId,
    priority_fee_percentage: Option<u64>,
    entry_group: &str,
    input_data: &std::collections::HashMap<String, std::collections::HashMap<String, NexusData>>,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    authorization_templates: &[AgentVertexAuthorizationTemplate],
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let agent_id = tx.object_id(agent_id)?;
    let network = tx.object_id(objects.network_id)?;
    let entry_group = tx.graph_entry_group(entry_group)?;
    let inputs = crate::transactions::scheduler::build_inputs_vec_map(tx, input_data)?;
    let priority_fee_percentage =
        tx.arg(&effective_priority_fee_percentage(priority_fee_percentage)?)?;
    agent_execution_config_arg(
        tx,
        agent_id,
        network,
        entry_group,
        inputs,
        priority_fee_percentage,
        skill_id,
        selected_dag,
        authorization_templates,
    )
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

fn scheduled_vertex_authorization_template_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    template: &AgentVertexAuthorizationTemplate,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.arg(&template.skill_id)?;
    let vertex = tx.ascii_string(&template.vertex)?;
    let recipient_id = tx.object_id(template.recipient_id.clone().into())?;
    tx.call_target(
        authorization_binding::agent_vertex_authorization_template_target,
        vec![skill_id, vertex, recipient_id],
    )
}

fn scheduled_vertex_authorization_templates_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    authorization_templates: &[AgentVertexAuthorizationTemplate],
) -> anyhow::Result<sui::types::Argument> {
    let authorization_templates = authorization_templates
        .iter()
        .map(|template| scheduled_vertex_authorization_template_arg(tx, template))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(tx.move_vector::<AgentVertexAuthorizationTemplate>(authorization_templates)?)
}
