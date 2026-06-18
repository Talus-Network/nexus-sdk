use crate::{
    idents::{move_std, registry::AgentRegistry, sui_framework, tap::TapStandard},
    sui,
    types::{
        AgentId,
        InterfaceRevision,
        NexusObjects,
        SkillId,
        TapFixedTool,
        TapPaymentPolicy,
        TapRecurrenceKind,
        TapSchedulePolicy,
        TapScheduledAuthorizationGrantTemplate,
        TapScheduledOccurrenceFinalState,
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

pub fn interface_revision(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    interface_revision: InterfaceRevision,
) -> anyhow::Result<sui::tx::Argument> {
    let interface_revision = tx.pure(&interface_revision.0);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::INTERFACE_REVISION,
        vec![interface_revision],
    ))
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
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
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
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
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
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    fixed_tools: Vec<TapFixedTool>,
) -> anyhow::Result<sui::tx::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let fixed_tools = fixed_tools_arg(tx, objects, &fixed_tools)?;
    let args = vec![
        registry,
        agent,
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
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
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

pub fn worksheet(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    skill_id: SkillId,
    execution_id: sui::types::Address,
) -> anyhow::Result<sui::tx::Argument> {
    let args = vec![registry, agent, tx.pure(&skill_id), tx.pure(&execution_id)];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::WORKSHEET,
        args,
    ))
}

pub fn workflow_worksheet_for_ids(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<sui::tx::Argument> {
    let agent_id = agent_id_from_address(tx, objects, agent_id)?;
    let args = vec![registry, agent_id, tx.pure(&skill_id)];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::WORKFLOW_WORKSHEET_FOR_IDS,
        args,
    ))
}

pub fn default_dag_executor_workflow_worksheet(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::DEFAULT_DAG_EXECUTOR_WORKFLOW_WORKSHEET,
        vec![registry],
    ))
}

pub fn confirm_tool_eval_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    worksheet: sui::tx::Argument,
) -> sui::tx::Argument {
    agent_registry_call(
        tx,
        objects,
        AgentRegistry::CONFIRM_TOOL_EVAL_FOR_WALK,
        vec![registry, worksheet],
    )
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
            source: crate::types::tap_payment_source_for_address(invoker)?,
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
            source: crate::types::tap_payment_source_for_address(agent_id)?,
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

#[allow(clippy::too_many_arguments)]
pub fn schedule_skill_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    skill_id: SkillId,
    long_term_gas_coin_id: sui::types::Address,
    refill_policy_commitment: Vec<u8>,
    schedule_policy: TapSchedulePolicy,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&skill_id);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        skill_id,
        tx.pure(&long_term_gas_coin_id),
        tx.pure(&refill_policy_commitment),
        schedule_policy,
        tx.pure(&schedule_entries_commitment),
        tx.pure(&first_after_ms),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SCHEDULE_SKILL_EXECUTION,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn schedule_skill_execution_address_funded(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepayment_coin: sui::tx::Argument,
    refund_recipient: sui::types::Address,
    payment_source: Vec<u8>,
    occurrence_budget: u64,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&skill_id);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.pure(&scheduler_task_id),
        skill_id,
        prepayment_coin,
        tx.pure(&refund_recipient),
        tx.pure(&payment_source),
        tx.pure(&occurrence_budget),
        schedule_policy,
        tx.pure(&refill_policy_commitment),
        tx.pure(&schedule_entries_commitment),
        tx.pure(&first_after_ms),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn schedule_skill_execution_address_funded_with_grants(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepayment_coin: sui::tx::Argument,
    refund_recipient: sui::types::Address,
    payment_source: Vec<u8>,
    occurrence_budget: u64,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
    grant_templates: Vec<TapScheduledAuthorizationGrantTemplate>,
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&skill_id);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let grant_templates =
        scheduled_authorization_grant_templates_arg(tx, objects, &grant_templates)?;
    let args = vec![
        registry,
        agent,
        tx.pure(&scheduler_task_id),
        skill_id,
        prepayment_coin,
        tx.pure(&refund_recipient),
        tx.pure(&payment_source),
        tx.pure(&occurrence_budget),
        schedule_policy,
        tx.pure(&refill_policy_commitment),
        tx.pure(&schedule_entries_commitment),
        tx.pure(&first_after_ms),
        grant_templates,
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED_WITH_GRANTS,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn schedule_default_dag_executor_skill_execution_address_funded(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    scheduler_task_id: sui::types::Address,
    prepayment_coin: sui::tx::Argument,
    refund_recipient: sui::types::Address,
    payment_source: Vec<u8>,
    occurrence_budget: u64,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        tx.pure(&scheduler_task_id),
        prepayment_coin,
        tx.pure(&refund_recipient),
        tx.pure(&payment_source),
        tx.pure(&occurrence_budget),
        schedule_policy,
        tx.pure(&refill_policy_commitment),
        tx.pure(&schedule_entries_commitment),
        tx.pure(&first_after_ms),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SCHEDULE_DEFAULT_DAG_EXECUTOR_SKILL_EXECUTION_ADDRESS_FUNDED,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn schedule_skill_execution_from_agent_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepay_amount: u64,
    occurrence_budget: u64,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&skill_id);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.pure(&scheduler_task_id),
        skill_id,
        tx.pure(&prepay_amount),
        tx.pure(&occurrence_budget),
        schedule_policy,
        tx.pure(&refill_policy_commitment),
        tx.pure(&schedule_entries_commitment),
        tx.pure(&first_after_ms),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn schedule_skill_execution_from_agent_vault_with_grants(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    agent: sui::tx::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepay_amount: u64,
    occurrence_budget: u64,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
    grant_templates: Vec<TapScheduledAuthorizationGrantTemplate>,
) -> anyhow::Result<sui::tx::Argument> {
    let skill_id = tx.pure(&skill_id);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let grant_templates =
        scheduled_authorization_grant_templates_arg(tx, objects, &grant_templates)?;
    let args = vec![
        registry,
        agent,
        tx.pure(&scheduler_task_id),
        skill_id,
        tx.pure(&prepay_amount),
        tx.pure(&occurrence_budget),
        schedule_policy,
        tx.pure(&refill_policy_commitment),
        tx.pure(&schedule_entries_commitment),
        tx.pure(&first_after_ms),
        grant_templates,
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT_WITH_GRANTS,
        args,
    ))
}

fn schedule_policy_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    schedule_policy: &TapSchedulePolicy,
) -> anyhow::Result<sui::tx::Argument> {
    let recurrence = match &schedule_policy.recurrence {
        TapRecurrenceKind::Once => {
            tap_interface_call(tx, objects, TapStandard::RECURRENCE_ONCE, vec![])
        }
        TapRecurrenceKind::Recursive {
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
    payment_policy: &TapPaymentPolicy,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(match payment_policy {
        TapPaymentPolicy::UserFunded => {
            tap_interface_call(tx, objects, TapStandard::PAYMENT_POLICY_USER_FUNDED, vec![])
        }
        TapPaymentPolicy::AgentFunded { max_budget } => {
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

fn fixed_tool_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    fixed_tool: &TapFixedTool,
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
    fixed_tools: &[TapFixedTool],
) -> anyhow::Result<sui::tx::Argument> {
    let fixed_tool_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.interface_pkg_id,
        crate::idents::tap::STANDARD_TAP_MODULE,
        sui::types::Identifier::from_static("TapFixedTool"),
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

fn scheduled_authorization_grant_template_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    template: &TapScheduledAuthorizationGrantTemplate,
) -> anyhow::Result<sui::tx::Argument> {
    let dag_id = tx.pure(&template.dag_id);
    let vertex = move_std::Ascii::ascii_string_from_str(tx, &template.vertex)?;
    let tool_package = tx.pure(&template.tool_package);
    let tool_module = move_std::Ascii::ascii_string_from_str(tx, &template.tool_module)?;
    let tool_function = move_std::Ascii::ascii_string_from_str(tx, &template.tool_function)?;
    let operation_commitment = tx.pure(&template.operation_commitment);
    let constraints_commitment = tx.pure(&template.constraints_commitment);
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::SCHEDULED_AUTHORIZATION_GRANT_TEMPLATE,
        vec![
            dag_id,
            vertex,
            tool_package,
            tool_module,
            tool_function,
            operation_commitment,
            constraints_commitment,
        ],
    ))
}

fn scheduled_authorization_grant_templates_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    grant_templates: &[TapScheduledAuthorizationGrantTemplate],
) -> anyhow::Result<sui::tx::Argument> {
    let template_type =
        crate::idents::tap::scheduled_authorization_grant_template_type(objects.interface_pkg_id);
    let vector = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
        )
        .with_type_args(vec![template_type.clone()]),
        vec![],
    );

    for template in grant_templates {
        let item = scheduled_authorization_grant_template_arg(tx, objects, template)?;
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

pub fn trigger_scheduled_skill_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::tx::Argument,
    scheduled_task: sui::tx::Argument,
    execution_id: sui::types::Address,
) -> anyhow::Result<sui::tx::Argument> {
    let execution_id = tx.pure(&execution_id);
    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::TRIGGER_SCHEDULED_SKILL_EXECUTION,
        vec![registry, scheduled_task, execution_id],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn complete_scheduled_skill_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    scheduled_task: sui::tx::Argument,
    execution_id: sui::types::Address,
    payment_id: sui::types::Address,
    final_state: TapScheduledOccurrenceFinalState,
    continue_recurring: bool,
    next_after_ms: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let final_state = scheduled_occurrence_final_state_arg(tx, objects, final_state);
    let execution_id = tx.pure(&execution_id);
    let payment_id = tx.pure(&payment_id);
    let continue_recurring = tx.pure(&continue_recurring);
    let next_after_ms = tx.pure(&next_after_ms);
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::COMPLETE_SCHEDULED_SKILL_OCCURRENCE,
        vec![
            scheduled_task,
            execution_id,
            payment_id,
            final_state,
            continue_recurring,
            next_after_ms,
        ],
    ))
}

fn scheduled_occurrence_final_state_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    final_state: TapScheduledOccurrenceFinalState,
) -> sui::tx::Argument {
    let ident = match final_state {
        TapScheduledOccurrenceFinalState::InFlight => {
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_IN_FLIGHT
        }
        TapScheduledOccurrenceFinalState::Accomplished => {
            TapStandard::SCHEDULED_OCCURRENCE_FINAL_STATE_ACCOMPLISHED
        }
        TapScheduledOccurrenceFinalState::Refunded => {
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

        fn expect_u64(&self, argument: &sui::types::Argument, expected: u64) {
            let sui::types::Input::Pure(value) = self.input(argument) else {
                panic!("expected pure u64 input");
            };
            let actual: u64 = bcs::from_bytes(value).expect("u64 BCS decodes");
            assert_eq!(actual, expected);
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
    fn worksheet_builder_uses_standard_tap_ident() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.pure(&1_u64);

        workflow_worksheet_for_ids(
            &mut tx,
            &objects,
            registry,
            sui::types::Address::from_static("0xa"),
            11,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        let worksheet_call = inspector.move_call(1);
        assert_eq!(call.package, sui_framework::PACKAGE_ID);
        assert_eq!(call.module, sui_framework::Object::ID_FROM_ADDRESS.module);
        assert_eq!(call.function, sui_framework::Object::ID_FROM_ADDRESS.name);
        assert_eq!(worksheet_call.package, objects.registry_pkg_id);
        assert_eq!(
            worksheet_call.module,
            AgentRegistry::WORKFLOW_WORKSHEET_FOR_IDS.module
        );
        assert_eq!(
            worksheet_call.function,
            AgentRegistry::WORKFLOW_WORKSHEET_FOR_IDS.name
        );
        assert_eq!(worksheet_call.arguments.len(), 3);
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
    fn execute_and_schedule_use_peer_standard_tap_idents() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.pure(&3_u64);
        let schedule_agent = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0xa"),
            1,
            false,
        ));
        schedule_skill_execution(
            &mut tx,
            &objects,
            registry,
            schedule_agent,
            11,
            sui::types::Address::from_static("0xc"),
            vec![3],
            TapSchedulePolicy::default(),
            vec![4],
            55,
        )
        .expect("schedule builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();
        let schedule_call = calls
            .iter()
            .find(|call| {
                call.package == objects.registry_pkg_id
                    && call.module == AgentRegistry::SCHEDULE_SKILL_EXECUTION.module
                    && call.function == AgentRegistry::SCHEDULE_SKILL_EXECUTION.name
            })
            .expect("schedule_skill_execution call");
        assert_eq!(
            schedule_call.function,
            AgentRegistry::SCHEDULE_SKILL_EXECUTION.name
        );
    }

    #[test]
    fn execute_and_schedule_prepare_tap_identity_handles_before_peer_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.pure(&3_u64);
        let schedule_agent = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0xa"),
            1,
            false,
        ));
        schedule_skill_execution(
            &mut tx,
            &objects,
            registry,
            schedule_agent,
            11,
            sui::types::Address::from_static("0xc"),
            vec![3],
            TapSchedulePolicy::default(),
            vec![4],
            55,
        )
        .expect("schedule builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();

        let schedule_call = calls
            .iter()
            .find(|call| call.function == AgentRegistry::SCHEDULE_SKILL_EXECUTION.name)
            .expect("schedule_skill_execution call");

        inspector.expect_u64(&schedule_call.arguments[2], 11);
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
            TapPaymentPolicy::UserFunded,
            TapSchedulePolicy::default(),
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
        let immutable_registry = tx.object(sui::tx::ObjectInput::shared(
            *objects.agent_registry.object_id(),
            objects.agent_registry.version(),
            false,
        ));
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
        let prepayment_coin = tx.pure(&7_u64);
        let default_prepayment_coin = tx.pure(&8_u64);

        agent_id_from_address(&mut tx, &objects, sui::types::Address::from_static("0xa"))
            .expect("agent id");
        interface_revision(&mut tx, &objects, InterfaceRevision(3)).expect("interface revision");
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
            TapPaymentPolicy::default(),
            TapSchedulePolicy::default(),
        )
        .expect("update policies");
        let registry = registry_arg(&mut tx);
        let agent = agent_arg(&mut tx);
        schedule_skill_execution_address_funded(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0x80"),
            11,
            prepayment_coin,
            sui::types::Address::from_static("0x81"),
            vec![2],
            100,
            TapSchedulePolicy::default(),
            vec![4],
            vec![5],
            200,
        )
        .expect("address funded schedule");
        schedule_default_dag_executor_skill_execution_address_funded(
            &mut tx,
            &objects,
            immutable_registry,
            sui::types::Address::from_static("0x82"),
            default_prepayment_coin,
            sui::types::Address::from_static("0x83"),
            vec![12],
            100,
            TapSchedulePolicy::default(),
            vec![14],
            vec![15],
            201,
        )
        .expect("default address funded schedule");
        let registry = registry_arg(&mut tx);
        let agent = agent_arg(&mut tx);
        schedule_skill_execution_from_agent_vault(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0x80"),
            11,
            300,
            100,
            TapSchedulePolicy::default(),
            vec![4],
            vec![5],
            200,
        )
        .expect("vault schedule");
        let registry = registry_arg(&mut tx);
        trigger_scheduled_skill_execution(
            &mut tx,
            &objects,
            registry,
            scheduled_task,
            sui::types::Address::from_static("0x90"),
        )
        .expect("trigger schedule");
        for state in [
            TapScheduledOccurrenceFinalState::InFlight,
            TapScheduledOccurrenceFinalState::Accomplished,
            TapScheduledOccurrenceFinalState::Refunded,
        ] {
            complete_scheduled_skill_occurrence(
                &mut tx,
                &objects,
                scheduled_task,
                sui::types::Address::from_static("0x90"),
                sui::types::Address::from_static("0x91"),
                state,
                true,
                500,
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
            TapStandard::INTERFACE_REVISION.name,
            AgentRegistry::GET_SKILL_REQUIREMENTS.name,
            AgentRegistry::UPDATE_DAG.name,
            AgentRegistry::UPDATE_SKILL_POLICIES.name,
            AgentRegistry::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED.name,
            AgentRegistry::SCHEDULE_DEFAULT_DAG_EXECUTOR_SKILL_EXECUTION_ADDRESS_FUNDED.name,
            AgentRegistry::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT.name,
            AgentRegistry::TRIGGER_SCHEDULED_SKILL_EXECUTION.name,
            TapStandard::COMPLETE_SCHEDULED_SKILL_OCCURRENCE.name,
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
    fn default_address_funded_schedule_accepts_immutable_registry() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.object(sui::tx::ObjectInput::shared(
            *objects.agent_registry.object_id(),
            objects.agent_registry.version(),
            false,
        ));
        let prepayment_coin = tx.pure(&8_u64);

        schedule_default_dag_executor_skill_execution_address_funded(
            &mut tx,
            &objects,
            registry,
            sui::types::Address::from_static("0x82"),
            prepayment_coin,
            sui::types::Address::from_static("0x83"),
            vec![12],
            100,
            TapSchedulePolicy::default(),
            vec![14],
            vec![15],
            201,
        )
        .expect("default address funded schedule");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let schedule_call = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .find(|call| {
                call.package == objects.registry_pkg_id
                    && call.function
                        == AgentRegistry::SCHEDULE_DEFAULT_DAG_EXECUTOR_SKILL_EXECUTION_ADDRESS_FUNDED.name
            })
            .expect("default address funded schedule call");
        assert_eq!(schedule_call.arguments.len(), 10);
        inspector.expect_shared_object(&schedule_call.arguments[0], &objects.agent_registry, false);
    }

    #[test]
    fn address_funded_schedule_builder_accepts_scheduled_grant_templates() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = agent_registry_arg(&mut tx, &objects, false).expect("registry");
        let agent = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0xa"),
            1,
            false,
        ));
        let prepayment_coin = tx.pure(&7_u64);

        schedule_skill_execution_address_funded_with_grants(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0x80"),
            11,
            prepayment_coin,
            sui::types::Address::from_static("0x81"),
            vec![2],
            100,
            TapSchedulePolicy::default(),
            vec![4],
            vec![5],
            200,
            vec![TapScheduledAuthorizationGrantTemplate {
                dag_id: sui::types::Address::from_static("0xd"),
                vertex: "demo_delayed_fire_vertex".to_string(),
                tool_package: sui::types::Address::from_static("0xf"),
                tool_module: "demo_delayed_fire_vertex".to_string(),
                tool_function: "execute".to_string(),
                operation_commitment: b"demo-tap-delayed-fire".to_vec(),
                constraints_commitment: Vec::new(),
            }],
        )
        .expect("address funded schedule with grant template succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();

        let template_call = calls
            .iter()
            .find(|call| {
                call.package == objects.interface_pkg_id
                    && call.function == TapStandard::SCHEDULED_AUTHORIZATION_GRANT_TEMPLATE.name
            })
            .expect("scheduled grant template constructor call");
        assert_eq!(template_call.arguments.len(), 7);

        let schedule_call = calls
            .iter()
            .find(|call| {
                call.package == objects.registry_pkg_id
                    && call.function
                        == AgentRegistry::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED_WITH_GRANTS.name
            })
            .expect("address funded schedule with grants call");
        assert_eq!(
            schedule_call.function,
            AgentRegistry::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED_WITH_GRANTS.name
        );
        assert_eq!(schedule_call.arguments.len(), 13);
        inspector.expect_u64(&schedule_call.arguments[3], 11);
    }

    #[test]
    fn agent_vault_schedule_builder_accepts_scheduled_grant_templates() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = agent_registry_arg(&mut tx, &objects, false).expect("registry");
        let agent = tx.object(sui::tx::ObjectInput::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));

        schedule_skill_execution_from_agent_vault_with_grants(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0x80"),
            12,
            50,
            25,
            TapSchedulePolicy::default(),
            vec![4],
            vec![5],
            200,
            vec![TapScheduledAuthorizationGrantTemplate {
                dag_id: sui::types::Address::from_static("0xd"),
                vertex: "demo_delayed_fire_vertex".to_string(),
                tool_package: sui::types::Address::from_static("0xf"),
                tool_module: "demo_delayed_fire_vertex".to_string(),
                tool_function: "execute".to_string(),
                operation_commitment: b"demo-tap-delayed-fire".to_vec(),
                constraints_commitment: Vec::new(),
            }],
        )
        .expect("agent vault schedule with grant template succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(calls.iter().any(|call| {
            call.package == objects.interface_pkg_id
                && call.function == TapStandard::SCHEDULED_AUTHORIZATION_GRANT_TEMPLATE.name
        }));
        let schedule_call = calls
            .iter()
            .find(|call| {
                call.package == objects.registry_pkg_id
                    && call.function
                        == AgentRegistry::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT_WITH_GRANTS.name
            })
            .expect("agent vault schedule with grants call");
        assert_eq!(schedule_call.arguments.len(), 11);
        inspector.expect_u64(&schedule_call.arguments[3], 12);
        inspector.expect_u64(&schedule_call.arguments[4], 50);
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
            TapPaymentPolicy::AgentFunded { max_budget: 100 },
            TapSchedulePolicy::default(),
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
