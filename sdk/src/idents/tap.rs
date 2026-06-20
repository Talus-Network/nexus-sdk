//! This module is slightly different than others as it only defines the
//! generic interface of Nexus agents. The packages and modules are retrieved at
//! runtime.

use crate::{
    idents::{registry::AGENT_REGISTRY_MODULE, ModuleAndNameIdent},
    sui,
};

pub const STANDARD_AGENT_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("agent");
pub const STANDARD_AUTHORIZATION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("authorization");
pub const STANDARD_PAYMENT_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("payment");
pub const INTERFACE_VERSION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("version");

// == Standard agent interface ==

pub struct TapStandard;

impl TapStandard {
    pub const AGENT_CREATED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("AgentCreatedEvent"),
    };
    pub const AGENT_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("AgentExecutionConfig"),
    };
    pub const AGENT_ID_FROM_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("agent_id_from_address"),
    };
    pub const AGENT_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AUTHORIZATION_MODULE,
        name: sui::types::Identifier::from_static("AgentVertexAuthorization"),
    };
    pub const AGENT_VERTEX_AUTHORIZATION_TEMPLATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AUTHORIZATION_MODULE,
        name: sui::types::Identifier::from_static("agent_vertex_authorization_template"),
    };
    pub const BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_AGENT_MODULE,
            name: sui::types::Identifier::from_static(
                "bootstrap_default_runtime_dag_skill_for_deployment",
            ),
        };
    pub const COMPLETE_SCHEDULED_PAYMENT_RESERVE_OCCURRENCE: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_PAYMENT_MODULE,
            name: sui::types::Identifier::from_static(
                "complete_scheduled_payment_reserve_occurrence",
            ),
        };
    pub const CREATE_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("create_agent"),
    };
    pub const DEFAULT_DAG_EXECUTOR_UPDATED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("DefaultDagExecutorUpdatedEvent"),
    };
    /// Deposit SUI into a standard TAP `AgentPaymentVault`.
    pub const DEPOSIT_AGENT_PAYMENT_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("deposit_agent_payment_vault"),
    };
    pub const EXECUTION_PAYMENT_RECEIPT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_PAYMENT_MODULE,
        name: sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
    };
    pub const FIXED_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("fixed_tool"),
    };
    pub const GET_SKILL_REQUIREMENTS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("get_skill_requirements"),
    };
    pub const INTERFACE_REVISION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: INTERFACE_VERSION_MODULE,
        name: sui::types::Identifier::from_static("interface_revision"),
    };
    pub const NEW_AGENT_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("new_agent_execution_config"),
    };
    pub const NEW_DEFAULT_AGENT_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("new_default_agent_execution_config"),
    };
    pub const PAYMENT_POLICY_AGENT_FUNDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_PAYMENT_MODULE,
        name: sui::types::Identifier::from_static("payment_policy_agent_funded"),
    };
    pub const PAYMENT_POLICY_USER_FUNDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_PAYMENT_MODULE,
        name: sui::types::Identifier::from_static("payment_policy_user_funded"),
    };
    pub const RECURRENCE_ONCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("recurrence_once"),
    };
    pub const RECURRENCE_RECURSIVE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("recurrence_recursive"),
    };
    pub const REGISTER_SKILL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("register_skill"),
    };
    pub const SCHEDULED_OCCURRENCE_FINAL_STATE_ACCOMPLISHED: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: STANDARD_PAYMENT_MODULE,
            name: sui::types::Identifier::from_static(
                "scheduled_occurrence_final_state_accomplished",
            ),
        };
    pub const SCHEDULED_OCCURRENCE_FINAL_STATE_IN_FLIGHT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_PAYMENT_MODULE,
        name: sui::types::Identifier::from_static("scheduled_occurrence_final_state_in_flight"),
    };
    pub const SCHEDULED_OCCURRENCE_FINAL_STATE_REFUNDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_PAYMENT_MODULE,
        name: sui::types::Identifier::from_static("scheduled_occurrence_final_state_refunded"),
    };
    pub const SCHEDULE_POLICY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("schedule_policy"),
    };
    pub const SETTLE_EXECUTION_PAYMENT_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("settle_execution_payment_vertex"),
    };
    pub const SHARED_OBJECT_REF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("shared_object_ref"),
    };
    pub const SKILL_ACTIVE_REVISION_UPDATED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("SkillActiveRevisionUpdatedEvent"),
    };
    pub const SKILL_REGISTERED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("SkillRegisteredEvent"),
    };
    pub const SNAPSHOT_EXECUTION_PAYMENT_TOOL_COST: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("snapshot_execution_payment_tool_cost"),
    };
    /// Withdraw unlocked SUI from a standard TAP `AgentPaymentVault`.
    pub const WITHDRAW_AGENT_PAYMENT_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_AGENT_MODULE,
        name: sui::types::Identifier::from_static("withdraw_agent_payment_vault"),
    };
}

pub fn interface_version_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        INTERFACE_VERSION_MODULE,
        sui::types::Identifier::from_static("InterfaceVersion"),
        vec![],
    )))
}

pub fn interface_revision_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    interface_version_type(package_id)
}

pub fn scheduled_vertex_authorization_template_type(
    package_id: sui::types::Address,
) -> sui::types::TypeTag {
    agent_vertex_authorization_template_type(package_id)
}

pub fn agent_vertex_authorization_template_type(
    package_id: sui::types::Address,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_AUTHORIZATION_MODULE,
        sui::types::Identifier::from_static("AgentVertexAuthorizationTemplate"),
        vec![],
    )))
}

pub fn agent_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_AGENT_MODULE,
        sui::types::Identifier::from_static("Agent"),
        vec![],
    )))
}

pub fn agent_payment_vault_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_AGENT_MODULE,
        sui::types::Identifier::from_static("AgentPaymentVault"),
        vec![],
    )))
}

pub fn execution_payment_receipt_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_PAYMENT_MODULE,
        sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
        vec![],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idents_use_agent_module() {
        assert_eq!(TapStandard::CREATE_AGENT.module, STANDARD_AGENT_MODULE);
        assert_eq!(
            TapStandard::REGISTER_SKILL.name,
            sui::types::Identifier::from_static("register_skill")
        );
        assert_eq!(
            TapStandard::AGENT_EXECUTION_CONFIG.name,
            sui::types::Identifier::from_static("AgentExecutionConfig")
        );
        assert_eq!(
            TapStandard::AGENT_VERTEX_AUTHORIZATION_TEMPLATE.name,
            sui::types::Identifier::from_static("agent_vertex_authorization_template")
        );
        assert_eq!(
            TapStandard::AGENT_VERTEX_AUTHORIZATION.name,
            sui::types::Identifier::from_static("AgentVertexAuthorization")
        );
        assert_eq!(
            TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name,
            sui::types::Identifier::from_static("deposit_agent_payment_vault")
        );
        assert_eq!(
            agent_payment_vault_type(sui::types::Address::from_static("0x1")),
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                sui::types::Address::from_static("0x1"),
                STANDARD_AGENT_MODULE,
                sui::types::Identifier::from_static("AgentPaymentVault"),
                vec![],
            )))
        );
    }

    #[test]
    fn type_tags_use_supplied_package() {
        let package = sui::types::Address::from_static("0x42");

        for (tag, expected_module, expected_name) in [
            (
                interface_revision_type(package),
                INTERFACE_VERSION_MODULE,
                sui::types::Identifier::from_static("InterfaceVersion"),
            ),
            (
                scheduled_vertex_authorization_template_type(package),
                STANDARD_AUTHORIZATION_MODULE,
                sui::types::Identifier::from_static("AgentVertexAuthorizationTemplate"),
            ),
            (
                execution_payment_receipt_type(package),
                STANDARD_PAYMENT_MODULE,
                sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
            ),
        ] {
            let sui::types::TypeTag::Struct(tag) = tag else {
                panic!("expected struct type tag");
            };
            assert_eq!(*tag.address(), package);
            assert_eq!(*tag.module(), expected_module);
            assert_eq!(*tag.name(), expected_name);
            assert!(tag.type_params().is_empty());
        }
    }
}
