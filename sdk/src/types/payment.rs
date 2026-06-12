//! Types for `nexus_interface::payment`.

use {
    super::{InterfaceVersion, SkillPaymentPolicy},
    crate::sui::{self, types::SuiBalance},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentPaymentSourceKind {
    UserFunded { user: sui::types::Address },
    AgentFunded { agent_id: sui::types::Address },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentExecutionPaymentFinalState {
    Pending,
    Accomplished,
    Refunded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentVertexExecutionPaymentSettlementKind {
    Free,
    Ticket,
    Paid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentExecutionPayment {
    pub id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub agent_id: sui::types::Address,
    pub skill_id: u64,
    pub interface_revision: InterfaceVersion,
    pub payment_policy: SkillPaymentPolicy,
    pub source_kind: CurrentPaymentSourceKind,
    pub max_budget: u64,
    pub locked_budget: u64,
    pub funds: SuiBalance,
    pub consumed: u64,
    pub accomplished: bool,
    pub refunded: bool,
    pub final_state: CurrentExecutionPaymentFinalState,
    pub tool_cost_snapshot: PaymentVecMap<Vec<u8>, u64>,
    pub locked_vertices: Vec<CurrentExecutionPaymentVertexLock>,
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
pub struct CurrentExecutionPaymentVertexLock {
    pub vertex_key: Vec<u8>,
    pub tool_fqn: Vec<u8>,
    pub amount: u64,
    pub settlement_kind: CurrentVertexExecutionPaymentSettlementKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentExecutionPaymentReceipt {
    pub id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub payment_id: sui::types::Address,
    pub agent_id: sui::types::Address,
    pub skill_id: u64,
    pub source_kind: CurrentPaymentSourceKind,
    pub max_budget: u64,
    pub resolved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledPaymentReserveReceipt {
    pub id: sui::types::Address,
    pub scheduled_task_id: sui::types::Address,
    pub reserve_id: sui::types::Address,
    pub agent_id: sui::types::Address,
    pub skill_id: u64,
    pub interface_version: InterfaceVersion,
    pub source_kind: CurrentPaymentSourceKind,
    pub prepaid_amount: u64,
    pub occurrence_budget: u64,
    pub resolved: bool,
    pub canceled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentExecutionPaymentHistoryList {
    pub id: sui::types::Address,
    pub execution_ids: Vec<sui::types::Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentScheduledOccurrenceRecord {
    pub occurrence_index: u64,
    pub execution_id: sui::types::Address,
    pub payment_id: sui::types::Address,
    pub interface_revision: InterfaceVersion,
    pub budget: u64,
    pub final_state: CurrentScheduledOccurrenceFinalState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrentScheduledOccurrenceFinalState {
    InFlight,
    Accomplished,
    Refunded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledPaymentReserve {
    pub id: sui::types::Address,
    pub scheduled_task_id: sui::types::Address,
    pub agent_id: sui::types::Address,
    pub skill_id: u64,
    pub interface_version: InterfaceVersion,
    pub agent_skill_authorization_id: sui::types::Address,
    pub payment_source: CurrentPaymentSourceKind,
    pub occurrence_budget: u64,
    pub remaining_funds: SuiBalance,
    pub payment_policy: SkillPaymentPolicy,
    pub in_flight: Vec<CurrentScheduledOccurrenceRecord>,
    pub payment_receipts: Vec<CurrentExecutionPaymentReceipt>,
}
