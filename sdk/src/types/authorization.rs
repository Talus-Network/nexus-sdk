//! Types for `nexus_interface::authorization`.

use {
    super::{deserialize_vertex_string, InterfaceVersion, MoveOption},
    crate::sui,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenValue<T> {
    pub value: T,
    pub by: sui::types::Address,
    pub recipient: MoveOption<sui::types::Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneableProvenValue<T> {
    pub value: T,
    pub by: sui::types::Address,
    pub recipient: MoveOption<sui::types::Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant<T> {
    pub proof: CloneableProvenValue<T>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSkillAuthorization {
    pub id: sui::types::Address,
    pub agent_id: sui::types::Address,
    pub skill_id: u64,
    pub interface_version: InterfaceVersion,
    pub vertex_authorization_grants: Vec<crate::types::Grant<AgentVertexAuthorization>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentVertexAuthorization {
    pub skill_id: u64,
    pub interface_version: InterfaceVersion,
    pub dag_id: sui::types::Address,
    pub vertex: String,
    pub trigger: AuthorizationTrigger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorizationTrigger {
    Execution { execution_id: sui::types::Address },
    ScheduledTask { task_id: sui::types::Address },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentVertexAuthorizationTemplate {
    #[serde(deserialize_with = "super::deserialize_sui_u64")]
    pub skill_id: u64,
    #[serde(deserialize_with = "deserialize_vertex_string")]
    pub vertex: String,
    pub recipient_id: sui::types::Address,
}
