//! This module is slightly different than others as it only defines the
//! generic interface of TAPs. The packages and modules are retrieved at
//! runtime.

use crate::{idents::ModuleAndNameIdent, sui};

pub const STANDARD_TAP_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("tap");

// == Standard TAP interface ==

pub struct TapStandard;

impl TapStandard {
    pub const ACCOMPLISH_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("accomplish_execution"),
    };
    pub const ACCOMPLISH_EXECUTION_FROM_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("accomplish_execution_from_vault"),
    };
    pub const AGENT_CREATED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("AgentCreatedEvent"),
    };
    pub const AGENT_ID_FROM_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("agent_id_from_address"),
    };
    pub const ANNOUNCE_ENDPOINT_REVISION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("announce_endpoint_revision"),
    };
    pub const BIND_AUTHORIZATION_TO_LEADER_ASSIGNMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("bind_authorization_to_leader_assignment"),
    };
    pub const BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("bootstrap_default_runtime_dag_skill"),
    };
    pub const BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_TAP_MODULE,
            name: sui::types::Identifier::from_static(
                "bootstrap_default_runtime_dag_skill_for_deployment",
            ),
        };
    pub const BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT_WITH_PACKAGE: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_TAP_MODULE,
            name: sui::types::Identifier::from_static(
                "bootstrap_default_runtime_dag_skill_for_deployment_with_package",
            ),
        };
    pub const CANCEL_SCHEDULED_SKILL_EXECUTION_ADDRESS_FUNDED: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_TAP_MODULE,
            name: sui::types::Identifier::from_static(
                "cancel_scheduled_skill_execution_address_funded",
            ),
        };
    pub const CANCEL_SCHEDULED_SKILL_EXECUTION_FROM_AGENT_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_TAP_MODULE,
            name: sui::types::Identifier::from_static(
                "cancel_scheduled_skill_execution_from_agent_vault",
            ),
        };
    pub const COMPLETE_SCHEDULED_SKILL_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("complete_scheduled_skill_execution"),
    };
    pub const COMPLETE_SCHEDULED_SKILL_OCCURRENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("complete_scheduled_skill_occurrence"),
    };
    pub const CONFIRM_TOOL_EVAL_FOR_WALK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("confirm_tool_eval_for_walk"),
    };
    pub const CONSUME_GAS_PAYMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("consume_gas_payment"),
    };
    pub const CONSUME_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("consume_vertex_authorization"),
    };
    pub const CREATE_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("create_agent"),
    };
    pub const CREATE_AGENT_SKILL_PAYMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("create_agent_skill_payment"),
    };
    pub const CREATE_AGENT_SKILL_PAYMENT_FROM_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("create_agent_skill_payment_from_vault"),
    };
    pub const CREATE_SCHEDULED_OCCURRENCE_PAYMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("create_scheduled_occurrence_payment"),
    };
    pub const CREATE_STANDARD_ENDPOINT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("create_standard_endpoint"),
    };
    pub const CREATE_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("create_vertex_authorization"),
    };
    pub const DEFAULT_EXECUTION_TARGET_UPDATED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("DefaultExecutionTargetUpdatedEvent"),
    };
    /// Deposit SUI into a standard TAP `AgentPaymentVault`.
    pub const DEPOSIT_AGENT_PAYMENT_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("deposit_agent_payment_vault"),
    };
    pub const ENDPOINT_REVISION_ACTIVATED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("EndpointRevisionActivatedEvent"),
    };
    pub const ENDPOINT_REVISION_ANNOUNCED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("EndpointRevisionAnnouncedEvent"),
    };
    pub const EXECUTE_AGENT_SKILL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("execute_agent_skill"),
    };
    pub const EXECUTION_PAYMENT_RECEIPT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
    };
    pub const EXPIRE_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("expire_vertex_authorization"),
    };
    pub const GET_SKILL_REQUIREMENTS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("get_skill_requirements"),
    };
    pub const INTERFACE_REVISION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("interface_revision"),
    };
    pub const PAYMENT_MODE_USER_FUNDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("payment_mode_user_funded"),
    };
    pub const PAYMENT_POLICY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("payment_policy"),
    };
    pub const REFILL_SCHEDULED_SKILL_EXECUTION_ADDRESS_FUNDED: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_TAP_MODULE,
            name: sui::types::Identifier::from_static(
                "refill_scheduled_skill_execution_address_funded",
            ),
        };
    pub const REFILL_SCHEDULED_SKILL_EXECUTION_FROM_AGENT_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_TAP_MODULE,
            name: sui::types::Identifier::from_static(
                "refill_scheduled_skill_execution_from_agent_vault",
            ),
        };
    pub const REFUND_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("refund_execution"),
    };
    pub const REFUND_EXECUTION_FROM_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("refund_execution_from_vault"),
    };
    pub const REFUND_EXECUTION_PAYMENT_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("refund_execution_payment_vertex"),
    };
    pub const REGISTER_SKILL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("register_skill"),
    };
    pub const REVOKE_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("revoke_vertex_authorization"),
    };
    pub const SCHEDULED_OCCURRENCE_FINAL_STATE_ACCOMPLISHED: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_TAP_MODULE,
            name: sui::types::Identifier::from_static(
                "scheduled_occurrence_final_state_accomplished",
            ),
        };
    pub const SCHEDULED_OCCURRENCE_FINAL_STATE_IN_FLIGHT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("scheduled_occurrence_final_state_in_flight"),
    };
    pub const SCHEDULED_OCCURRENCE_FINAL_STATE_REFUNDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("scheduled_occurrence_final_state_refunded"),
    };
    pub const SCHEDULE_POLICY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("schedule_policy"),
    };
    pub const SCHEDULE_SKILL_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("schedule_skill_execution"),
    };
    pub const SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("schedule_skill_execution_address_funded"),
    };
    pub const SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("schedule_skill_execution_from_agent_vault"),
    };
    pub const SETTLE_EXECUTION_PAYMENT_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("settle_execution_payment_vertex"),
    };
    pub const SET_ACTIVE_ENDPOINT_REVISION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("set_active_endpoint_revision"),
    };
    pub const SHARED_OBJECT_REF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("shared_object_ref"),
    };
    pub const SHARE_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("share_agent"),
    };
    pub const SHARE_STANDARD_ENDPOINT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("share_standard_endpoint"),
    };
    pub const SHARE_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("share_vertex_authorization"),
    };
    pub const SKILL_ID_FROM_U64: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("skill_id_from_u64"),
    };
    pub const SKILL_REGISTERED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("SkillRegisteredEvent"),
    };
    pub const SNAPSHOT_EXECUTION_PAYMENT_TOOL_COST: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("snapshot_execution_payment_tool_cost"),
    };
    pub const TRIGGER_SCHEDULED_SKILL_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("trigger_scheduled_skill_execution"),
    };
    pub const VERIFY_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("verify_vertex_authorization"),
    };
    /// Withdraw unlocked SUI from a standard TAP `AgentPaymentVault`.
    pub const WITHDRAW_AGENT_PAYMENT_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("withdraw_agent_payment_vault"),
    };
    pub const WORKFLOW_WORKSHEET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("workflow_worksheet"),
    };
    pub const WORKFLOW_WORKSHEET_FOR_IDS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("workflow_worksheet_for_ids"),
    };
    pub const WORKSHEET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("worksheet"),
    };
}

pub fn interface_revision_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_TAP_MODULE,
        sui::types::Identifier::from_static("InterfaceRevision"),
        vec![],
    )))
}

pub fn scheduled_skill_task_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_TAP_MODULE,
        sui::types::Identifier::from_static("ScheduledSkillTask"),
        vec![],
    )))
}

pub fn standard_endpoint_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_TAP_MODULE,
        sui::types::Identifier::from_static("StandardEndpoint"),
        vec![],
    )))
}

pub fn agent_payment_vault_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_TAP_MODULE,
        sui::types::Identifier::from_static("AgentPaymentVault"),
        vec![],
    )))
}

pub fn execution_payment_receipt_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_TAP_MODULE,
        sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
        vec![],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_tap_idents_use_tap_module() {
        assert_eq!(TapStandard::CREATE_AGENT.module, STANDARD_TAP_MODULE);
        assert_eq!(
            TapStandard::REGISTER_SKILL.name,
            sui::types::Identifier::from_static("register_skill")
        );
        assert_eq!(
            TapStandard::SCHEDULE_SKILL_EXECUTION.name,
            sui::types::Identifier::from_static("schedule_skill_execution")
        );
        assert_eq!(
            TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name,
            sui::types::Identifier::from_static("deposit_agent_payment_vault")
        );
        assert_eq!(
            agent_payment_vault_type(sui::types::Address::from_static("0x1")),
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                sui::types::Address::from_static("0x1"),
                STANDARD_TAP_MODULE,
                sui::types::Identifier::from_static("AgentPaymentVault"),
                vec![],
            )))
        );
    }
}
