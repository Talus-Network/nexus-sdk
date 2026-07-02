//! SDK helpers for generated TAP bindings.

use {
    crate::{
        move_bindings::{
            interface::{
                agent::{
                    Agent, AgentPaymentVault, AgentVaultFieldKey, FixedTool, SkillDagBinding,
                    SkillRecurrenceKind, SkillRequirement, SkillSchedulePolicy,
                },
                payment::{PaymentSourceKind, SkillPaymentPolicy},
                version::InterfaceVersion,
            },
            registry::agent_registry::{
                DefaultDagExecutor, DefaultDagExecutorFieldKey, SkillRecord,
            },
            sui_framework::object::{ID, UID},
            workflow::execution::DagExecutionPaymentFieldKey,
        },
        sui,
        types::{AgentId, DefaultDagExecutorTarget},
    },
    std::fmt,
};

impl PartialOrd for InterfaceVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InterfaceVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl fmt::Display for InterfaceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl Default for SkillPaymentPolicy {
    fn default() -> Self {
        Self::UserFunded
    }
}

impl SkillPaymentPolicy {
    pub fn user_funded() -> Self {
        Self::UserFunded
    }

    pub fn agent_funded(max_budget: u64) -> Self {
        Self::AgentFunded { max_budget }
    }

    pub fn max_budget(&self) -> u64 {
        match self {
            Self::UserFunded => 0,
            Self::AgentFunded { max_budget } => *max_budget,
        }
    }
}

impl PaymentSourceKind {
    pub fn user_funded(user: sui::types::Address) -> Self {
        Self::UserFunded { user }
    }

    pub fn agent_funded(agent_id: AgentId) -> Self {
        Self::AgentFunded {
            agent_id: ID::new(agent_id),
        }
    }

    pub fn identity(&self) -> sui::types::Address {
        match self {
            Self::UserFunded { user } => *user,
            Self::AgentFunded { agent_id } => agent_id.clone().into(),
        }
    }
}

impl SkillDagBinding {
    pub fn pinned(dag_id: sui::types::Address) -> Self {
        Self::Pinned { dag_id }
    }

    pub fn runtime_selected() -> Self {
        Self::RuntimeSelected
    }

    pub fn pinned_dag_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::Pinned { dag_id } => Some(*dag_id),
            Self::RuntimeSelected => None,
        }
    }
}

impl Default for SkillSchedulePolicy {
    fn default() -> Self {
        Self {
            recurrence: SkillRecurrenceKind::Once,
            allow_recursive: false,
        }
    }
}

impl FixedTool {
    pub fn tool_registry_address(&self) -> sui::types::Address {
        self.tool_registry_id.clone().into()
    }

    pub fn tool_fqn_string(&self) -> String {
        self.tool_fqn.clone().into()
    }
}

impl Default for SkillRequirement {
    fn default() -> Self {
        Self {
            input_commitment: Vec::new(),
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        }
    }
}

impl SkillRecord {
    pub fn description_bytes(&self) -> &[u8] {
        &self.description
    }
}

impl Agent {
    pub fn object_id(&self) -> AgentId {
        self.id.clone().into()
    }

    pub fn from_ids(
        agent_id: AgentId,
        next_skill_id: u64,
        registry_id: Option<sui::types::Address>,
    ) -> Self {
        Self {
            id: UID::new(agent_id),
            next_skill_id,
            registry_id: registry_id.map(ID::new).into(),
        }
    }
}

impl DefaultDagExecutor {
    pub fn target(&self) -> DefaultDagExecutorTarget {
        DefaultDagExecutorTarget {
            agent_id: self.agent.object_id(),
            skill_id: self.skill_id,
        }
    }
}

impl Default for DefaultDagExecutorFieldKey {
    fn default() -> Self {
        Self { dummy_field: false }
    }
}

impl AgentPaymentVault {
    pub fn object_id(&self) -> AgentId {
        self.id.id.bytes
    }

    pub fn agent_id_address(&self) -> AgentId {
        self.agent_id.bytes
    }

    pub fn available_balance_value(&self) -> u64 {
        self.available_balance.value
    }

    pub fn unlocked_balance_value(&self) -> u64 {
        self.available_balance
            .value
            .saturating_sub(self.locked_amount)
    }
}

impl Default for DagExecutionPaymentFieldKey {
    fn default() -> Self {
        Self { dummy_field: false }
    }
}

impl Default for AgentVaultFieldKey {
    fn default() -> Self {
        Self { dummy_field: false }
    }
}
