use crate::{
    idents::{move_std, pure_arg, sui_framework, tap::TapStandard},
    sui,
    types::{
        AgentId,
        InterfaceRevision,
        NexusObjects,
        SkillId,
        TapPaymentPolicy,
        TapSchedulePolicy,
        TapScheduledOccurrenceFinalState,
        TapSharedObjectRef,
    },
};

fn tap_registry_call(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::types::Argument>,
) -> sui::types::Argument {
    tap_call_with_package(tx, objects.registry_pkg_id(), ident, args)
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

pub fn tap_registry_arg(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::types::Argument> {
    let registry = objects
        .tap_registry()
        .ok_or_else(|| anyhow::anyhow!("NexusObjects missing tap_registry object reference"))?;

    Ok(tx.input(sui::tx::Input::shared(
        *registry.object_id(),
        registry.version(),
        true,
    )))
}

pub fn create_agent(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    operator: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let operator = tx.input(pure_arg(&operator)?);

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::CREATE_AGENT,
        vec![registry, operator],
    ))
}

fn public_share_tap_identity(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
            vec![crate::idents::tap::agent_type(objects.interface_pkg_id)],
        ),
        vec![agent],
    )
}

pub fn create_standard_endpoint(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    package_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let package_id = tx.input(pure_arg(&package_id)?);
    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::CREATE_STANDARD_ENDPOINT,
        vec![package_id],
    ))
}

pub fn share_standard_endpoint(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    endpoint: sui::types::Argument,
) -> sui::types::Argument {
    tap_registry_call(
        tx,
        objects,
        TapStandard::SHARE_STANDARD_ENDPOINT,
        vec![endpoint],
    )
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

pub fn skill_id_from_u64(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    skill_id: SkillId,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.input(pure_arg(&skill_id)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::SKILL_ID_FROM_U64,
        vec![skill_id],
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
    operator: sui::types::Address,
    tap_package_id: sui::types::Address,
    endpoint_object_id: sui::types::Address,
    endpoint_object_version: u64,
    endpoint_object_digest: Vec<u8>,
    config_digest: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        tx.input(pure_arg(&operator)?),
        tx.input(pure_arg(&tap_package_id)?),
        tx.input(pure_arg(&endpoint_object_id)?),
        tx.input(pure_arg(&endpoint_object_version)?),
        tx.input(pure_arg(&endpoint_object_digest)?),
        tx.input(pure_arg(&config_digest)?),
    ];

    let result = tap_registry_call(
        tx,
        objects,
        TapStandard::BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT,
        args,
    );
    let agent = result
        .nested(0)
        .ok_or_else(|| anyhow::anyhow!("default TAP deployment bootstrap did not return Agent"))?;
    public_share_tap_identity(tx, objects, agent);

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn register_skill(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag_id: sui::types::Address,
    tap_package_id: sui::types::Address,
    workflow_commitment: Vec<u8>,
    requirements_commitment: Vec<u8>,
    metadata_commitment: Vec<u8>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_commitment: Vec<u8>,
    endpoint_object_id: sui::types::Address,
    endpoint_object_version: u64,
    endpoint_object_digest: Vec<u8>,
    shared_objects: Vec<TapSharedObjectRef>,
    config_digest: Vec<u8>,
    active_for_new_executions: bool,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let shared_objects = shared_object_refs_arg(tx, objects, &shared_objects)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&dag_id)?),
        tx.input(pure_arg(&tap_package_id)?),
        tx.input(pure_arg(&workflow_commitment)?),
        tx.input(pure_arg(&requirements_commitment)?),
        tx.input(pure_arg(&metadata_commitment)?),
        payment_policy,
        schedule_policy,
        tx.input(pure_arg(&capability_schema_commitment)?),
        tx.input(pure_arg(&endpoint_object_id)?),
        tx.input(pure_arg(&endpoint_object_version)?),
        tx.input(pure_arg(&endpoint_object_digest)?),
        shared_objects,
        tx.input(pure_arg(&config_digest)?),
        tx.input(pure_arg(&active_for_new_executions)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::REGISTER_SKILL,
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

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::GET_SKILL_REQUIREMENTS,
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
    endpoint_object_id: sui::types::Address,
    endpoint_object_version: u64,
    endpoint_object_digest: Vec<u8>,
    shared_objects: Vec<TapSharedObjectRef>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_commitment: Vec<u8>,
    config_digest: Vec<u8>,
    active_for_new_executions: bool,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = payment_policy_arg(tx, objects, &payment_policy)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let shared_objects = shared_object_refs_arg(tx, objects, &shared_objects)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&interface_revision)?),
        tx.input(pure_arg(&endpoint_object_id)?),
        tx.input(pure_arg(&endpoint_object_version)?),
        tx.input(pure_arg(&endpoint_object_digest)?),
        shared_objects,
        payment_policy,
        schedule_policy,
        tx.input(pure_arg(&capability_schema_commitment)?),
        tx.input(pure_arg(&config_digest)?),
        tx.input(pure_arg(&active_for_new_executions)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::ANNOUNCE_ENDPOINT_REVISION,
        args,
    ))
}

pub fn set_active_endpoint_revision(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
    active_for_new_executions: bool,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&interface_revision)?),
        tx.input(pure_arg(&active_for_new_executions)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::SET_ACTIVE_ENDPOINT_REVISION,
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

    Ok(tap_registry_call(tx, objects, TapStandard::WORKSHEET, args))
}

pub fn workflow_worksheet(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![registry, agent, tx.input(pure_arg(&skill_id)?)];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::WORKFLOW_WORKSHEET,
        args,
    ))
}

pub fn confirm_tool_eval_for_walk(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    worksheet: sui::types::Argument,
) -> sui::types::Argument {
    tap_registry_call(
        tx,
        objects,
        TapStandard::CONFIRM_TOOL_EVAL_FOR_WALK,
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
    /// This is the source encoding accepted by the direct Move
    /// `create_agent_skill_payment` policy check. To settle from the agent's
    /// on-chain vault, use `create_agent_skill_payment_from_vault` instead.
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

pub fn create_agent_skill_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    payment_coin: sui::types::Argument,
    execution_id: sui::types::Address,
    input: AgentSkillPaymentInput,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = skill_id_from_u64(tx, objects, input.skill_id)?;
    let args = vec![
        registry,
        agent,
        skill_id,
        tx.input(pure_arg(&execution_id)?),
        payment_coin,
        tx.input(pure_arg(&input.source)?),
        tx.input(pure_arg(&input.max_budget)?),
        tx.input(pure_arg(&input.refund_mode)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::CREATE_AGENT_SKILL_PAYMENT,
        args,
    ))
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
    // The TAP Move module checks that the transaction sender is the agent
    // owner or operator registered in the TapRegistry.
    let amount = tx.input(pure_arg(&amount)?);

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::WITHDRAW_AGENT_PAYMENT_VAULT,
        vec![registry, agent, amount],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn create_agent_skill_payment_from_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    execution_id: sui::types::Address,
    max_budget: u64,
    refund_mode: u8,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = skill_id_from_u64(tx, objects, skill_id)?;
    let args = vec![
        registry,
        agent,
        skill_id,
        tx.input(pure_arg(&execution_id)?),
        tx.input(pure_arg(&max_budget)?),
        tx.input(pure_arg(&refund_mode)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::CREATE_AGENT_SKILL_PAYMENT_FROM_VAULT,
        args,
    ))
}

pub fn consume_gas_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
    execution_id: sui::types::Address,
    endpoint_object_id: sui::types::Address,
    leader_cap_id: sui::types::Address,
    amount: u64,
) -> anyhow::Result<sui::types::Argument> {
    let execution_id = tx.input(pure_arg(&execution_id)?);
    let endpoint_object_id = tx.input(pure_arg(&endpoint_object_id)?);
    let leader_cap_id = tx.input(pure_arg(&leader_cap_id)?);
    let amount = tx.input(pure_arg(&amount)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::CONSUME_GAS_PAYMENT,
        vec![
            agent,
            execution_id,
            endpoint_object_id,
            leader_cap_id,
            amount,
        ],
    ))
}

pub fn accomplish_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
    execution_id: sui::types::Address,
    endpoint_object_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let execution_id = tx.input(pure_arg(&execution_id)?);
    let endpoint_object_id = tx.input(pure_arg(&endpoint_object_id)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::ACCOMPLISH_EXECUTION,
        vec![agent, execution_id, endpoint_object_id],
    ))
}

pub fn refund_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    agent: sui::types::Argument,
    execution_id: sui::types::Address,
    endpoint_object_id: sui::types::Address,
    refund_reason: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let execution_id = tx.input(pure_arg(&execution_id)?);
    let endpoint_object_id = tx.input(pure_arg(&endpoint_object_id)?);
    let refund_reason = tx.input(pure_arg(&refund_reason)?);

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::REFUND_EXECUTION,
        vec![agent, execution_id, endpoint_object_id, refund_reason],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_agent_skill(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    input_commitment: Vec<u8>,
    payment_id: sui::types::Argument,
    execution_id: sui::types::Address,
    authorization_plan_commitment: Option<Vec<u8>>,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = skill_id_from_u64(tx, objects, skill_id)?;
    let args = vec![
        registry,
        agent,
        skill_id,
        tx.input(pure_arg(&input_commitment)?),
        payment_id,
        tx.input(pure_arg(&execution_id)?),
        tx.input(pure_arg(&authorization_plan_commitment)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::EXECUTE_AGENT_SKILL,
        args,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn schedule_skill_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    skill_id: SkillId,
    input_commitment: Vec<u8>,
    long_term_gas_coin_id: sui::types::Address,
    refill_policy_commitment: Vec<u8>,
    authorization_plan_commitment: Option<Vec<u8>>,
    schedule_policy: TapSchedulePolicy,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = skill_id_from_u64(tx, objects, skill_id)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        skill_id,
        tx.input(pure_arg(&input_commitment)?),
        tx.input(pure_arg(&long_term_gas_coin_id)?),
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&authorization_plan_commitment)?),
        schedule_policy,
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::SCHEDULE_SKILL_EXECUTION,
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
    input_commitment: Vec<u8>,
    prepayment_coin: sui::types::Argument,
    refund_recipient: sui::types::Address,
    payment_source: Vec<u8>,
    occurrence_budget: u64,
    refund_mode: u8,
    authorization_plan_commitment: Option<Vec<u8>>,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = skill_id_from_u64(tx, objects, skill_id)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&scheduler_task_id)?),
        skill_id,
        tx.input(pure_arg(&input_commitment)?),
        prepayment_coin,
        tx.input(pure_arg(&refund_recipient)?),
        tx.input(pure_arg(&payment_source)?),
        tx.input(pure_arg(&occurrence_budget)?),
        tx.input(pure_arg(&refund_mode)?),
        tx.input(pure_arg(&authorization_plan_commitment)?),
        schedule_policy,
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
    ];

    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED,
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
    input_commitment: Vec<u8>,
    prepay_amount: u64,
    occurrence_budget: u64,
    refund_mode: u8,
    authorization_plan_commitment: Option<Vec<u8>>,
    schedule_policy: TapSchedulePolicy,
    refill_policy_commitment: Vec<u8>,
    schedule_entries_commitment: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = skill_id_from_u64(tx, objects, skill_id)?;
    let schedule_policy = schedule_policy_arg(tx, objects, &schedule_policy)?;
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&scheduler_task_id)?),
        skill_id,
        tx.input(pure_arg(&input_commitment)?),
        tx.input(pure_arg(&prepay_amount)?),
        tx.input(pure_arg(&occurrence_budget)?),
        tx.input(pure_arg(&refund_mode)?),
        tx.input(pure_arg(&authorization_plan_commitment)?),
        schedule_policy,
        tx.input(pure_arg(&refill_policy_commitment)?),
        tx.input(pure_arg(&schedule_entries_commitment)?),
        tx.input(pure_arg(&first_after_ms)?),
    ];

    Ok(tap_interface_call(
        tx,
        objects,
        TapStandard::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT,
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
        _ => anyhow::bail!(
            "TAP payment mode {:?} is not yet supported by PTB builder",
            mode
        ),
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

pub fn trigger_scheduled_skill_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    scheduled_task: sui::types::Argument,
    execution_id: sui::types::Address,
) -> anyhow::Result<sui::types::Argument> {
    let execution_id = tx.input(pure_arg(&execution_id)?);
    Ok(tap_registry_call(
        tx,
        objects,
        TapStandard::TRIGGER_SCHEDULED_SKILL_EXECUTION,
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
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            false,
        ));

        worksheet(
            &mut tx,
            &objects,
            registry,
            agent,
            11,
            sui::types::Address::from_static("0xc"),
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.registry_pkg_id());
        assert_eq!(call.module, TapStandard::WORKSHEET.module);
        assert_eq!(call.function, TapStandard::WORKSHEET.name);
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn execute_and_schedule_use_peer_standard_tap_idents() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));
        let payment = tx.input(pure_arg(&2_u64).unwrap());

        execute_agent_skill(
            &mut tx,
            &objects,
            registry,
            agent,
            11,
            vec![1],
            payment,
            sui::types::Address::from_static("0xd"),
            Some(vec![2]),
        )
        .expect("execute builder succeeds");

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
            vec![1],
            sui::types::Address::from_static("0xc"),
            vec![3],
            None,
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
        let execute_call = calls
            .iter()
            .find(|call| {
                call.package == objects.registry_pkg_id()
                    && call.module == TapStandard::EXECUTE_AGENT_SKILL.module
                    && call.function == TapStandard::EXECUTE_AGENT_SKILL.name
            })
            .expect("execute_agent_skill call");
        let schedule_call = calls
            .iter()
            .find(|call| {
                call.package == objects.registry_pkg_id()
                    && call.module == TapStandard::SCHEDULE_SKILL_EXECUTION.module
                    && call.function == TapStandard::SCHEDULE_SKILL_EXECUTION.name
            })
            .expect("schedule_skill_execution call");
        assert_eq!(execute_call.function, TapStandard::EXECUTE_AGENT_SKILL.name);
        assert_eq!(
            schedule_call.function,
            TapStandard::SCHEDULE_SKILL_EXECUTION.name
        );
    }

    #[test]
    fn execute_and_schedule_prepare_tap_identity_handles_before_peer_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));
        let payment = tx.input(pure_arg(&2_u64).unwrap());

        execute_agent_skill(
            &mut tx,
            &objects,
            registry,
            agent,
            11,
            vec![1],
            payment,
            sui::types::Address::from_static("0xd"),
            Some(vec![2]),
        )
        .expect("execute builder succeeds");

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
            vec![1],
            sui::types::Address::from_static("0xc"),
            vec![3],
            None,
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

        let execute_idx = calls
            .iter()
            .position(|call| call.function == TapStandard::EXECUTE_AGENT_SKILL.name)
            .expect("execute_agent_skill call");
        let schedule_idx = calls
            .iter()
            .position(|call| call.function == TapStandard::SCHEDULE_SKILL_EXECUTION.name)
            .expect("schedule_skill_execution call");
        let first_skill_id_idx = calls
            .iter()
            .position(|call| call.function == TapStandard::SKILL_ID_FROM_U64.name)
            .expect("skill id conversion call");

        assert!(first_skill_id_idx < execute_idx);
        assert!(execute_idx < schedule_idx);
        assert!(
            calls[execute_idx].arguments.iter().any(|argument| {
                matches!(argument, sui::types::Argument::Result(index) if *index as usize == first_skill_id_idx)
            }),
            "execute call should use the converted skill id handle"
        );
    }

    #[test]
    fn standard_endpoint_builders_use_standard_tap_create_and_share_idents() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        let endpoint =
            create_standard_endpoint(&mut tx, &objects, sui::types::Address::from_static("0x44"))
                .expect("create endpoint builder succeeds");
        share_standard_endpoint(&mut tx, &objects, endpoint);

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let create_call = inspector.move_call(0);
        assert_eq!(create_call.package, objects.registry_pkg_id());
        assert_eq!(
            create_call.module,
            TapStandard::CREATE_STANDARD_ENDPOINT.module
        );
        assert_eq!(
            create_call.function,
            TapStandard::CREATE_STANDARD_ENDPOINT.name
        );
        assert_eq!(create_call.arguments.len(), 1);
        let sui::types::Input::Pure { value } = inspector.input(&create_call.arguments[0]) else {
            panic!("expected pure package id input");
        };
        let package_id: sui::types::Address =
            bcs::from_bytes(value).expect("package id BCS decodes");
        assert_eq!(package_id, sui::types::Address::from_static("0x44"));

        let share_call = inspector.move_call(1);
        assert_eq!(share_call.package, objects.registry_pkg_id());
        assert_eq!(
            share_call.module,
            TapStandard::SHARE_STANDARD_ENDPOINT.module
        );
        assert_eq!(
            share_call.function,
            TapStandard::SHARE_STANDARD_ENDPOINT.name
        );
        assert_eq!(share_call.arguments, vec![endpoint]);
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
            sui::types::Address::from_static("0xe"),
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
            sui::types::Address::from_static("0xf"),
            1,
            vec![5],
            vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0x10"),
            )],
            vec![6],
            true,
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
                        if call.package == objects.registry_pkg_id()
                            && call.module == TapStandard::REGISTER_SKILL.module
                            && call.function == TapStandard::REGISTER_SKILL.name
                )
            })
            .expect("register_skill call");
        let call = inspector.move_call(register_call_idx);
        assert_eq!(call.package, objects.registry_pkg_id());
        assert_eq!(call.module, TapStandard::REGISTER_SKILL.module);
        assert_eq!(call.function, TapStandard::REGISTER_SKILL.name);
        assert_eq!(call.arguments.len(), 16);
    }

    #[test]
    fn payment_and_vault_builders_target_standard_tap_functions() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_registry_arg(&mut tx, &objects).expect("registry");
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));
        let payment_coin = tx.input(pure_arg(&9_u64).unwrap());
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

        create_agent_skill_payment(
            &mut tx,
            &objects,
            registry,
            agent,
            payment_coin,
            sui::types::Address::from_static("0xe"),
            invoker_input,
        )
        .expect("create payment");
        deposit_agent_payment_vault(&mut tx, &objects, agent, vault_coin);
        withdraw_agent_payment_vault(&mut tx, &objects, registry, agent, 33).expect("withdraw");
        create_agent_skill_payment_from_vault(
            &mut tx,
            &objects,
            registry,
            agent,
            vault_input.skill_id,
            sui::types::Address::from_static("0xf"),
            vault_input.max_budget,
            vault_input.refund_mode,
        )
        .expect("create payment from vault");
        consume_gas_payment(
            &mut tx,
            &objects,
            agent,
            sui::types::Address::from_static("0xe"),
            sui::types::Address::from_static("0x30"),
            sui::types::Address::from_static("0x31"),
            44,
        )
        .expect("consume gas");
        accomplish_execution(
            &mut tx,
            &objects,
            agent,
            sui::types::Address::from_static("0xe"),
            sui::types::Address::from_static("0x30"),
        )
        .expect("accomplish");
        refund_execution(
            &mut tx,
            &objects,
            agent,
            sui::types::Address::from_static("0xe"),
            sui::types::Address::from_static("0x30"),
            vec![3, 4],
        )
        .expect("refund");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let function_names = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) if call.package == objects.interface_pkg_id => {
                    Some(call.function.clone())
                }
                sui::types::Command::MoveCall(call)
                    if call.package == objects.registry_pkg_id() =>
                {
                    Some(call.function.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for expected in [
            TapStandard::CREATE_AGENT_SKILL_PAYMENT.name,
            TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name,
            TapStandard::WITHDRAW_AGENT_PAYMENT_VAULT.name,
            TapStandard::CREATE_AGENT_SKILL_PAYMENT_FROM_VAULT.name,
            TapStandard::CONSUME_GAS_PAYMENT.name,
            TapStandard::ACCOMPLISH_EXECUTION.name,
            TapStandard::REFUND_EXECUTION.name,
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
        let registry = tap_registry_arg(&mut tx, &objects).expect("registry");
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
            sui::types::Address::from_static("0x60"),
            4,
            vec![1; 32],
            vec![TapSharedObjectRef::mutable(
                sui::types::Address::from_static("0x61"),
            )],
            TapPaymentPolicy::default(),
            TapSchedulePolicy::default(),
            vec![8],
            vec![9],
            true,
        )
        .expect("announce");
        set_active_endpoint_revision(
            &mut tx,
            &objects,
            registry,
            agent,
            11,
            InterfaceRevision(3),
            true,
        )
        .expect("set active");
        schedule_skill_execution_address_funded(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0x80"),
            11,
            vec![1],
            prepayment_coin,
            sui::types::Address::from_static("0x81"),
            vec![2],
            100,
            0,
            Some(vec![3]),
            TapSchedulePolicy::default(),
            vec![4],
            vec![5],
            200,
        )
        .expect("address funded schedule");
        schedule_skill_execution_from_agent_vault(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0x80"),
            11,
            vec![1],
            300,
            100,
            0,
            None,
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
            TapStandard::GET_SKILL_REQUIREMENTS.name,
            TapStandard::ANNOUNCE_ENDPOINT_REVISION.name,
            TapStandard::SET_ACTIVE_ENDPOINT_REVISION.name,
            TapStandard::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED.name,
            TapStandard::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT.name,
            TapStandard::TRIGGER_SCHEDULED_SKILL_EXECUTION.name,
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
    fn unsupported_payment_mode_is_rejected_before_register_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let agent = tx.input(sui::tx::Input::shared(
            sui::types::Address::from_static("0xa"),
            1,
            true,
        ));

        let error = register_skill(
            &mut tx,
            &objects,
            registry,
            agent,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
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
            sui::types::Address::from_static("0xf"),
            1,
            vec![5],
            vec![],
            vec![6],
            true,
        )
        .expect_err("unsupported payment mode");

        assert!(error
            .to_string()
            .contains("not yet supported by PTB builder"));
    }
}
