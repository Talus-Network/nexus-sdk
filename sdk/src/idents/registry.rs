use crate::{idents::ModuleAndNameIdent, sui};

// == `nexus_registry::agent_registry` ==

pub struct AgentRegistry;

pub const AGENT_REGISTRY_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("agent_registry");

impl AgentRegistry {
    pub const AGENT_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("AgentRegistry"),
    };
    pub const BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: AGENT_REGISTRY_MODULE,
            name: sui::types::Identifier::from_static(
                "bootstrap_default_runtime_dag_skill_for_deployment",
            ),
        };
    pub const CREATE_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("create_agent"),
    };
    pub const CREATE_SKILL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("create_skill"),
    };
    pub const GET_SKILL_REQUIREMENTS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("get_skill_requirements"),
    };
    pub const NEW_DEFAULT_DAG_EXECUTOR_SCHEDULED_OCCURRENCE_PAYMENT_FOR_EXECUTION:
        ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static(
            "new_default_dag_executor_scheduled_occurrence_payment_for_execution",
        ),
    };
    pub const NEW_SCHEDULED_OCCURRENCE_PAYMENT_FOR_EXECUTION: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: AGENT_REGISTRY_MODULE,
            name: sui::types::Identifier::from_static(
                "new_scheduled_occurrence_payment_for_execution",
            ),
        };
    pub const REGISTER_SKILL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_skill"),
    };
    pub const REGISTER_SKILL_WITH_FIXED_TOOLS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_skill_with_fixed_tools"),
    };
    pub const SET_AGENT_ACTIVE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("set_agent_active"),
    };
    pub const SET_SKILL_ACTIVE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("set_skill_active"),
    };
    pub const TRIGGER_SCHEDULED_SKILL_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("trigger_scheduled_skill_execution"),
    };
    pub const UPDATE_DAG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("update_dag"),
    };
    pub const UPDATE_SKILL_DESCRIPTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("update_skill_description"),
    };
    pub const UPDATE_SKILL_POLICIES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("update_skill_policies"),
    };
    pub const WITHDRAW_AGENT_PAYMENT_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: AGENT_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("withdraw_agent_payment_vault"),
    };
}

// == `nexus_registry::network_auth` ==

pub struct NetworkAuth;

pub const NETWORK_AUTH_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("network_auth");

impl NetworkAuth {
    /// Create a new key binding for an identity.
    ///
    /// `nexus_registry::network_auth::create_binding`
    pub const CREATE_BINDING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("create_binding"),
    };
    /// Move type `nexus_registry::network_auth::IdentityKey`.
    pub const IDENTITY_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("IdentityKey"),
    };
    /// Move type `nexus_registry::network_auth::KeyBinding`.
    pub const KEY_BINDING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("KeyBinding"),
    };
    /// The NetworkAuth struct type.
    ///
    /// `nexus_registry::network_auth::NetworkAuth`
    pub const NETWORK_AUTH: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("NetworkAuth"),
    };
    /// Construct a proof-of-possession for a key registration slot.
    ///
    /// `nexus_registry::network_auth::new_proof_of_key`
    pub const NEW_PROOF_OF_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("new_proof_of_key"),
    };
    /// Create proof-of-identity for a leader, using a leader capability.
    ///
    /// `nexus_registry::network_auth::prove_leader`
    pub const PROVE_LEADER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("prove_leader"),
    };
    /// Create proof-of-identity for an off-chain tool.
    ///
    /// `nexus_registry::network_auth::prove_offchain_tool`
    pub const PROVE_OFFCHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("prove_offchain_tool"),
    };
    /// Register a new key on an existing binding and set it active.
    ///
    /// `nexus_registry::network_auth::register_key`
    pub const REGISTER_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("register_key"),
    };
}

// == `nexus_registry::leader` ==

/// Registry-side leader identifiers are separate from `workflow::Leader`; the existing workflow identifier struct only models the old workflow package target and does not include registry-only mutators such as `set_status`.
pub struct Leader;

pub const LEADER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("leader");

impl Leader {
    /// Set a leader's status through the on-record leader capability.
    ///
    /// `nexus_registry::leader::set_status`
    pub const SET_STATUS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("set_status"),
    };
    /// Create `LeaderStatus::Active`.
    ///
    /// `nexus_registry::leader::status_active`
    pub const STATUS_ACTIVE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("status_active"),
    };
    /// Create `LeaderStatus::Suspended`.
    ///
    /// `nexus_registry::leader::status_suspended`
    pub const STATUS_SUSPENDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("status_suspended"),
    };
}

/// Helper to turn a registry `ModuleAndNameIdent` into a `sui::types::TypeTag`.
pub fn into_type_tag(
    registry_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        registry_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}
