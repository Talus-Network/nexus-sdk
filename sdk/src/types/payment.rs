//! Companion helpers for generated `nexus_interface::payment` types.

use {
    super::interface::payment::{
        ExecutionPayment,
        ExecutionPaymentFinalState,
        ExecutionPaymentHistoryList,
        ExecutionPaymentReceipt,
        ScheduledOccurrenceFinalState,
        ScheduledPaymentReserve,
        ScheduledPaymentReserveReceipt,
        SkillPaymentPolicy,
        VertexExecutionPaymentSettlementKind,
    },
    serde::{de::Error as _, Deserialize, Deserializer},
    serde_json::Value,
};

impl<'de> Deserialize<'de> for ExecutionPaymentFinalState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum PaymentFinalStateBcs {
                Pending,
                Accomplished,
                Refunded,
            }

            return PaymentFinalStateBcs::deserialize(deserializer).map(|state| match state {
                PaymentFinalStateBcs::Pending => Self::Pending,
                PaymentFinalStateBcs::Accomplished => Self::Accomplished,
                PaymentFinalStateBcs::Refunded => Self::Refunded,
            });
        }

        let value = Value::deserialize(deserializer)?;
        super::serde_parsers::deserialize_tap_execution_payment_final_state_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP execution payment final state value"))
    }
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
        super::serde_parsers::deserialize_vertex_execution_payment_settlement_kind_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment settlement kind value"))
    }
}

impl<'de> Deserialize<'de> for ScheduledOccurrenceFinalState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum ScheduledOccurrenceFinalStateBcs {
                InFlight,
                Accomplished,
                Refunded,
            }

            return ScheduledOccurrenceFinalStateBcs::deserialize(deserializer).map(|state| {
                match state {
                    ScheduledOccurrenceFinalStateBcs::InFlight => Self::InFlight,
                    ScheduledOccurrenceFinalStateBcs::Accomplished => Self::Accomplished,
                    ScheduledOccurrenceFinalStateBcs::Refunded => Self::Refunded,
                }
            });
        }

        let value = Value::deserialize(deserializer)?;
        super::serde_parsers::deserialize_tap_scheduled_occurrence_final_state_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP scheduled occurrence final state value"))
    }
}

impl ExecutionPayment {
    pub fn payment_id(&self) -> crate::sui::types::Address {
        self.id.id.bytes
    }

    pub fn skill_revision_key(&self) -> crate::types::SkillRevisionLookupKey {
        crate::types::SkillRevisionLookupKey {
            agent_id: self.agent_id.bytes,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }

    pub fn locks(&self) -> u64 {
        self.locked_vertices.len() as u64
    }
}

impl Clone for ExecutionPayment {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            execution_id: self.execution_id,
            agent_id: self.agent_id.clone(),
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
            payment_policy: self.payment_policy.clone(),
            source_kind: self.source_kind.clone(),
            max_budget: self.max_budget,
            locked_budget: self.locked_budget,
            funds: self.funds.clone(),
            consumed: self.consumed,
            accomplished: self.accomplished,
            refunded: self.refunded,
            final_state: self.final_state.clone(),
            tool_cost_snapshot: self.tool_cost_snapshot.clone(),
            locked_vertices: self.locked_vertices.clone(),
        }
    }
}

impl Clone for ExecutionPaymentReceipt {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            execution_id: self.execution_id,
            payment_id: self.payment_id,
            agent_id: self.agent_id.clone(),
            skill_id: self.skill_id,
            source_kind: self.source_kind.clone(),
            max_budget: self.max_budget,
            resolved: self.resolved,
        }
    }
}

impl Clone for ExecutionPaymentHistoryList {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            execution_ids: self.execution_ids.clone(),
        }
    }
}

impl Clone for ScheduledPaymentReserve {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            scheduled_task_id: self.scheduled_task_id,
            agent_id: self.agent_id.clone(),
            skill_id: self.skill_id,
            interface_version: self.interface_version,
            agent_skill_authorization_id: self.agent_skill_authorization_id.clone(),
            payment_source: self.payment_source.clone(),
            occurrence_budget: self.occurrence_budget,
            remaining_funds: self.remaining_funds.clone(),
            payment_policy: self.payment_policy.clone(),
            in_flight: self.in_flight.clone(),
            payment_receipts: self.payment_receipts.clone(),
        }
    }
}

impl Clone for ScheduledPaymentReserveReceipt {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            scheduled_task_id: self.scheduled_task_id,
            reserve_id: self.reserve_id,
            agent_id: self.agent_id.clone(),
            skill_id: self.skill_id,
            interface_version: self.interface_version,
            source_kind: self.source_kind.clone(),
            prepaid_amount: self.prepaid_amount,
            occurrence_budget: self.occurrence_budget,
            resolved: self.resolved,
            canceled: self.canceled,
        }
    }
}

pub(crate) fn deserialize_skill_payment_policy<'de, D>(
    deserializer: D,
) -> Result<SkillPaymentPolicy, D::Error>
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
    use {super::*, crate::types::sui_framework::vec_map::VecMap};

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
        assert_eq!(
            payment.id.id.bytes,
            crate::sui::types::Address::from_static("0xa1")
        );
        let _: VecMap<Vec<u8>, u64> = payment.tool_cost_snapshot;
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
