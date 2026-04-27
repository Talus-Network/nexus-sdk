//! This module is slightly different than others as it only defines the
//! generic interface of TAPs. The packages and modules are retrieved at
//! runtime.

use crate::{idents::ModuleAndNameIdent, sui};

pub const STANDARD_TAP_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("tap");

// == Nexus Interface V1 ==

pub struct TapV1;

impl TapV1 {
    /// Confirm walk eval with the TAP.
    pub const CONFIRM_TOOL_EVAL_FOR_WALK: sui::types::Identifier =
        sui::types::Identifier::from_static("confirm_tool_eval_for_walk");
    /// Get the TAP worksheet so that we can stamp it.
    pub const WORKSHEET: sui::types::Identifier = sui::types::Identifier::from_static("worksheet");
}

// == Standard TAP interface ==

pub struct TapStandard;

impl TapStandard {
    pub const ACCOMPLISH_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("accomplish_execution"),
    };
    pub const AGENT_CREATED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("AgentCreatedEvent"),
    };
    pub const ANNOUNCE_ENDPOINT_REVISION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("announce_endpoint_revision"),
    };
    pub const BIND_AUTHORIZATION_TO_LEADER_ASSIGNMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("bind_authorization_to_leader_assignment"),
    };
    pub const COMPLETE_SCHEDULED_SKILL_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("complete_scheduled_skill_execution"),
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
    pub const CREATE_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("create_vertex_authorization"),
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
    pub const EXPIRE_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("expire_vertex_authorization"),
    };
    pub const GET_SKILL_REQUIREMENTS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("get_skill_requirements"),
    };
    pub const REFUND_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("refund_execution"),
    };
    pub const REGISTER_SKILL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("register_skill"),
    };
    pub const REVOKE_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("revoke_vertex_authorization"),
    };
    pub const SCHEDULE_SKILL_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("schedule_skill_execution"),
    };
    pub const SET_ACTIVE_ENDPOINT_REVISION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("set_active_endpoint_revision"),
    };
    pub const SKILL_REGISTERED_EVENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("SkillRegisteredEvent"),
    };
    pub const TRIGGER_SCHEDULED_SKILL_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("trigger_scheduled_skill_execution"),
    };
    pub const VERIFY_VERTEX_AUTHORIZATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("verify_vertex_authorization"),
    };
    pub const WORKFLOW_WORKSHEET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("workflow_worksheet"),
    };
    pub const WORKSHEET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STANDARD_TAP_MODULE,
        name: sui::types::Identifier::from_static("worksheet"),
    };
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
    }
}
