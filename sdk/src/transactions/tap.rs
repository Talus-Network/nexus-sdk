use crate::{
    idents::{
        interface,
        move_std,
        registry::AgentRegistry,
        scheduler,
        sui_framework,
        tap::TapStandard,
    },
    sui,
    types::{
        AgentId,
        AgentVertexAuthorizationTemplate,
        FixedTool,
        NexusObjects,
        RecurrenceKind,
        ScheduledOccurrenceFinalState,
        SkillId,
        SkillPaymentPolicy,
        SkillSchedulePolicy,
    },
};

fn agent_registry_call(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::tx::Argument>,
) -> sui::tx::Argument {
    tap_call_with_package(tx, objects.registry_pkg_id, ident, args)
}

fn tap_interface_call(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::tx::Argument>,
) -> sui::tx::Argument {
    tap_call_with_package(tx, objects.interface_pkg_id, ident, args)
}

fn scheduler_call(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::tx::Argument>,
) -> sui::tx::Argument {
    tap_call_with_package(tx, objects.scheduler_pkg_id, ident, args)
}

fn tap_call_with_package(
    tx: &mut sui::tx::TransactionBuilder,
    package: sui::types::Address,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::tx::Argument>,
) -> sui::tx::Argument {
    tx.move_call(
        sui::tx::Function::new(package, ident.module, ident.name),
        args,
    )
}

pub fn agent_registry_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    mutability: bool,
) -> anyhow::Result<sui::tx::Argument> {
    let registry = &objects.agent_registry;

    Ok(tx.object(sui::tx::ObjectInput::shared(
        *registry.object_id(),
        registry.version(),
        mutability,
    )))
}

fn tool_registry_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    mutability: bool,
) -> anyhow::Result<sui::tx::Argument> {
    let registry = &objects.tool_registry;

    Ok(tx.object(sui::tx::ObjectInput::shared(
        *registry.object_id(),
        registry.version(),
        mutability,
    )))
}

pub fn create_agent(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::CREATE_AGENT,
        vec![registry],
    ))
}

pub fn agent_id_from_address(
    tx: &mut sui::tx::TransactionBuilder,
    _objects: &NexusObjects,
    agent_id: AgentId,
) -> anyhow::Result<sui::tx::Argument> {
    sui_framework::Object::id_from_object_id(tx, agent_id)
}

#[allow(clippy::too_many_arguments)]
pub fn bootstrap_default_runtime_dag_skill_for_deployment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    let result = agent_registry_call(
        tx,
        objects,
        AgentRegistry::BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT,
        vec![registry],
    );
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn create_skill(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    dag_id: sui::types::Address,
    description: Vec<u8>,
    input_commitment: Vec<u8>,
    payment_policy: SkillPaymentPolicy,
    schedule_policy: SkillSchedulePolicy,
    active: bool,
) -> anyhow::Result<sui::tx::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.pure(&dag_id),
        tx.pure(&description),
        tx.pure(&input_commitment),
        payment_policy,
        schedule_policy,
        tx.pure(&active),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::CREATE_SKILL,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn register_skill(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    dag_id: sui::types::Address,
    description: Vec<u8>,
    input_commitment: Vec<u8>,
    payment_policy: SkillPaymentPolicy,
    schedule_policy: SkillSchedulePolicy,
) -> anyhow::Result<sui::tx::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.pure(&dag_id),
        tx.pure(&description),
        tx.pure(&input_commitment),
        payment_policy,
        schedule_policy,
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::REGISTER_SKILL,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn register_skill_with_fixed_tools(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    dag_id: sui::types::Address,
    description: Vec<u8>,
    input_commitment: Vec<u8>,
    payment_policy: SkillPaymentPolicy,
    schedule_policy: SkillSchedulePolicy,
    fixed_tools: Vec<FixedTool>,
) -> anyhow::Result<sui::tx::Argument> {
    let tool_registry = tool_registry_arg(tx, objects, false)?;
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let fixed_tools = fixed_tools_arg(tx, objects, &fixed_tools)?;
    let args = vec![
        registry,
        agent,
        tool_registry,
        tx.pure(&dag_id),
        tx.pure(&description),
        tx.pure(&input_commitment),
        payment_policy,
        schedule_policy,
        fixed_tools,
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::REGISTER_SKILL_WITH_FIXED_TOOLS,
        args,
    ))
}

pub fn get_skill_requirements(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    skill_id: SkillId,
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&skill_id);

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::GET_SKILL_REQUIREMENTS,
        vec![registry, agent, skill_id],
    ))
}

pub fn set_skill_active(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    skill_id: SkillId,
    active: bool,
) -> anyhow::Result<sui::tx::Argument> {
    let args = vec![registry, agent, tx.pure(&skill_id), tx.pure(&active)];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SET_SKILL_ACTIVE,
        args,
    ))
}

pub fn set_agent_active(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    active: bool,
) -> anyhow::Result<sui::tx::Argument> {
    let args = vec![registry, agent, tx.pure(&active)];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SET_AGENT_ACTIVE,
        args,
    ))
}

pub fn update_skill_description(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    skill_id: SkillId,
    description: Vec<u8>,
) -> anyhow::Result<sui::tx::Argument> {
    let args = vec![registry, agent, tx.pure(&skill_id), tx.pure(&description)];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::UPDATE_SKILL_DESCRIPTION,
        args,
    ))
}

pub fn update_dag(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    skill_id: SkillId,
    dag_id: sui::types::Address,
) -> anyhow::Result<sui::tx::Argument> {
    let args = vec![registry, agent, tx.pure(&skill_id), tx.pure(&dag_id)];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::UPDATE_DAG,
        args,
    ))
}

pub fn update_skill_policies(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    skill_id: SkillId,
    payment_policy: SkillPaymentPolicy,
    schedule_policy: SkillSchedulePolicy,
) -> anyhow::Result<sui::tx::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.pure(&skill_id),
        payment_policy,
        schedule_policy,
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::UPDATE_SKILL_POLICIES,
        args,
    ))
}

#[derive(Clone, Debug)]
pub struct AgentSkillPaymentInput {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub source: Vec<u8>,
    pub max_budget: u64,
}

impl AgentSkillPaymentInput {
    pub fn invoker_source(
        agent_id: AgentId,
        skill_id: SkillId,
        invoker: sui::types::Address,
        max_budget: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            agent_id,
            skill_id,
            source: crate::types::payment_source_from_address(invoker)?,
            max_budget,
        })
    }

    /// Build direct payment source bytes for an agent-funded policy.
    ///
    /// This is the source encoding accepted by the standard TAP payment policy.
    pub fn agent_vault_source(
        agent_id: AgentId,
        skill_id: SkillId,
        max_budget: u64,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            agent_id,
            skill_id,
            source: crate::types::payment_source_from_address(agent_id)?,
            max_budget,
        })
    }
}

pub fn deposit_agent_payment_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::tx::Argument,
    coin: sui::tx::Argument,
) -> sui::tx::Argument {
    // The Move interface accepts any depositor; authorization is enforced only
    // on withdrawal.
    tap_interface_call(
        tx,
        objects,
        TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT,
        vec![agent, coin],
    )
}

pub fn withdraw_agent_payment_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    amount: u64,
) -> anyhow::Result<sui::tx::Argument> {
    // The TAP Move module checks mutable agent custody through the registry.
    let amount = tx.pure(&amount);

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::WITHDRAW_AGENT_PAYMENT_VAULT,
        vec![registry, agent, amount],
    ))
}

/// PTB template to create a sender-owned invoker-funded scheduled task for an explicit agent.
#[allow(clippy::too_many_arguments)]
pub fn new_invoker_funded_agent_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    metadata: sui::tx::Argument,
    constraints: sui::tx::Argument,
    execution: sui::tx::Argument,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    agent_id: AgentId,
    dag_id: sui::types::Address,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &std::collections::HashMap<
        String,
        std::collections::HashMap<String, crate::types::DataStorage>,
    >,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    prepayment_coin: sui::tx::Argument,
    refund_recipient: sui::types::Address,
    occurrence_budget: u64,
    authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
) -> anyhow::Result<sui::tx::Argument> {
    let agent_config = scheduled_agent_execution_config_arg(
        tx,
        objects,
        agent_id,
        dag_id,
        priority_fee_per_gas_unit,
        entry_group,
        input_data,
        skill_id,
        selected_dag,
        &authorization_templates,
    )?;
    let refund_recipient = tx.pure(&refund_recipient);
    let occurrence_budget = tx.pure(&occurrence_budget);
    Ok(scheduler_call(
        tx,
        objects,
        scheduler::Scheduler::NEW_INVOKER_FUNDED_AGENT_TASK,
        vec![
            metadata,
            constraints,
            execution,
            registry,
            agent,
            agent_config,
            prepayment_coin,
            refund_recipient,
            occurrence_budget,
        ],
    ))
}

/// PTB template to create an agent-owned scheduled task with agent-vault reserve components.
#[allow(clippy::too_many_arguments)]
pub fn new_agent_funded_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    metadata: sui::tx::Argument,
    constraints: sui::tx::Argument,
    execution: sui::tx::Argument,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    agent_id: AgentId,
    dag_id: sui::types::Address,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &std::collections::HashMap<
        String,
        std::collections::HashMap<String, crate::types::DataStorage>,
    >,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    prepay_amount: u64,
    occurrence_budget: u64,
    authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
) -> anyhow::Result<sui::tx::Argument> {
    let agent_config = scheduled_agent_execution_config_arg(
        tx,
        objects,
        agent_id,
        dag_id,
        priority_fee_per_gas_unit,
        entry_group,
        input_data,
        skill_id,
        selected_dag,
        &authorization_templates,
    )?;
    let prepay_amount = tx.pure(&prepay_amount);
    let occurrence_budget = tx.pure(&occurrence_budget);
    Ok(scheduler_call(
        tx,
        objects,
        scheduler::Scheduler::NEW_AGENT_FUNDED_TASK,
        vec![
            metadata,
            constraints,
            execution,
            registry,
            agent,
            agent_config,
            prepay_amount,
            occurrence_budget,
        ],
    ))
}

fn schedule_policy_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    schedule_policy: &SkillSchedulePolicy,
) -> anyhow::Result<sui::tx::Argument> {
    let recurrence = match &schedule_policy.recurrence {
        RecurrenceKind::Once => {
            tap_interface_call(tx, objects, TapStandard::RECURRENCE_ONCE, vec![])
        }
        RecurrenceKind::Recursive {
            min_interval_ms,
            max_occurrences,
        } => {
            let min_interval_ms = tx.pure(min_interval_ms);
            let max_occurrences = option_u64_arg(tx, max_occurrences.as_ref())?;
            tap_interface_call(
                tx,
                objects,
                TapStandard::RECURRENCE_RECURSIVE,
                vec![min_interval_ms, max_occurrences],
            )
        }
    };
    let allow_recursive = tx.pure(&schedule_policy.allow_recursive);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::SCHEDULE_POLICY,
        vec![recurrence, allow_recursive],
    ))
}

fn option_u64_arg(
    tx: &mut sui::tx::TransactionBuilder,
    value: Option<&u64>,
) -> anyhow::Result<sui::tx::Argument> {
    match value {
        Some(value) => {
            let value = tx.pure(value);
            Ok(move_std::Option::some(tx, sui::types::TypeTag::U64, value))
        }
        None => Ok(move_std::Option::none(tx, sui::types::TypeTag::U64)),
    }
}

fn payment_policy_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    payment_policy: &SkillPaymentPolicy,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(match payment_policy {
        SkillPaymentPolicy::UserFunded => {
            tap_interface_call(tx, objects, TapStandard::PAYMENT_POLICY_USER_FUNDED, vec![])
        }
        SkillPaymentPolicy::AgentFunded { max_budget } => {
            let max_budget = tx.pure(max_budget);
            tap_interface_call(
                tx,
                objects,
                TapStandard::PAYMENT_POLICY_AGENT_FUNDED,
                vec![max_budget],
            )
        }
    })
}

fn option_id_arg(
    tx: &mut sui::tx::TransactionBuilder,
    value: Option<sui::types::Address>,
) -> anyhow::Result<sui::tx::Argument> {
    let id_type = sui_framework::into_type_tag(sui_framework::Object::ID_TYPE);
    match value {
        Some(value) => {
            let value = sui_framework::Object::id_from_object_id(tx, value)?;
            Ok(move_std::Option::some(tx, id_type, value))
        }
        None => Ok(move_std::Option::none(tx, id_type)),
    }
}

pub(crate) fn default_agent_execution_config_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag_id: sui::tx::Argument,
    network: sui::tx::Argument,
    entry_group: sui::tx::Argument,
    inputs: sui::tx::Argument,
    priority_fee_per_gas_unit: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::NEW_DEFAULT_AGENT_EXECUTION_CONFIG,
        vec![
            dag_id,
            network,
            entry_group,
            inputs,
            priority_fee_per_gas_unit,
        ],
    ))
}

pub(crate) fn agent_execution_config_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent_id: sui::tx::Argument,
    network: sui::tx::Argument,
    entry_group: sui::tx::Argument,
    inputs: sui::tx::Argument,
    priority_fee_per_gas_unit: sui::tx::Argument,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    authorization_templates: &[AgentVertexAuthorizationTemplate],
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&skill_id);
    let selected_dag = option_id_arg(tx, selected_dag)?;
    let authorization_templates =
        scheduled_vertex_authorization_templates_arg(tx, objects, authorization_templates)?;
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::NEW_AGENT_EXECUTION_CONFIG,
        vec![
            agent_id,
            network,
            entry_group,
            inputs,
            priority_fee_per_gas_unit,
            skill_id,
            selected_dag,
            authorization_templates,
        ],
    ))
}

#[allow(clippy::too_many_arguments)]
fn scheduled_agent_execution_config_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent_id: AgentId,
    dag_id: sui::types::Address,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &std::collections::HashMap<
        String,
        std::collections::HashMap<String, crate::types::DataStorage>,
    >,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
    authorization_templates: &[AgentVertexAuthorizationTemplate],
) -> anyhow::Result<sui::tx::Argument> {
    let agent_id = sui_framework::Object::id_from_object_id(tx, agent_id)?;
    let network = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;
    let entry_group =
        interface::Graph::entry_group_from_str(tx, objects.interface_pkg_id, entry_group)?;
    let inputs = crate::transactions::scheduler::build_inputs_vec_map(tx, objects, input_data)?;
    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);
    let selected_dag = selected_dag.or(Some(dag_id));
    agent_execution_config_arg(
        tx,
        objects,
        agent_id,
        network,
        entry_group,
        inputs,
        priority_fee_per_gas_unit,
        skill_id,
        selected_dag,
        authorization_templates,
    )
}

fn fixed_tool_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    fixed_tool: &FixedTool,
) -> anyhow::Result<sui::tx::Argument> {
    let tool_registry_id =
        sui_framework::Object::id_from_object_id(tx, fixed_tool.tool_registry_id)?;
    let tool_fqn = move_std::Ascii::ascii_string_from_str(tx, &fixed_tool.tool_fqn)?;

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::FIXED_TOOL,
        vec![tool_registry_id, tool_fqn],
    ))
}

fn fixed_tools_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    fixed_tools: &[FixedTool],
) -> anyhow::Result<sui::tx::Argument> {
    let fixed_tool_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.interface_pkg_id,
        crate::idents::tap::STANDARD_AGENT_MODULE,
        sui::types::Identifier::from_static("FixedTool"),
        vec![],
    )));
    let vector = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
        )
        .with_type_args(vec![fixed_tool_type.clone()]),
        vec![],
    );

    for fixed_tool in fixed_tools {
        let item = fixed_tool_arg(tx, objects, fixed_tool)?;
        tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Vector::PUSH_BACK.module,
                move_std::Vector::PUSH_BACK.name,
            )
            .with_type_args(vec![fixed_tool_type.clone()]),
            vec![vector, item],
        );
    }

    Ok(vector)
}

fn scheduled_vertex_authorization_template_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    template: &AgentVertexAuthorizationTemplate,
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&template.skill_id);
    let vertex = move_std::Ascii::ascii_string_from_str(tx, &template.vertex)?;
    let recipient_id = sui_framework::Object::id_from_object_id(tx, template.recipient_id)?;
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::AGENT_VERTEX_AUTHORIZATION_TEMPLATE,
        vec![skill_id, vertex, recipient_id],
    ))
}

fn scheduled_vertex_authorization_templates_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    authorization_templates: &[AgentVertexAuthorizationTemplate],
) -> anyhow::Result<sui::tx::Argument> {
    let template_type =
        crate::idents::tap::agent_vertex_authorization_template_type(objects.interface_pkg_id);
    let vector = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
        )
        .with_type_args(vec![template_type.clone()]),
        vec![],
    );

    for template in authorization_templates {
        let item = scheduled_vertex_authorization_template_arg(tx, objects, template)?;
        tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Vector::PUSH_BACK.module,
                move_std::Vector::PUSH_BACK.name,
            )
            .with_type_args(vec![template_type.clone()]),
            vec![vector, item],
        );
    }

    Ok(vector)
}

#[allow(clippy::too_many_arguments)]
pub fn complete_scheduled_payment_reserve_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    reserve: sui::tx::Argument,
    execution_id: sui::types::Address,
    payment_id: sui::types::Address,
    final_state: ScheduledOccurrenceFinalState,
) -> anyhow::Result<sui::tx::Argument> {
    let final_state = scheduled_occurrence_final_state_arg(tx, objects, final_state);
    let execution_id = tx.pure(&execution_id);
    let payment_id = tx.pure(&payment_id);
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::COMPLETE_SCHEDULED_PAYMENT_RESERVE_OCCURRENCE,
        vec![reserve, execution_id, payment_id, final_state],
    ))
}

fn scheduled_occurrence_final_state_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    final_state: ScheduledOccurrenceFinalState,
) -> sui::tx::Argument {
    let ident = match final_state {
        ScheduledOccurrenceFinalState::InFlight => {
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_IN_FLIGHT
        }
        ScheduledOccurrenceFinalState::Accomplished => {
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_ACCOMPLISHED
        }
        ScheduledOccurrenceFinalState::Refunded => {
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_REFUNDED
        }
    };
    tap_interface_call(tx, objects, ident, vec![])
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks};

    struct TxInspector {
        tx: sui::types::Transaction,
    }

    impl TxInspector {
        fn new(tx: sui::types::Transaction) -> Self {
            Self { tx }
        }

        fn commands(&self) -> &Vec<sui::types::Command> {
            let sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction { commands, .. },
            ) = &self.tx.kind
            else {
                panic!("expected programmable transaction");
            };

            commands
        }

        fn inputs(&self) -> &Vec<sui::types::Input> {
            let sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction { inputs, .. },
            ) = &self.tx.kind
            else {
                panic!("expected programmable transaction");
            };

            inputs
        }

        fn input(&self, argument: &sui::types::Argument) -> &sui::types::Input {
            let sui::types::Argument::Input(index) = argument else {
                panic!("expected input argument, got {argument:?}");
            };

            self.inputs()
                .get(*index as usize)
                .unwrap_or_else(|| panic!("missing input at index {index}"))
        }

        fn expect_shared_object(
            &self,
            argument: &sui::types::Argument,
            expected: &sui::types::ObjectReference,
            expected_mutable: bool,
        ) {
            let sui::types::Input::Shared(shared) = self.input(argument) else {
                panic!("expected shared object input");
            };
            assert_eq!(shared.object_id(), *expected.object_id());
            assert_eq!(shared.version(), expected.version());
            assert_eq!(shared.mutability().is_mutable(), expected_mutable);
        }

        fn move_call(&self, index: usize) -> &sui::types::MoveCall {
            match self.commands().get(index) {
                Some(sui::types::Command::MoveCall(call)) => call,
                Some(other) => panic!("expected move call, got {other:?}"),
                None => panic!("missing command at index {index}"),
            }
        }
    }

    #[test]
    fn registry_arg_helpers_select_expected_mutability() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut immutable_tx = sui::tx::TransactionBuilder::new();
        agent_registry_arg(&mut immutable_tx, &objects, false).expect("immutable registry");
        let immutable_inspector =
            TxInspector::new(sui_mocks::mock_finish_transaction(immutable_tx));
        immutable_inspector.expect_shared_object(
            &sui::types::Argument::Input(0),
            &objects.agent_registry,
            false,
        );

        let mut mutable_tx = sui::tx::TransactionBuilder::new();
        agent_registry_arg(&mut mutable_tx, &objects, true).expect("mutable registry");
        let mutable_inspector = TxInspector::new(sui_mocks::mock_finish_transaction(mutable_tx));
        mutable_inspector.expect_shared_object(
            &sui::types::Argument::Input(0),
            &objects.agent_registry,
            true,
        );
    }

    #[test]
    fn register_skill_with_fixed_tools_passes_tool_registry_reference() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = agent_registry_arg(&mut tx, &objects, true).expect("registry arg");
        let agent = tx.pure(&2_u64);

        register_skill_with_fixed_tools(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0xd"),
            b"demo skill".to_vec(),
            b"input commitment".to_vec(),
            SkillPaymentPolicy::UserFunded,
            SkillSchedulePolicy::default(),
            vec![FixedTool {
                tool_registry_id: *objects.tool_registry.object_id(),
                tool_fqn: "demo.tool@1".to_string(),
            }],
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .find(|call| {
                call.package == objects.registry_pkg_id
                    && call.function == AgentRegistry::REGISTER_SKILL_WITH_FIXED_TOOLS.name
            })
            .expect("register_skill_with_fixed_tools call");

        assert_eq!(
            call.module,
            AgentRegistry::REGISTER_SKILL_WITH_FIXED_TOOLS.module
        );
        assert_eq!(call.arguments.len(), 9);
        inspector.expect_shared_object(&call.arguments[2], &objects.tool_registry, false);
    }

    #[test]
    fn new_agent_funded_task_builds_scheduler_constructor_call_from_tap_surface() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let metadata = tx.pure(&1_u64);
        let constraints = tx.pure(&2_u64);
        let execution = tx.pure(&3_u64);
        let registry = tx.pure(&4_u64);
        let agent = tx.pure(&5_u64);

        new_agent_funded_task(
            &mut tx,
            &objects,
            metadata,
            constraints,
            execution,
            registry,
            agent,
            sui::types::Address::from_static("0xa"),
            sui::types::Address::from_static("0xd"),
            11,
            "default",
            &std::collections::HashMap::new(),
            7,
            Some(sui::types::Address::from_static("0xd")),
            50,
            25,
            vec![AgentVertexAuthorizationTemplate {
                skill_id: 7,
                vertex: "demo_delayed_fire_vertex".to_string(),
                recipient_id: sui::types::Address::from_static("0x82"),
            }],
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.function == scheduler::Scheduler::NEW_AGENT_FUNDED_TASK.name
            })
            .expect("combined agent-vault constructor call");
        assert_eq!(
            call.module,
            scheduler::Scheduler::NEW_AGENT_FUNDED_TASK.module
        );
        assert_eq!(call.arguments.len(), 8);
    }

    #[test]
    fn new_invoker_funded_agent_task_builds_scheduler_constructor_call_from_tap_surface() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let metadata = tx.pure(&1_u64);
        let constraints = tx.pure(&2_u64);
        let execution = tx.pure(&3_u64);
        let registry = tx.pure(&4_u64);
        let agent = tx.pure(&5_u64);
        let coin = tx.pure(&6_u64);

        new_invoker_funded_agent_task(
            &mut tx,
            &objects,
            metadata,
            constraints,
            execution,
            registry,
            agent,
            sui::types::Address::from_static("0xa"),
            sui::types::Address::from_static("0xd"),
            11,
            "default",
            &std::collections::HashMap::new(),
            7,
            None,
            coin,
            sui::types::Address::from_static("0x81"),
            25,
            vec![],
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.function == scheduler::Scheduler::NEW_INVOKER_FUNDED_AGENT_TASK.name
            })
            .expect("combined address-funded constructor call");
        assert_eq!(
            call.module,
            scheduler::Scheduler::NEW_INVOKER_FUNDED_AGENT_TASK.module
        );
        assert_eq!(call.arguments.len(), 9);
    }

    #[test]
    fn register_skill_builder_carries_artifact_identity_and_config() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.pure(&1_u64);
        let agent = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));

        register_skill(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0xd"),
            vec![3],
            vec![2],
            SkillPaymentPolicy::UserFunded,
            SkillSchedulePolicy::default(),
        )
        .expect("register skill builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let register_call_idx = inspector
            .commands()
            .iter()
            .position(|command| {
                matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.package == objects.registry_pkg_id
                            && call.module == AgentRegistry::REGISTER_SKILL.module
                            && call.function == AgentRegistry::REGISTER_SKILL.name
                )
            })
            .expect("register_skill call");
        let call = inspector.move_call(register_call_idx);
        assert_eq!(call.package, objects.registry_pkg_id);
        assert_eq!(call.module, AgentRegistry::REGISTER_SKILL.module);
        assert_eq!(call.function, AgentRegistry::REGISTER_SKILL.name);
        assert_eq!(call.arguments.len(), 7);
    }

    #[test]
    fn payment_and_vault_builders_target_standard_tap_functions() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = agent_registry_arg(&mut tx, &objects, true).expect("registry");
        let agent = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));
        let vault_coin = tx.pure(&10_u64);

        let invoker_input = AgentSkillPaymentInput::invoker_source(
            sui::types::Address::from_static("0xa"),
            11,
            sui::types::Address::from_static("0x1"),
            100,
        )
        .expect("invoker source");
        assert_eq!(invoker_input.skill_id, 11);
        let vault_input = AgentSkillPaymentInput::agent_vault_source(
            sui::types::Address::from_static("0xa"),
            12,
            101,
        )
        .expect("agent vault source");
        assert_eq!(vault_input.max_budget, 101);

        deposit_agent_payment_vault(&mut tx, &objects, agent, vault_coin);
        withdraw_agent_payment_vault(&mut tx, &objects, registry, agent, 33).expect("withdraw");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let function_names = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) if call.package == objects.interface_pkg_id => {
                    Some(call.function.clone())
                }
                sui::types::Command::MoveCall(call) if call.package == objects.registry_pkg_id => {
                    Some(call.function.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for expected in [
            TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name,
            AgentRegistry::WITHDRAW_AGENT_PAYMENT_VAULT.name,
        ] {
            assert!(
                function_names.contains(&expected),
                "missing TAP call {expected}"
            );
        }
    }

    #[test]
    fn endpoint_and_schedule_builders_cover_variants() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry_arg =
            |tx: &mut sui::tx::TransactionBuilder| agent_registry_arg(tx, &objects, true).unwrap();
        let agent_arg = |tx: &mut sui::tx::TransactionBuilder| {
            tx.object(sui::tx::ObjectInput::shared(
                sui::types::Address::from_static("0xa"),
                1,
                true,
            ))
        };
        let scheduled_task = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0x50"),
            3,
            true,
        ));

        agent_id_from_address(&mut tx, &objects, sui::types::Address::from_static("0xa"))
            .expect("agent id");
        let registry = registry_arg(&mut tx);
        let agent = agent_arg(&mut tx);
        get_skill_requirements(&mut tx, &objects, registry, agent, 11).expect("requirements");
        let registry = registry_arg(&mut tx);
        let agent = agent_arg(&mut tx);
        update_dag(
            &mut tx,
            &objects,
            registry,
            agent,
            11,
            sui::types::Address::from_static("0xd"),
        )
        .expect("update dag");
        let registry = registry_arg(&mut tx);
        let agent = agent_arg(&mut tx);
        update_skill_policies(
            &mut tx,
            &objects,
            registry,
            agent,
            11,
            SkillPaymentPolicy::default(),
            SkillSchedulePolicy::default(),
        )
        .expect("update policies");
        for state in [
            ScheduledOccurrenceFinalState::InFlight,
            ScheduledOccurrenceFinalState::Accomplished,
            ScheduledOccurrenceFinalState::Refunded,
        ] {
            complete_scheduled_payment_reserve_occurrence(
                &mut tx,
                &objects,
                scheduled_task,
                sui::types::Address::from_static("0x90"),
                sui::types::Address::from_static("0x91"),
                state,
            )
            .expect("complete occurrence");
        }

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let function_names = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call.function.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        for expected in [
            sui_framework::Object::ID_FROM_ADDRESS.name,
            AgentRegistry::GET_SKILL_REQUIREMENTS.name,
            AgentRegistry::UPDATE_DAG.name,
            AgentRegistry::UPDATE_SKILL_POLICIES.name,
            TapStandard::COMPLETE_SCHEDULED_PAYMENT_RESERVE_OCCURRENCE.name,
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_IN_FLIGHT.name,
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_ACCOMPLISHED.name,
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_REFUNDED.name,
        ] {
            assert!(
                function_names.contains(&expected),
                "missing TAP call {expected}"
            );
        }
    }

    #[test]
    fn register_skill_builder_supports_agent_funded_payment_mode() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.pure(&1_u64);
        let agent = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));

        register_skill(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0xd"),
            vec![3],
            vec![2],
            SkillPaymentPolicy::AgentFunded { max_budget: 100 },
            SkillSchedulePolicy::default(),
        )
        .expect("agent funded payment mode is supported");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let function_names = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) if call.package == objects.interface_pkg_id => {
                    Some(call.function.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(
            function_names.contains(&TapStandard::PAYMENT_POLICY_AGENT_FUNDED.name),
            "missing agent-funded payment policy constructor"
        );
    }
}
