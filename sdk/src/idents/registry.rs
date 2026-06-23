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

pub struct ToolRegistry;

const TOOL_REGISTRY_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("tool_registry");

impl ToolRegistry {
    /// Claim collateral for a tool. The function call returns Balance<SUI>.
    ///
    /// `nexus_registry::tool_registry::claim_collateral`
    pub const CLAIM_COLLATERAL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("claim_collateral"),
    };
    /// Claim collateral for a tool and transfer the balance to the tx sender.
    ///
    /// `nexus_registry::tool_registry::claim_collateral_for_self`
    pub const CLAIM_COLLATERAL_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("claim_collateral_for_self"),
    };
    /// OverSlashing struct type. Used to fetch caps for slashing tools.
    ///
    /// `nexus_registry::tool_registry::OverSlashing`
    pub const OVER_SLASHING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("OverSlashing"),
    };
    /// OverTool struct type. Used for fetching tool owner caps.
    ///
    /// `nexus_registry::tool_registry::OverTool`
    pub const OVER_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("OverTool"),
    };
    /// Register an off-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_registry::tool_registry::register_off_chain_tool`
    pub const REGISTER_OFF_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_off_chain_tool"),
    };
    /// Register an on-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_registry::tool_registry::register_on_chain_tool`
    pub const REGISTER_ON_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_on_chain_tool"),
    };
    /// Register a cap-gated on-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_registry::tool_registry::register_on_chain_tool_with_workflow_authorization_cap`
    pub const REGISTER_ON_CHAIN_TOOL_WITH_WORKFLOW_AUTHORIZATION_CAP: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: TOOL_REGISTRY_MODULE,
            name: sui::types::Identifier::from_static(
                "register_on_chain_tool_with_workflow_authorization_cap",
            ),
        };
    /// Configure verifier methods supported by an off-chain tool.
    ///
    /// `nexus_registry::tool_registry::set_off_chain_supported_verifier_methods`
    pub const SET_OFF_CHAIN_SUPPORTED_VERIFIER_METHODS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("set_off_chain_supported_verifier_methods"),
    };
    /// Tool struct type. Used for fetching tool info.
    ///
    /// `nexus_registry::tool_registry::Tool`
    pub const TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("Tool"),
    };
    /// The ToolRegistry struct type.
    ///
    /// `nexus_registry::tool_registry::ToolRegistry`
    pub const TOOL_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("ToolRegistry"),
    };
    /// Unregister a tool.
    ///
    /// `nexus_registry::tool_registry::unregister`
    pub const UNREGISTER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("unregister"),
    };
    /// Update a tool's timeout.
    ///
    /// `nexus_registry::tool_registry::update_tool_timeout`
    pub const UPDATE_TOOL_TIMEOUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("update_tool_timeout"),
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

// == `nexus_registry::leader_cap` ==

pub struct LeaderCap;

pub const LEADER_CAP_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("leader_cap");

impl LeaderCap {
    /// Create N leader caps for self and the provided addresses.
    ///
    /// `nexus_registry::leader_cap::create_for_self_and_addresses`
    pub const CREATE_FOR_SELF_AND_ADDRESSES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::types::Identifier::from_static("create_for_self_and_addresses"),
    };
    /// This is used as a generic argument for
    /// [crate::idents::primitives::OwnerCap::CLONEABLE_OWNER_CAP].
    ///
    /// `nexus_registry::leader_cap::OverNetwork`
    pub const OVER_NETWORK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::types::Identifier::from_static("OverNetwork"),
    };
}

// == `nexus_registry::leader` ==

pub struct Leader;

pub const LEADER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("leader");

impl Leader {
    /// Activate a leader and claim ownership of its `Active` state with a fresh token.
    ///
    /// `nexus_registry::leader::activate_and_claim`
    pub const ACTIVATE_AND_CLAIM: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("activate_and_claim"),
    };
    /// Allow an address to request leader capabilities.
    ///
    /// `nexus_registry::leader::allow_address`
    pub const ALLOW_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("allow_address"),
    };
    /// Disallow an address from requesting leader capabilities.
    ///
    /// `nexus_registry::leader::disallow_address`
    pub const DISALLOW_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("disallow_address"),
    };
    /// Create empty metadata for a leader.
    ///
    /// `nexus_registry::leader::empty_metadata`
    pub const EMPTY_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("empty_metadata"),
    };
    /// Admin capability type for modifying leader allowlist.
    ///
    /// `nexus_registry::leader::LeaderCapabilitiesAdminCap`
    pub const LEADER_CAPABILITIES_ADMIN_CAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("LeaderCapabilitiesAdminCap"),
    };
    /// LeaderRegistry type for lookups.
    ///
    /// `nexus_registry::leader::LeaderRegistry`
    pub const LEADER_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("LeaderRegistry"),
    };
    /// Create metadata with the provided map.
    ///
    /// `nexus_registry::leader::new_metadata`
    pub const NEW_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("new_metadata"),
    };
    /// Register the caller as a leader and stake.
    ///
    /// `nexus_registry::leader::register`
    pub const REGISTER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("register"),
    };
    /// Configure verifier methods supported by a leader.
    ///
    /// `nexus_registry::leader::set_supported_verifier_methods`
    pub const SET_SUPPORTED_VERIFIER_METHODS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("set_supported_verifier_methods"),
    };
    /// Stake SUI into a leader's pool.
    ///
    /// `nexus_registry::leader::stake`
    pub const STAKE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("stake"),
    };
    /// Suspend a leader only if the caller still holds the on-record claim token.
    ///
    /// `nexus_registry::leader::suspend_if_token`
    pub const SUSPEND_IF_TOKEN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("suspend_if_token"),
    };
}

// == `nexus_registry::verifier_registry` ==

pub struct VerifierRegistry;

pub const VERIFIER_REGISTRY_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("verifier_registry");

impl VerifierRegistry {
    /// Verifier registry shared object type.
    ///
    /// `nexus_registry::verifier_registry::VerifierRegistry`
    pub const VERIFIER_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("VerifierRegistry"),
    };
    /// Admin capability for verifier registry configuration.
    ///
    /// `nexus_registry::verifier_registry::VerifierRegistryAdminCap`
    pub const VERIFIER_REGISTRY_ADMIN_CAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("VerifierRegistryAdminCap"),
    };
    /// Configure the DAG-wide default leader verifier policy after validating the method.
    ///
    /// `nexus_registry::verifier_registry::with_default_leader_verifier`
    pub const WITH_DEFAULT_LEADER_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("with_default_leader_verifier"),
    };
    /// Configure the DAG-wide default tool verifier policy after validating the method.
    ///
    /// `nexus_registry::verifier_registry::with_default_tool_verifier`
    pub const WITH_DEFAULT_TOOL_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("with_default_tool_verifier"),
    };
    /// Configure a vertex leader verifier after validating the method.
    ///
    /// `nexus_registry::verifier_registry::with_vertex_leader_verifier`
    pub const WITH_VERTEX_LEADER_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_leader_verifier"),
    };
    /// Configure a vertex tool verifier after validating the method.
    ///
    /// `nexus_registry::verifier_registry::with_vertex_tool_verifier`
    pub const WITH_VERTEX_TOOL_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_tool_verifier"),
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
