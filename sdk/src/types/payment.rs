//! Types for `nexus_interface::payment`.

use {
    super::{
        serde_parsers::{
            deserialize_tap_address_value,
            deserialize_tap_execution_payment_final_state_value,
            deserialize_tap_scheduled_occurrence_final_state_value,
            deserialize_tap_u64_value,
            deserialize_vertex_execution_payment_settlement_kind_value,
        },
        InterfaceVersion,
        SkillPaymentPolicy,
        SuiBalance,
    },
    crate::sui,
    serde::{de::Error as _, Deserialize, Deserializer, Serialize},
    serde_json::Value,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionPaymentSourceKind {
    UserFunded {
        #[serde(deserialize_with = "deserialize_tap_address_value")]
        user: sui::types::Address,
    },
    AgentFunded {
        #[serde(deserialize_with = "deserialize_tap_address_value")]
        agent_id: sui::types::Address,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPaymentFinalState {
    Pending,
    Accomplished,
    Refunded,
}

impl<'de> Deserialize<'de> for ExecutionPaymentFinalState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum RawState {
                Pending,
                Accomplished,
                Refunded,
            }

            return RawState::deserialize(deserializer).map(|state| match state {
                RawState::Pending => Self::Pending,
                RawState::Accomplished => Self::Accomplished,
                RawState::Refunded => Self::Refunded,
            });
        }

        let value = Value::deserialize(deserializer)?;
        deserialize_tap_execution_payment_final_state_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP execution payment final state value"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VertexExecutionPaymentSettlementKind {
    Free,
    Ticket,
    Paid,
}

impl<'de> Deserialize<'de> for VertexExecutionPaymentSettlementKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return u8::deserialize(deserializer).map(|value| match value {
                0 => Self::Free,
                1 => Self::Ticket,
                2 => Self::Paid,
                _ => Self::Paid,
            });
        }

        let value = Value::deserialize(deserializer)?;
        deserialize_vertex_execution_payment_settlement_kind_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment settlement kind value"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledOccurrenceFinalState {
    InFlight,
    Accomplished,
    Refunded,
}

impl<'de> Deserialize<'de> for ScheduledOccurrenceFinalState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum RawState {
                InFlight,
                Accomplished,
                Refunded,
            }

            return RawState::deserialize(deserializer).map(|state| match state {
                RawState::InFlight => Self::InFlight,
                RawState::Accomplished => Self::Accomplished,
                RawState::Refunded => Self::Refunded,
            });
        }

        let value = Value::deserialize(deserializer)?;
        deserialize_tap_scheduled_occurrence_final_state_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP scheduled occurrence final state value"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPayment {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub execution_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub agent_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: u64,
    pub interface_revision: InterfaceVersion,
    #[serde(deserialize_with = "deserialize_skill_payment_policy")]
    pub payment_policy: SkillPaymentPolicy,
    pub source_kind: ExecutionPaymentSourceKind,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub max_budget: u64,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub locked_budget: u64,
    pub funds: SuiBalance,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub consumed: u64,
    pub accomplished: bool,
    pub refunded: bool,
    pub final_state: ExecutionPaymentFinalState,
    pub tool_cost_snapshot: PaymentVecMap<Vec<u8>, u64>,
    #[serde(default)]
    pub locked_vertices: Vec<ExecutionPaymentVertexLock>,
}

impl ExecutionPayment {
    pub fn payment_id(&self) -> sui::types::Address {
        self.id
    }

    pub fn skill_revision_key(&self) -> crate::types::SkillRevisionKey {
        crate::types::SkillRevisionKey {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }

    pub fn locks(&self) -> u64 {
        self.locked_vertices.len() as u64
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentVecMap<K, V> {
    pub contents: Vec<PaymentVecMapEntry<K, V>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentVecMapEntry<K, V> {
    pub key: K,
    pub value: V,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPaymentVertexLock {
    #[serde(deserialize_with = "super::serde_parsers::deserialize_tap_byte_vector")]
    pub vertex_key: Vec<u8>,
    #[serde(deserialize_with = "super::serde_parsers::deserialize_tap_byte_vector")]
    pub tool_fqn: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub amount: u64,
    pub settlement_kind: VertexExecutionPaymentSettlementKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPaymentReceipt {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub execution_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub payment_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub agent_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: u64,
    pub source_kind: ExecutionPaymentSourceKind,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub max_budget: u64,
    pub resolved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledPaymentReserveReceipt {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub scheduled_task_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub reserve_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub agent_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: u64,
    pub interface_version: InterfaceVersion,
    pub source_kind: ExecutionPaymentSourceKind,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub prepaid_amount: u64,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub occurrence_budget: u64,
    pub resolved: bool,
    pub canceled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPaymentHistoryList {
    #[serde(deserialize_with = "super::serde_parsers::deserialize_tap_address_value")]
    pub id: sui::types::Address,
    pub execution_ids: Vec<sui::types::Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledOccurrenceRecord {
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub occurrence_index: u64,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub execution_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub payment_id: sui::types::Address,
    pub interface_revision: InterfaceVersion,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub budget: u64,
    pub final_state: ScheduledOccurrenceFinalState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledPaymentReserve {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub scheduled_task_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub agent_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: u64,
    pub interface_version: InterfaceVersion,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub agent_skill_authorization_id: sui::types::Address,
    pub payment_source: ExecutionPaymentSourceKind,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub occurrence_budget: u64,
    pub remaining_funds: SuiBalance,
    #[serde(deserialize_with = "deserialize_skill_payment_policy")]
    pub payment_policy: SkillPaymentPolicy,
    pub in_flight: Vec<ScheduledOccurrenceRecord>,
    pub payment_receipts: Vec<ExecutionPaymentReceipt>,
}

fn deserialize_skill_payment_policy<'de, D>(deserializer: D) -> Result<SkillPaymentPolicy, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return SkillPaymentPolicy::deserialize(deserializer);
    }

    let value = Value::deserialize(deserializer)?;
    parse_skill_payment_policy_value(&value)
        .ok_or_else(|| D::Error::custom("missing TAP skill payment policy value"))
}

fn parse_skill_payment_policy_value(value: &Value) -> Option<SkillPaymentPolicy> {
    fn policy_from_text(text: &str, value: &Value) -> Option<SkillPaymentPolicy> {
        match text {
            "user_funded" | "UserFunded" | "userFunded" => Some(SkillPaymentPolicy::UserFunded),
            "agent_funded" | "AgentFunded" | "agentFunded" => {
                Some(SkillPaymentPolicy::AgentFunded {
                    max_budget: extract_max_budget(value).unwrap_or(0),
                })
            }
            _ => None,
        }
    }

    match value {
        Value::String(text) => policy_from_text(text, value),
        Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(Value::String(text)) = object.get(key) {
                    if let Some(policy) = policy_from_text(text, value) {
                        return Some(policy);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(policy) = parse_skill_payment_policy_value(fields) {
                    return Some(policy);
                }
            }

            object
                .iter()
                .find_map(|(key, nested)| policy_from_text(key, nested))
        }
        _ => None,
    }
}

fn extract_max_budget(value: &Value) -> Option<u64> {
    match value {
        Value::Object(object) => {
            if let Some(budget) = object.get("max_budget").and_then(parse_u64_value) {
                return Some(budget);
            }

            object
                .get("fields")
                .and_then(extract_max_budget)
                .or_else(|| object.values().find_map(extract_max_budget))
        }
        _ => None,
    }
}

fn parse_u64_value(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_execution_payment_deserializes_move_json_byte_vectors() {
        let payment: ExecutionPayment = serde_json::from_value(serde_json::json!({
            "id": "0xa1",
            "execution_id": "0xa2",
            "agent_id": "0xa",
            "skill_id": "11",
            "interface_revision": "7",
            "payment_policy": "UserFunded",
            "source_kind": { "UserFunded": { "user": "0x1" } },
            "max_budget": "100",
            "locked_budget": "25",
            "funds": { "value": "1000000" },
            "consumed": "15",
            "accomplished": false,
            "refunded": false,
            "final_state": "Pending",
            "tool_cost_snapshot": {
                "contents": [
                    {
                        "key": [120, 121, 122, 46, 111, 110, 99, 104],
                        "value": 5
                    }
                ]
            },
            "locked_vertices": [
                {
                    "vertex_key": [224, 167, 144],
                    "tool_fqn": [120, 121, 122, 46, 112, 97, 121],
                    "amount": "11",
                    "settlement_kind": "Paid"
                }
            ]
        }))
        .expect("move json payment with byte vectors");

        assert_eq!(
            payment.tool_cost_snapshot.contents[0].key,
            vec![120, 121, 122, 46, 111, 110, 99, 104]
        );
        assert_eq!(payment.locked_vertices[0].vertex_key, vec![224, 167, 144]);
        assert_eq!(
            payment.locked_vertices[0].tool_fqn,
            vec![120, 121, 122, 46, 112, 97, 121]
        );
    }

    #[test]
    fn tap_execution_payment_deserializes_move_json_payment_mode() {
        let user_funded: ExecutionPayment = serde_json::from_value(serde_json::json!({
            "id": "0xa1",
            "execution_id": "0xa2",
            "agent_id": "0xa",
            "skill_id": "11",
            "interface_revision": "7",
            "payment_policy": { "type": "UserFunded" },
            "source_kind": {
                "UserFunded": { "user": "0x1" }
            },
            "max_budget": "0",
            "locked_budget": "0",
            "funds": { "value": "1000000" },
            "consumed": "0",
            "accomplished": false,
            "refunded": false,
            "final_state": "Pending",
            "tool_cost_snapshot": { "contents": [] },
            "locked_vertices": []
        }))
        .expect("move json user-funded payment");
        assert_eq!(user_funded.payment_policy, SkillPaymentPolicy::UserFunded);

        let agent_funded: ExecutionPayment = serde_json::from_value(serde_json::json!({
            "id": "0xa3",
            "execution_id": "0xa4",
            "agent_id": "0xa",
            "skill_id": "11",
            "interface_revision": "7",
            "payment_policy": {
                "fields": { "max_budget": "300" },
                "type": "AgentFunded"
            },
            "source_kind": {
                "AgentFunded": {
                    "agent_id": "0xa"
                }
            },
            "max_budget": "300",
            "locked_budget": "0",
            "funds": { "value": "1000000" },
            "consumed": "0",
            "accomplished": false,
            "refunded": false,
            "final_state": "Pending",
            "tool_cost_snapshot": { "contents": [] },
            "locked_vertices": []
        }))
        .expect("move json agent-funded payment");

        assert_eq!(
            agent_funded.payment_policy,
            SkillPaymentPolicy::AgentFunded { max_budget: 300 }
        );
    }

    #[test]
    fn payment_enums_deserialize_move_json_forms() {
        assert_eq!(
            serde_json::from_value::<VertexExecutionPaymentSettlementKind>(serde_json::json!({
                "Paid": {}
            }))
            .expect("keyed settlement kind"),
            VertexExecutionPaymentSettlementKind::Paid,
        );
        assert_eq!(
            serde_json::from_value::<ExecutionPaymentFinalState>(serde_json::json!({
                "fields": { "variant": "Accomplished" }
            }))
            .expect("nested payment final state"),
            ExecutionPaymentFinalState::Accomplished,
        );
        assert_eq!(
            serde_json::from_value::<ScheduledOccurrenceFinalState>(serde_json::json!({
                "fields": { "@variant": "inFlight" }
            }))
            .expect("nested scheduled occurrence state"),
            ScheduledOccurrenceFinalState::InFlight,
        );
    }
}
