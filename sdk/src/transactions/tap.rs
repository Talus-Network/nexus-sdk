use crate::{
    idents::{move_std, pure_arg, registry::AgentRegistry, tap::TapStandard},
    sui,
    types::{
        AgentId,
        InterfaceRevision,
        NexusObjects,
        SkillId,
        TapAuthorizedTool,
        TapPaymentPolicy,
        TapSchedulePolicy,
        TapScheduledAuthorizationGrantTemplate,
        TapScheduledOccurrenceFinalState,
        TapSharedObjectRef,
        TapVertexAuthorizationSchema,
    },
};

fn agent_registry_call(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::types::Argument>,
) -> sui::types::Argument {
    tap_call_with_package(tx, objects.registry_pkg_id, ident, args)
}

fn tap_interface_call(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::types::Argument>,
) -> sui::types::Argument {
    tap_call_with_package(tx, objects.interface_pkg_id, ident, args)
}

fn tap_call_with_package(
    tx: &mut sui::tx::TransactionBuilder,
    package: sui::types::Address,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::types::Argument>,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(package, ident.module, ident.name, vec![]),
        args,
    )
}

pub fn agent_registry_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    mutability: bool,
) -> anyhow::Result<sui::types::Argument> {
    let registry = &objects.agent_registry;

    Ok(tx.input(sui::tx::Input::shared(
        *registry.object_id(),
        registry.version(),
        mutability,
    )))
}
pub fn create_agent(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::CREATE_AGENT,
        vec![registry],
    ))
}

pub fn agent_id_from_address(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent_id: AgentId,
) -> anyhow::Result<sui::types::Argument> {
    let agent_id = tx.input(pure_arg(&agent_id)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::AGENT_ID_FROM_ADDRESS,
        vec![agent_id],
    ))
}

pub fn interface_revision(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    interface_revision: InterfaceRevision,
) -> anyhow::Result<sui::types::Argument> {
    let interface_revision = tx.input(pure_arg(&interface_revision.0)?);

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
    registry: sui::types::Argument,
    config_digest: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![registry, tx.input(pure_arg(&config_digest)?)];

    let result = agent_registry_call(
        tx,
        objects,
        AgentRegistry::BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT,
        args,
    );
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn create_skill(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag_id: sui::types::Address,
    description: Vec<u8>,
    workflow_commitment: Vec<u8>,
    requirements_commitment: Vec<u8>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_commitment: Vec<u8>,
    active: bool,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&dag_id)?),
        tx.input(pure_arg(&description)?),
        tx.input(pure_arg(&workflow_commitment)?),
        tx.input(pure_arg(&requirements_commitment)?),
        payment_policy,
        schedule_policy,
        tx.input(pure_arg(&capability_schema_commitment)?),
        tx.input(pure_arg(&active)?),
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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag_id: sui::types::Address,
    workflow_commitment: Vec<u8>,
    requirements_commitment: Vec<u8>,
    metadata_commitment: Vec<u8>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_commitment: Vec<u8>,
    shared_objects: Vec<TapSharedObjectRef>,
    config_digest: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let shared_objects = shared_object_refs_arg(tx, objects, &shared_objects)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&dag_id)?),
        tx.input(pure_arg(&workflow_commitment)?),
        tx.input(pure_arg(&requirements_commitment)?),
        tx.input(pure_arg(&metadata_commitment)?),
        payment_policy,
        schedule_policy,
        tx.input(pure_arg(&capability_schema_commitment)?),
        shared_objects,
        tx.input(pure_arg(&config_digest)?),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::REGISTER_SKILL,
        args,
    ))
}

/// Build a `tap::TapAuthorizedTool` Move value from a typed entry.
pub fn authorized_tool_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool: &TapAuthorizedTool,
) -> anyhow::Result<sui::types::Argument> {
    let package_id = tx.input(pure_arg(&tool.package_id)?);
    let module_name = move_std::Ascii::ascii_string_from_str(tx, tool.module.as_str())?;
    let function_name = move_std::Ascii::ascii_string_from_str(tx, tool.function.as_str())?;
    let operation_commitment = tx.input(pure_arg(&tool.operation_commitment)?);
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::AUTHORIZED_TOOL,
        vec![package_id, module_name, function_name, operation_commitment],
    ))
}

/// Build a `tap::TapVertexAuthorizationSchema` Move value with each `TapAuthorizedTool`
/// individually constructed and pushed into the on-chain `vector`.
pub fn vertex_authorization_schema_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    schema: &TapVertexAuthorizationSchema,
) -> anyhow::Result<sui::types::Argument> {
    let schema_commitment = tx.input(pure_arg(&schema.schema_commitment)?);
    let authorized_tool_type =
        crate::idents::tap::tap_authorized_tool_type(objects.interface_pkg_id);
    let fixed_tools = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
            vec![authorized_tool_type.clone()],
        ),
        vec![],
    );
    // `vector::push_back` mutates by reference and returns nothing — keep the
    // original `fixed_tools` Argument and drop the move-call result.
    for tool in &schema.fixed_tools {
        let tool_arg = authorized_tool_arg(tx, objects, tool)?;
        tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Vector::PUSH_BACK.module,
                move_std::Vector::PUSH_BACK.name,
                vec![authorized_tool_type.clone()],
            ),
            vec![fixed_tools, tool_arg],
        );
    }
    let requires_payment = tx.input(pure_arg(&schema.requires_payment)?);
    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::VERTEX_AUTHORIZATION_SCHEMA,
        vec![schema_commitment, fixed_tools, requires_payment],
    ))
}

/// Variant of `register_skill` that passes the full `TapVertexAuthorizationSchema`.
/// Required when the skill is cap-gated (non-empty `fixed_tools` or
/// `requires_payment = true`); the chain reconstructs the requirements digest with
/// the schema baked in, so the simpler `register_skill` would fail the config
/// digest assertion.
#[allow(clippy::too_many_arguments)]
pub fn register_skill_with_vertex_authorization_schema(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag_id: sui::types::Address,
    workflow_commitment: Vec<u8>,
    requirements_commitment: Vec<u8>,
    metadata_commitment: Vec<u8>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_commitment: Vec<u8>,
    vertex_authorization_schema: &TapVertexAuthorizationSchema,
    shared_objects: Vec<TapSharedObjectRef>,
    config_digest: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let shared_objects = shared_object_refs_arg(tx, objects, &shared_objects)?;
    let vertex_authorization_schema =
        vertex_authorization_schema_arg(tx, objects, vertex_authorization_schema)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&dag_id)?),
        tx.input(pure_arg(&workflow_commitment)?),
        tx.input(pure_arg(&requirements_commitment)?),
        tx.input(pure_arg(&metadata_commitment)?),
        payment_policy,
        schedule_policy,
        tx.input(pure_arg(&capability_schema_commitment)?),
        vertex_authorization_schema,
        shared_objects,
        tx.input(pure_arg(&config_digest)?),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::REGISTER_SKILL_WITH_VERTEX_AUTHORIZATION_SCHEMA,
        args,
    ))
}

pub fn get_skill_requirements(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::GET_SKILL_REQUIREMENTS,
        vec![registry, agent, skill_id],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn announce_endpoint_revision(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
    shared_objects: Vec<TapSharedObjectRef>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_commitment: Vec<u8>,
    config_digest: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let shared_objects = shared_object_refs_arg(tx, objects, &shared_objects)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&interface_revision)?),
        shared_objects,
        payment_policy,
        schedule_policy,
        tx.input(pure_arg(&capability_schema_commitment)?),
        tx.input(pure_arg(&config_digest)?),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::ANNOUNCE_ENDPOINT_REVISION,
        args,
    ))
}

pub fn set_skill_active_revision(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&interface_revision)?),
    ];

    Ok(agent_registry_call(
        tx,
        objects,
        AgentRegistry::SET_SKILL_ACTIVE_REVISION,
        args,
    ))
}

pub fn set_skill_active(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    active: bool,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&active)?),
    ];

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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    active: bool,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![registry, agent, tx.input(pure_arg(&active)?)];

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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    description: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&description)?),
    ];

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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    dag_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&dag_id)?),
    ];

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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    execution_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&execution_id)?),
    ];

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
    registry: sui::types::Argument,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<sui::types::Argument> {
    let agent_id = agent_id_from_address(tx, objects, agent_id)?;
    let args = vec![registry, agent_id, tx.input(pure_arg(&skill_id)?)];

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
    registry: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
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
    registry: sui::types::Argument,
    worksheet: sui::types::Argument,
) -> sui::types::Argument {
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
    pub refund_mode: u8,
}

impl AgentSkillPaymentInput {
    pub fn invoker_source(
        agent_id: AgentId,
        skill_id: SkillId,
        invoker: sui::types::Address,
        max_budget: u64,
        refund_mode: u8,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            agent_id,
            skill_id,
            source: crate::types::tap_payment_source_for_address(invoker)?,
            max_budget,
            refund_mode,
        })
    }

    /// Build direct payment source bytes for an agent-funded policy.
    ///
    /// This is the source encoding accepted by the standard TAP payment policy.
    pub fn agent_vault_source(
        agent_id: AgentId,
        skill_id: SkillId,
        max_budget: u64,
        refund_mode: u8,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            agent_id,
            skill_id,
            source: crate::types::tap_payment_source_for_address(agent_id)?,
            max_budget,
            refund_mode,
        })
    }
}

pub fn deposit_agent_payment_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
    coin: sui::types::Argument,
) -> sui::types::Argument {
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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    amount: u64,
) -> anyhow::Result<sui::types::Argument> {
    // The TAP Move module checks mutable agent custody through the registry.
    let amount = tx.input(pure_arg(&amount)?);

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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    long_term_gas_coin_id: sui::types::Address,
    refill_policy_commitment: Vec<u8>,
    schedule_policy: TapSchedulePolicy,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        skill_id,
        tx.input(pure_arg(&long_term_gas_coin_id)?),
        tx.input(pure_arg(&refill_policy_commitment)?),
        schedule_policy,
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepayment_coin: sui::types::Argument,
    refund_recipient: sui::types::Address,
    payment_source: Vec<u8>,
    occurrence_budget: u64,
    refund_mode: u8,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&scheduler_task_id)?),
        skill_id,
        prepayment_coin,
        tx.input(pure_arg(&refund_recipient)?),
        tx.input(pure_arg(&payment_source)?),
        tx.input(pure_arg(&occurrence_budget)?),
        tx.input(pure_arg(&refund_mode)?),
        schedule_policy,
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepayment_coin: sui::types::Argument,
    refund_recipient: sui::types::Address,
    payment_source: Vec<u8>,
    occurrence_budget: u64,
    refund_mode: u8,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
    grant_templates: Vec<TapScheduledAuthorizationGrantTemplate>,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let grant_templates =
        scheduled_authorization_grant_templates_arg(tx, objects, &grant_templates)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&scheduler_task_id)?),
        skill_id,
        prepayment_coin,
        tx.input(pure_arg(&refund_recipient)?),
        tx.input(pure_arg(&payment_source)?),
        tx.input(pure_arg(&occurrence_budget)?),
        tx.input(pure_arg(&refund_mode)?),
        schedule_policy,
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
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
    registry: sui::types::Argument,
    scheduler_task_id: sui::types::Address,
    prepayment_coin: sui::types::Argument,
    refund_recipient: sui::types::Address,
    payment_source: Vec<u8>,
    occurrence_budget: u64,
    refund_mode: u8,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        tx.input(pure_arg(&scheduler_task_id)?),
        prepayment_coin,
        tx.input(pure_arg(&refund_recipient)?),
        tx.input(pure_arg(&payment_source)?),
        tx.input(pure_arg(&occurrence_budget)?),
        tx.input(pure_arg(&refund_mode)?),
        schedule_policy,
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepay_amount: u64,
    occurrence_budget: u64,
    refund_mode: u8,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&scheduler_task_id)?),
        skill_id,
        tx.input(pure_arg(&prepay_amount)?),
        tx.input(pure_arg(&occurrence_budget)?),
        tx.input(pure_arg(&refund_mode)?),
        schedule_policy,
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
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
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    scheduler_task_id: sui::types::Address,
    skill_id: SkillId,
    prepay_amount: u64,
    occurrence_budget: u64,
    refund_mode: u8,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
    grant_templates: Vec<TapScheduledAuthorizationGrantTemplate>,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let grant_templates =
        scheduled_authorization_grant_templates_arg(tx, objects, &grant_templates)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&scheduler_task_id)?),
        skill_id,
        tx.input(pure_arg(&prepay_amount)?),
        tx.input(pure_arg(&occurrence_budget)?),
        tx.input(pure_arg(&refund_mode)?),
        schedule_policy,
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
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
) -> anyhow::Result<sui::types::Argument> {
    let recurrence_kind =
        move_std::Ascii::ascii_string_from_str(tx, &schedule_policy.recurrence_kind)?;
    let min_interval_ms = tx.input(pure_arg(&schedule_policy.min_interval_ms)?);
    let max_occurrences = tx.input(pure_arg(&schedule_policy.max_occurrences)?);
    let allow_recursive = tx.input(pure_arg(&schedule_policy.allow_recursive)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::SCHEDULE_POLICY,
        vec![
            recurrence_kind,
            min_interval_ms,
            max_occurrences,
            allow_recursive,
        ],
    ))
}

fn payment_mode_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    mode: &crate::types::TapPaymentMode,
) -> anyhow::Result<sui::types::Argument> {
    match mode {
        crate::types::TapPaymentMode::UserFunded => Ok(tap_interface_call(
            tx,
            objects,
            TapStandard::PAYMENT_MODE_USER_FUNDED,
            vec![],
        )),
        crate::types::TapPaymentMode::AgentFunded => Ok(tap_interface_call(
            tx,
            objects,
            TapStandard::PAYMENT_MODE_AGENT_FUNDED,
            vec![],
        )),
    }
}

fn payment_policy_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    payment_policy: &TapPaymentPolicy,
) -> anyhow::Result<sui::types::Argument> {
    let mode = payment_mode_arg(tx, objects, &payment_policy.mode)?;
    let max_budget = tx.input(pure_arg(&payment_policy.max_budget)?);
    let token_type_commitment = tx.input(pure_arg(&payment_policy.token_type_commitment)?);
    let refund_mode = tx.input(pure_arg(&payment_policy.refund_mode)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::PAYMENT_POLICY,
        vec![mode, max_budget, token_type_commitment, refund_mode],
    ))
}

fn shared_object_ref_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    shared_object: &TapSharedObjectRef,
) -> anyhow::Result<sui::types::Argument> {
    let id = tx.input(pure_arg(&shared_object.id)?);
    let mutable = tx.input(pure_arg(&shared_object.mutable)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::SHARED_OBJECT_REF,
        vec![id, mutable],
    ))
}

fn shared_object_refs_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    shared_objects: &[TapSharedObjectRef],
) -> anyhow::Result<sui::types::Argument> {
    let shared_object_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        objects.interface_pkg_id,
        crate::idents::tap::STANDARD_TAP_MODULE,
        sui::types::Identifier::from_static("TapSharedObjectRef"),
        vec![],
    )));
    let vector = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
            vec![shared_object_type.clone()],
        ),
        vec![],
    );

    for shared_object in shared_objects {
        let item = shared_object_ref_arg(tx, objects, shared_object)?;
        tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Vector::PUSH_BACK.module,
                move_std::Vector::PUSH_BACK.name,
                vec![shared_object_type.clone()],
            ),
            vec![vector, item],
        );
    }

    Ok(vector)
}

fn scheduled_authorization_grant_template_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    template: &TapScheduledAuthorizationGrantTemplate,
) -> anyhow::Result<sui::types::Argument> {
    let dag_id = tx.input(pure_arg(&template.dag_id)?);
    let vertex = move_std::Ascii::ascii_string_from_str(tx, &template.vertex)?;
    let tool_package = tx.input(pure_arg(&template.tool_package)?);
    let tool_module = move_std::Ascii::ascii_string_from_str(tx, &template.tool_module)?;
    let tool_function = move_std::Ascii::ascii_string_from_str(tx, &template.tool_function)?;
    let operation_commitment = tx.input(pure_arg(&template.operation_commitment)?);
    let constraints_commitment = tx.input(pure_arg(&template.constraints_commitment)?);
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
) -> anyhow::Result<sui::types::Argument> {
    let template_type =
        crate::idents::tap::scheduled_authorization_grant_template_type(objects.interface_pkg_id);
    let vector = tx.move_call(
        sui::tx::Function::new(
            move_std::PACKAGE_ID,
            move_std::Vector::EMPTY.module,
            move_std::Vector::EMPTY.name,
            vec![template_type.clone()],
        ),
        vec![],
    );

    for template in grant_templates {
        let item = scheduled_authorization_grant_template_arg(tx, objects, template)?;
        tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                move_std::Vector::PUSH_BACK.module,
                move_std::Vector::PUSH_BACK.name,
                vec![template_type.clone()],
            ),
            vec![vector, item],
        );
    }

    Ok(vector)
}

pub fn trigger_scheduled_skill_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    scheduled_task: sui::types::Argument,
    execution_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let execution_id = tx.input(pure_arg(&execution_id)?);
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
    scheduled_task: sui::types::Argument,
    execution_id: sui::types::Address,
    payment_id: sui::types::Address,
    final_state: TapScheduledOccurrenceFinalState,
    continue_recurring: bool,
    next_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let final_state = scheduled_occurrence_final_state_arg(tx, objects, final_state);
    let execution_id = tx.input(pure_arg(&execution_id)?);
    let payment_id = tx.input(pure_arg(&payment_id)?);
    let continue_recurring = tx.input(pure_arg(&continue_recurring)?);
    let next_after_ms = tx.input(pure_arg(&next_after_ms)?);
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
) -> sui::types::Argument {
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
    use {
        super::*,
        crate::{test_utils::sui_mocks, types::TapPaymentMode},
    };

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
            let sui::types::Input::Pure { value } = self.input(argument) else {
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
            let sui::types::Input::Shared {
                object_id,
                initial_shared_version,
                mutable,
            } = self.input(argument)
            else {
                panic!("expected shared object input");
            };
            assert_eq!(object_id, expected.object_id());
            assert_eq!(*initial_shared_version, expected.version());
            assert_eq!(*mutable, expected_mutable);
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
        let registry = tx.input(pure_arg(&1_u64).unwrap());

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
        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, TapStandard::AGENT_ID_FROM_ADDRESS.module);
        assert_eq!(call.function, TapStandard::AGENT_ID_FROM_ADDRESS.name);
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
        let immutable_registry =
            agent_registry_arg(&mut immutable_tx, &objects, false).expect("immutable registry");
        let immutable_inspector =
            TxInspector::new(sui_mocks::mock_finish_transaction(immutable_tx));
        immutable_inspector.expect_shared_object(
            &immutable_registry,
            &objects.agent_registry,
            false,
        );

        let mut mutable_tx = sui::tx::TransactionBuilder::new();
        let mutable_registry =
            agent_registry_arg(&mut mutable_tx, &objects, true).expect("mutable registry");
        let mutable_inspector = TxInspector::new(sui_mocks::mock_finish_transaction(mutable_tx));
        mutable_inspector.expect_shared_object(&mutable_registry, &objects.agent_registry, true);
    }

    #[test]
    fn execute_and_schedule_use_peer_standard_tap_idents() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&3_u64).unwrap());
        let schedule_agent = tx.input(sui::tx::Input::shared(
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
        let registry = tx.input(pure_arg(&3_u64).unwrap());
        let schedule_agent = tx.input(sui::tx::Input::shared(
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
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let agent = tx.input(sui::tx::Input::shared(
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
            vec![1],
            vec![2],
            vec![3],
            TapPaymentPolicy {
                mode: TapPaymentMode::UserFunded,
                max_budget: 100,
                token_type_commitment: Vec::new(),
                refund_mode: 0,
            },
            TapSchedulePolicy::default(),
            vec![4],
            vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0x10"),
            )],
            vec![6],
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
        assert_eq!(call.arguments.len(), 11);
    }

    #[test]
    fn payment_and_vault_builders_target_standard_tap_functions() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = agent_registry_arg(&mut tx, &objects, true).expect("registry");
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));
        let vault_coin = tx.input(pure_arg(&10_u64).unwrap());

        let invoker_input = AgentSkillPaymentInput::invoker_source(
            sui::types::Address::from_static("0xa"),
            11,
            sui::types::Address::from_static("0x1"),
            100,
            0,
        )
        .expect("invoker source");
        assert_eq!(invoker_input.skill_id, 11);
        let vault_input = AgentSkillPaymentInput::agent_vault_source(
            sui::types::Address::from_static("0xa"),
            12,
            101,
            1,
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
        let registry = agent_registry_arg(&mut tx, &objects, true).expect("registry");
        let immutable_registry = tx.input(sui::tx::Input::shared(
            *objects.agent_registry.object_id(),
            objects.agent_registry.version(),
            false,
        ));
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));
        let scheduled_task = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0x50"),
            3,
            true,
        ));
        let prepayment_coin = tx.input(pure_arg(&7_u64).unwrap());
        let default_prepayment_coin = tx.input(pure_arg(&8_u64).unwrap());

        agent_id_from_address(&mut tx, &objects, sui::types::Address::from_static("0xa"))
            .expect("agent id");
        interface_revision(&mut tx, &objects, InterfaceRevision(3)).expect("interface revision");
        get_skill_requirements(&mut tx, &objects, registry, agent, 11).expect("requirements");
        announce_endpoint_revision(
            &mut tx,
            &objects,
            registry,
            agent,
            11,
            InterfaceRevision(3),
            vec![TapSharedObjectRef::mutable(
                sui::types::Address::from_static("0x61"),
            )],
            TapPaymentPolicy::default(),
            TapSchedulePolicy::default(),
            vec![8],
            vec![9],
        )
        .expect("announce");
        set_skill_active_revision(&mut tx, &objects, registry, agent, 11, InterfaceRevision(3))
            .expect("set active");
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
            0,
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
            0,
            TapSchedulePolicy::default(),
            vec![14],
            vec![15],
            201,
        )
        .expect("default address funded schedule");
        schedule_skill_execution_from_agent_vault(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0x80"),
            11,
            300,
            100,
            0,
            TapSchedulePolicy::default(),
            vec![4],
            vec![5],
            200,
        )
        .expect("vault schedule");
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
            TapStandard::AGENT_ID_FROM_ADDRESS.name,
            TapStandard::INTERFACE_REVISION.name,
            AgentRegistry::GET_SKILL_REQUIREMENTS.name,
            AgentRegistry::ANNOUNCE_ENDPOINT_REVISION.name,
            AgentRegistry::SET_SKILL_ACTIVE_REVISION.name,
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
        let registry = tx.input(sui::tx::Input::shared(
            *objects.agent_registry.object_id(),
            objects.agent_registry.version(),
            false,
        ));
        let prepayment_coin = tx.input(pure_arg(&8_u64).unwrap());

        schedule_default_dag_executor_skill_execution_address_funded(
            &mut tx,
            &objects,
            registry,
            sui::types::Address::from_static("0x82"),
            prepayment_coin,
            sui::types::Address::from_static("0x83"),
            vec![12],
            100,
            0,
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
        assert_eq!(schedule_call.arguments.len(), 11);
        inspector.expect_shared_object(&schedule_call.arguments[0], &objects.agent_registry, false);
    }

    #[test]
    fn address_funded_schedule_builder_accepts_scheduled_grant_templates() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = agent_registry_arg(&mut tx, &objects, false).expect("registry");
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            false,
        ));
        let prepayment_coin = tx.input(pure_arg(&7_u64).unwrap());

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
            0,
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
        assert_eq!(schedule_call.arguments.len(), 14);
        inspector.expect_u64(&schedule_call.arguments[3], 11);
    }

    #[test]
    fn agent_vault_schedule_builder_accepts_scheduled_grant_templates() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = agent_registry_arg(&mut tx, &objects, false).expect("registry");
        let agent = tx.input(sui::tx::Input::shared(
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
            0,
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
        assert_eq!(schedule_call.arguments.len(), 12);
        inspector.expect_u64(&schedule_call.arguments[3], 12);
        inspector.expect_u64(&schedule_call.arguments[4], 50);
    }

    #[test]
    fn register_skill_builder_supports_agent_funded_payment_mode() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let agent = tx.input(sui::tx::Input::shared(
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
            vec![1],
            vec![2],
            vec![3],
            TapPaymentPolicy {
                mode: TapPaymentMode::AgentFunded,
                max_budget: 100,
                token_type_commitment: Vec::new(),
                refund_mode: 0,
            },
            TapSchedulePolicy::default(),
            vec![4],
            vec![],
            vec![6],
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
            function_names.contains(&TapStandard::PAYMENT_MODE_AGENT_FUNDED.name),
            "missing agent-funded payment mode constructor"
        );
    }

    fn authorized_tool_fixture() -> TapAuthorizedTool {
        TapAuthorizedTool {
            package_id: sui::types::Address::from_static("0x42"),
            module: "tap_demo".to_string(),
            function: "execute_authorized".to_string(),
            operation_commitment: vec![1, 2, 3],
        }
    }

    #[test]
    fn authorized_tool_arg_targets_tap_authorized_tool_constructor() {
        // The cap-gated register path needs to construct a Move
        // `TapAuthorizedTool` value from a typed entry before assembling the
        // schema. Verify the helper emits exactly one move call to
        // `tap::authorized_tool` on the interface package and passes the four
        // expected arguments — a regression that drops one of these would
        // mismatch the requirements digest on chain.
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let tool = authorized_tool_fixture();
        authorized_tool_arg(&mut tx, &objects, &tool).expect("authorized tool arg");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let authorized_call = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .find(|call| {
                call.package == objects.interface_pkg_id
                    && call.module == TapStandard::AUTHORIZED_TOOL.module
                    && call.function == TapStandard::AUTHORIZED_TOOL.name
            })
            .expect("authorized_tool call");
        // (package_id, module_name, function_name, operation_commitment).
        assert_eq!(authorized_call.arguments.len(), 4);
    }

    #[test]
    fn vertex_authorization_schema_arg_builds_fixed_tools_vector_in_order() {
        // The schema helper must build the `fixed_tools` vector with one
        // `vector::push_back` per entry and a single trailing
        // `tap::vertex_authorization_schema` call. We assert the count of
        // both move calls so a refactor that drops a push or collapses the
        // vector into a pure input is caught — the on-chain digest binds the
        // exact construction sequence, so any drift breaks register_skill.
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let schema = TapVertexAuthorizationSchema {
            schema_commitment: vec![9],
            fixed_tools: vec![authorized_tool_fixture(), authorized_tool_fixture()],
            requires_payment: true,
        };

        vertex_authorization_schema_arg(&mut tx, &objects, &schema).expect("schema arg");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let move_calls: Vec<&sui::types::MoveCall> = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect();

        let empty_calls = move_calls
            .iter()
            .filter(|call| call.function == move_std::Vector::EMPTY.name)
            .count();
        let push_back_calls = move_calls
            .iter()
            .filter(|call| call.function == move_std::Vector::PUSH_BACK.name)
            .count();
        let schema_calls = move_calls
            .iter()
            .filter(|call| {
                call.package == objects.interface_pkg_id
                    && call.function == TapStandard::VERTEX_AUTHORIZATION_SCHEMA.name
            })
            .count();
        let authorized_tool_calls = move_calls
            .iter()
            .filter(|call| call.function == TapStandard::AUTHORIZED_TOOL.name)
            .count();

        assert_eq!(empty_calls, 1, "exactly one vector::empty for fixed_tools");
        assert_eq!(push_back_calls, 2, "one push_back per fixed_tool");
        assert_eq!(authorized_tool_calls, 2, "one authorized_tool per entry");
        assert_eq!(
            schema_calls, 1,
            "one trailing vertex_authorization_schema call"
        );
    }

    #[test]
    fn vertex_authorization_schema_arg_empty_fixed_tools_skips_push_back() {
        // The default/empty schema produces a `vector::empty` followed
        // immediately by the `vertex_authorization_schema` constructor; no
        // push_back calls should run. This is the shape the SDK passes when
        // the higher-level `is_default` check is true and the caller chose
        // the cap-gated path anyway (e.g. `requires_payment = true` with no
        // fixed tools).
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let schema = TapVertexAuthorizationSchema {
            schema_commitment: Vec::new(),
            fixed_tools: Vec::new(),
            requires_payment: true,
        };

        vertex_authorization_schema_arg(&mut tx, &objects, &schema).expect("empty schema arg");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let push_back_calls = inspector
            .commands()
            .iter()
            .filter(|command| {
                matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.function == move_std::Vector::PUSH_BACK.name
                )
            })
            .count();
        assert_eq!(push_back_calls, 0);
    }

    #[test]
    fn register_skill_with_vertex_authorization_schema_routes_through_cap_gated_entrypoint() {
        // The cap-gated registration path must route through
        // `register_skill_with_vertex_authorization_schema` on the registry
        // package — never the simpler `register_skill`. Routing through the
        // wrong entrypoint causes an on-chain digest mismatch because the
        // chain reconstructs requirements with the full schema. We assert
        // the right registry call and that the schema sub-builder is
        // invoked (one `vertex_authorization_schema` interface call).
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));
        let schema = TapVertexAuthorizationSchema {
            schema_commitment: vec![1, 2],
            fixed_tools: vec![authorized_tool_fixture()],
            requires_payment: false,
        };

        register_skill_with_vertex_authorization_schema(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0xd"),
            vec![1],
            vec![2],
            vec![3],
            TapPaymentPolicy {
                mode: TapPaymentMode::UserFunded,
                max_budget: 100,
                token_type_commitment: Vec::new(),
                refund_mode: 0,
            },
            TapSchedulePolicy::default(),
            vec![4],
            &schema,
            vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0x10"),
            )],
            vec![6],
        )
        .expect("cap-gated register builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let move_calls: Vec<&sui::types::MoveCall> = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect();

        let cap_gated_register = move_calls
            .iter()
            .find(|call| {
                call.package == objects.registry_pkg_id
                    && call.module
                        == AgentRegistry::REGISTER_SKILL_WITH_VERTEX_AUTHORIZATION_SCHEMA.module
                    && call.function
                        == AgentRegistry::REGISTER_SKILL_WITH_VERTEX_AUTHORIZATION_SCHEMA.name
            })
            .expect("cap-gated register call");
        // (registry, agent, dag_id, workflow_commitment, requirements_commitment,
        //  metadata_commitment, payment_policy, schedule_policy,
        //  capability_schema_commitment, vertex_authorization_schema,
        //  shared_objects, config_digest)
        assert_eq!(cap_gated_register.arguments.len(), 12);

        // Verify the schema sub-builder also ran (no regression collapsing it
        // into a bare input).
        let schema_call_present = move_calls.iter().any(|call| {
            call.package == objects.interface_pkg_id
                && call.function == TapStandard::VERTEX_AUTHORIZATION_SCHEMA.name
        });
        assert!(
            schema_call_present,
            "vertex_authorization_schema constructor must run inside cap-gated register"
        );

        // The simpler register_skill entrypoint must NOT have been called.
        let non_cap_gated_register = move_calls.iter().any(|call| {
            call.package == objects.registry_pkg_id
                && call.function == AgentRegistry::REGISTER_SKILL.name
        });
        assert!(
            !non_cap_gated_register,
            "cap-gated path must not fall back to plain register_skill"
        );
    }
}
