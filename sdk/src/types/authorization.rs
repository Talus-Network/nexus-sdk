//! Types for `nexus_interface::authorization`.

use {
    super::{InterfaceVersion, MoveOption},
    crate::sui,
    serde::{de::Error as _, Deserialize, Serialize},
};

fn deserialize_vertex_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        return String::from_utf8(bytes).map_err(D::Error::custom);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(bytes) = super::parse_byte_vector_value(&value).map_err(D::Error::custom)? {
        return String::from_utf8(bytes).map_err(D::Error::custom);
    }

    let text = super::parse_string_value(&value)
        .map_err(D::Error::custom)?
        .ok_or_else(|| D::Error::custom("missing authorization vertex value"))?;

    if let Some(hex) = text.strip_prefix("0x") {
        if hex.len() % 2 == 0 && hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            if let Ok(bytes) = hex::decode(hex) {
                if let Ok(decoded) = String::from_utf8(bytes) {
                    return Ok(decoded);
                }
            }
        }
    }

    Ok(text)
}

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
