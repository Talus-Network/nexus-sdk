use crate::{
    idents::{pure_arg, tap::TapStandard},
    sui,
    types::{
        AgentId,
        InterfaceRevision,
        NexusObjects,
        SkillId,
        TapPaymentPolicy,
        TapSchedulePolicy,
        TapSharedObjectRef,
    },
};

fn tap_call(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    ident: crate::idents::ModuleAndNameIdent,
    args: Vec<sui::types::Argument>,
) -> sui::types::Argument {
    tx.move_call(
        sui::tx::Function::new(objects.interface_pkg_id, ident.module, ident.name, vec![]),
        args,
    )
}

pub fn create_agent(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    operator: sui::types::Address,
    metadata_hash: Vec<u8>,
    auth_mode: u8,
) -> anyhow::Result<sui::types::Argument> {
    let operator = tx.input(pure_arg(&operator)?);
    let metadata_hash = tx.input(pure_arg(&metadata_hash)?);
    let auth_mode = tx.input(pure_arg(&auth_mode)?);

    Ok(tap_call(
        tx,
        objects,
        TapStandard::CREATE_AGENT,
        vec![registry, operator, metadata_hash, auth_mode],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn register_skill(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag_id: sui::types::Address,
    tap_package_id: sui::types::Address,
    workflow_hash: Vec<u8>,
    requirements_hash: Vec<u8>,
    metadata_hash: Vec<u8>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_hash: Vec<u8>,
    endpoint_object_id: sui::types::Address,
    endpoint_object_version: u64,
    endpoint_object_digest: Vec<u8>,
    shared_objects: Vec<TapSharedObjectRef>,
    config_digest: Vec<u8>,
    active_for_new_executions: bool,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        agent,
        tx.input(pure_arg(&dag_id)?),
        tx.input(pure_arg(&tap_package_id)?),
        tx.input(pure_arg(&workflow_hash)?),
        tx.input(pure_arg(&requirements_hash)?),
        tx.input(pure_arg(&metadata_hash)?),
        tx.input(pure_arg(&payment_policy)?),
        tx.input(pure_arg(&schedule_policy)?),
        tx.input(pure_arg(&capability_schema_hash)?),
        tx.input(pure_arg(&endpoint_object_id)?),
        tx.input(pure_arg(&endpoint_object_version)?),
        tx.input(pure_arg(&endpoint_object_digest)?),
        tx.input(pure_arg(&shared_objects)?),
        tx.input(pure_arg(&config_digest)?),
        tx.input(pure_arg(&active_for_new_executions)?),
    ];

    Ok(tap_call(tx, objects, TapStandard::REGISTER_SKILL, args))
}

pub fn get_skill_requirements(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<sui::types::Argument> {
    let agent_id = tx.input(pure_arg(&agent_id)?);
    let skill_id = tx.input(pure_arg(&skill_id)?);

    Ok(tap_call(
        tx,
        objects,
        TapStandard::GET_SKILL_REQUIREMENTS,
        vec![registry, agent_id, skill_id],
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn announce_endpoint_revision(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent_id: AgentId,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
    endpoint_object_id: sui::types::Address,
    endpoint_object_version: u64,
    endpoint_object_digest: Vec<u8>,
    shared_objects: Vec<TapSharedObjectRef>,
    payment_policy: TapPaymentPolicy,
    schedule_policy: TapSchedulePolicy,
    capability_schema_hash: Vec<u8>,
    config_digest: Vec<u8>,
    active_for_new_executions: bool,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        tx.input(pure_arg(&agent_id)?),
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&interface_revision)?),
        tx.input(pure_arg(&endpoint_object_id)?),
        tx.input(pure_arg(&endpoint_object_version)?),
        tx.input(pure_arg(&endpoint_object_digest)?),
        tx.input(pure_arg(&shared_objects)?),
        tx.input(pure_arg(&payment_policy)?),
        tx.input(pure_arg(&schedule_policy)?),
        tx.input(pure_arg(&capability_schema_hash)?),
        tx.input(pure_arg(&config_digest)?),
        tx.input(pure_arg(&active_for_new_executions)?),
    ];

    Ok(tap_call(
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
    agent_id: AgentId,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
    active_for_new_executions: bool,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        tx.input(pure_arg(&agent_id)?),
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&interface_revision)?),
        tx.input(pure_arg(&active_for_new_executions)?),
    ];

    Ok(tap_call(
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
    agent_id: AgentId,
    skill_id: SkillId,
    execution: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        tx.input(pure_arg(&agent_id)?),
        tx.input(pure_arg(&skill_id)?),
        execution,
    ];

    Ok(tap_call(tx, objects, TapStandard::WORKSHEET, args))
}

#[derive(Clone, Debug)]
pub struct VertexAuthorizationInput {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub walk_execution_id: sui::types::Address,
    pub vertex_execution_id: sui::types::Address,
    pub target_object_id: sui::types::Address,
    pub allowed_tool_package: sui::types::Address,
    pub allowed_tool_module: Vec<u8>,
    pub allowed_tool_function: Vec<u8>,
    pub operation_hash: Vec<u8>,
    pub constraints_hash: Vec<u8>,
    pub expires_at_ms: u64,
    pub max_uses: u64,
    pub payment_required: bool,
}

pub fn create_vertex_authorization(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    input: VertexAuthorizationInput,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        tx.input(pure_arg(&input.agent_id)?),
        tx.input(pure_arg(&input.skill_id)?),
        tx.input(pure_arg(&input.walk_execution_id)?),
        tx.input(pure_arg(&input.vertex_execution_id)?),
        tx.input(pure_arg(&input.target_object_id)?),
        tx.input(pure_arg(&input.allowed_tool_package)?),
        tx.input(pure_arg(&input.allowed_tool_module)?),
        tx.input(pure_arg(&input.allowed_tool_function)?),
        tx.input(pure_arg(&input.operation_hash)?),
        tx.input(pure_arg(&input.constraints_hash)?),
        tx.input(pure_arg(&input.expires_at_ms)?),
        tx.input(pure_arg(&input.max_uses)?),
        tx.input(pure_arg(&input.payment_required)?),
    ];

    Ok(tap_call(
        tx,
        objects,
        TapStandard::CREATE_VERTEX_AUTHORIZATION,
        args,
    ))
}

pub fn bind_authorization_to_leader_assignment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    grant: sui::types::Argument,
    execution: sui::types::Argument,
    vertex_execution_id: sui::types::Address,
    leader_assignment_id: sui::types::Address,
    endpoint_revision: InterfaceRevision,
    payment_id: Option<sui::types::Address>,
    clock: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        grant,
        execution,
        tx.input(pure_arg(&vertex_execution_id)?),
        tx.input(pure_arg(&leader_assignment_id)?),
        tx.input(pure_arg(&endpoint_revision)?),
        tx.input(pure_arg(&payment_id)?),
        clock,
    ];

    Ok(tap_call(
        tx,
        objects,
        TapStandard::BIND_AUTHORIZATION_TO_LEADER_ASSIGNMENT,
        args,
    ))
}

pub fn verify_vertex_authorization(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    args: Vec<sui::types::Argument>,
) -> sui::types::Argument {
    tap_call(tx, objects, TapStandard::VERIFY_VERTEX_AUTHORIZATION, args)
}

pub fn consume_vertex_authorization(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    cap: sui::types::Argument,
) -> sui::types::Argument {
    tap_call(
        tx,
        objects,
        TapStandard::CONSUME_VERTEX_AUTHORIZATION,
        vec![cap],
    )
}

pub fn revoke_vertex_authorization(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    grant: sui::types::Argument,
) -> sui::types::Argument {
    tap_call(
        tx,
        objects,
        TapStandard::REVOKE_VERTEX_AUTHORIZATION,
        vec![grant],
    )
}

pub fn expire_vertex_authorization(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    grant: sui::types::Argument,
    clock: sui::types::Argument,
) -> sui::types::Argument {
    tap_call(
        tx,
        objects,
        TapStandard::EXPIRE_VERTEX_AUTHORIZATION,
        vec![grant, clock],
    )
}

#[derive(Clone, Debug)]
pub struct AgentSkillPaymentInput {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub source: Vec<u8>,
    pub auth: Vec<u8>,
    pub max_budget: u64,
    pub refund_mode: u8,
}

pub fn create_agent_skill_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    execution: sui::types::Argument,
    input: AgentSkillPaymentInput,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        tx.input(pure_arg(&input.agent_id)?),
        tx.input(pure_arg(&input.skill_id)?),
        execution,
        tx.input(pure_arg(&input.source)?),
        tx.input(pure_arg(&input.auth)?),
        tx.input(pure_arg(&input.max_budget)?),
        tx.input(pure_arg(&input.refund_mode)?),
    ];

    Ok(tap_call(
        tx,
        objects,
        TapStandard::CREATE_AGENT_SKILL_PAYMENT,
        args,
    ))
}

pub fn consume_gas_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    payment: sui::types::Argument,
    execution: sui::types::Argument,
    endpoint: sui::types::Argument,
    leader_cap: sui::types::Argument,
    amount: u64,
) -> anyhow::Result<sui::types::Argument> {
    let amount = tx.input(pure_arg(&amount)?);

    Ok(tap_call(
        tx,
        objects,
        TapStandard::CONSUME_GAS_PAYMENT,
        vec![payment, execution, endpoint, leader_cap, amount],
    ))
}

pub fn accomplish_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
    payment: sui::types::Argument,
    endpoint: sui::types::Argument,
    result_summary_hash: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let result_summary_hash = tx.input(pure_arg(&result_summary_hash)?);

    Ok(tap_call(
        tx,
        objects,
        TapStandard::ACCOMPLISH_EXECUTION,
        vec![execution, payment, endpoint, result_summary_hash],
    ))
}

pub fn refund_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    execution: sui::types::Argument,
    payment: sui::types::Argument,
    endpoint: sui::types::Argument,
    refund_reason: Vec<u8>,
) -> anyhow::Result<sui::types::Argument> {
    let refund_reason = tx.input(pure_arg(&refund_reason)?);

    Ok(tap_call(
        tx,
        objects,
        TapStandard::REFUND_EXECUTION,
        vec![execution, payment, endpoint, refund_reason],
    ))
}

pub fn execute_agent_skill(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    agent_id: AgentId,
    skill_id: SkillId,
    input_commitment: Vec<u8>,
    payment: sui::types::Argument,
    authorization_plan_hash: Option<Vec<u8>>,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        tx.input(pure_arg(&agent_id)?),
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&input_commitment)?),
        payment,
        tx.input(pure_arg(&authorization_plan_hash)?),
    ];

    Ok(tap_call(
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
    agent_id: AgentId,
    skill_id: SkillId,
    input_commitment: Vec<u8>,
    long_term_gas_coin_id: sui::types::Address,
    refill_policy_hash: Vec<u8>,
    authorization_plan_hash: Option<Vec<u8>>,
    schedule_policy: TapSchedulePolicy,
    schedule_entries_hash: Vec<u8>,
    first_after_ms: u64,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        registry,
        tx.input(pure_arg(&agent_id)?),
        tx.input(pure_arg(&skill_id)?),
        tx.input(pure_arg(&input_commitment)?),
        tx.input(pure_arg(&long_term_gas_coin_id)?),
        tx.input(pure_arg(&refill_policy_hash)?),
        tx.input(pure_arg(&authorization_plan_hash)?),
        tx.input(pure_arg(&schedule_policy)?),
        tx.input(pure_arg(&schedule_entries_hash)?),
        tx.input(pure_arg(&first_after_ms)?),
    ];

    Ok(tap_call(
        tx,
        objects,
        TapStandard::SCHEDULE_SKILL_EXECUTION,
        args,
    ))
}

pub fn trigger_scheduled_skill_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    registry: sui::types::Argument,
    scheduled_task: sui::types::Argument,
    scheduler_cap: sui::types::Argument,
    clock: sui::types::Argument,
) -> sui::types::Argument {
    tap_call(
        tx,
        objects,
        TapStandard::TRIGGER_SCHEDULED_SKILL_EXECUTION,
        vec![registry, scheduled_task, scheduler_cap, clock],
    )
}

pub fn complete_scheduled_skill_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    scheduled_task: sui::types::Argument,
    execution: sui::types::Argument,
    continue_recurring: bool,
    next_after_ms: u64,
    clock: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let args = vec![
        scheduled_task,
        execution,
        tx.input(pure_arg(&continue_recurring)?),
        tx.input(pure_arg(&next_after_ms)?),
        clock,
    ];

    Ok(tap_call(
        tx,
        objects,
        TapStandard::COMPLETE_SCHEDULED_SKILL_EXECUTION,
        args,
    ))
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
        let execution = tx.input(pure_arg(&2_u64).unwrap());

        worksheet(
            &mut tx,
            &objects,
            registry,
            AgentId(sui::types::Address::from_static("0xa")),
            SkillId(sui::types::Address::from_static("0xb")),
            execution,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, TapStandard::WORKSHEET.module);
        assert_eq!(call.function, TapStandard::WORKSHEET.name);
        assert_eq!(call.arguments.len(), 4);
    }

    #[test]
    fn execute_and_schedule_use_peer_standard_tap_idents() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let payment = tx.input(pure_arg(&2_u64).unwrap());

        execute_agent_skill(
            &mut tx,
            &objects,
            registry,
            AgentId(sui::types::Address::from_static("0xa")),
            SkillId(sui::types::Address::from_static("0xb")),
            vec![1],
            payment,
            Some(vec![2]),
        )
        .expect("execute builder succeeds");

        let registry = tx.input(pure_arg(&3_u64).unwrap());
        schedule_skill_execution(
            &mut tx,
            &objects,
            registry,
            AgentId(sui::types::Address::from_static("0xa")),
            SkillId(sui::types::Address::from_static("0xb")),
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
        assert_eq!(
            inspector.move_call(0).function,
            TapStandard::EXECUTE_AGENT_SKILL.name
        );
        assert_eq!(
            inspector.move_call(1).function,
            TapStandard::SCHEDULE_SKILL_EXECUTION.name
        );
    }

    #[test]
    fn register_skill_builder_carries_artifact_identity_and_config() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(pure_arg(&1_u64).unwrap());
        let agent = tx.input(pure_arg(&2_u64).unwrap());

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
                token_type_hash: Vec::new(),
                auth_mode: 0,
                refund_mode: 0,
            },
            TapSchedulePolicy::default(),
            vec![4],
            sui::types::Address::from_static("0xf"),
            1,
            vec![5],
            vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0x10"),
                2,
            )],
            vec![6],
            true,
        )
        .expect("register skill builder succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.interface_pkg_id);
        assert_eq!(call.module, TapStandard::REGISTER_SKILL.module);
        assert_eq!(call.function, TapStandard::REGISTER_SKILL.name);
        assert_eq!(call.arguments.len(), 16);
    }
}
