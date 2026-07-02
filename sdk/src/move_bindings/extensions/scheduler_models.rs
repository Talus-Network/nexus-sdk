use crate::{
    move_bindings::{
        interface::agent as agent_move, move_std::type_name::TypeName,
        primitives::policy::Symbol as PolicySymbol,
    },
    sui,
};

impl agent_move::ExecutionSelection {
    pub fn dag_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::AgentSkill { selected_dag, .. } => {
                selected_dag.as_option().map(|dag_id| dag_id.bytes)
            }
            Self::DefaultAgent { dag_id } => Some(dag_id.bytes),
        }
    }

    pub fn is_default_agent(&self) -> bool {
        matches!(self, Self::DefaultAgent { .. })
    }

    pub fn is_agent_skill(&self) -> bool {
        matches!(self, Self::AgentSkill { .. })
    }
}

impl agent_move::AgentExecutionConfig {
    pub fn network_address(&self) -> sui::types::Address {
        self.network.bytes
    }

    pub fn entry_group_name(&self) -> &str {
        self.entry_group.as_str()
    }
}

impl PolicySymbol {
    pub fn witness(name: TypeName) -> Self {
        Self::Witness { pos0: name }
    }

    pub fn uid(uid: sui::types::Address) -> Self {
        Self::Uid {
            pos0: crate::move_bindings::sui_framework::object::ID::new(uid),
        }
    }

    pub fn as_witness(&self) -> Option<&TypeName> {
        match self {
            PolicySymbol::Witness { pos0 } => Some(pos0),
            PolicySymbol::Uid { .. } => None,
        }
    }

    pub fn as_uid(&self) -> Option<&sui::types::Address> {
        match self {
            PolicySymbol::Uid { pos0 } => Some(&pos0.bytes),
            PolicySymbol::Witness { .. } => None,
        }
    }

    pub fn matches_qualified_name(&self, expected: &str) -> bool {
        self.as_witness()
            .map(|name| name.matches_qualified_name(expected))
            .unwrap_or(false)
    }
}
