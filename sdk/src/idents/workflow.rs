use crate::{
    idents::ModuleAndNameIdent,
    sui,
    types::{FailureEvidenceKind, VerifierConfig, VerifierMode},
};

// == `nexus_workflow::{execution, execution_entries, execution_resolution, execution_settlement, execution_submission}` and `nexus_interface::verifier` ==

const EXECUTION_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("execution");
const EXECUTION_ENTRIES_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("execution_entries");
const EXECUTION_RESOLUTION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("execution_resolution");
const EXECUTION_SETTLEMENT_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("execution_settlement");
const EXECUTION_SUBMISSION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("execution_submission");
const VERIFIER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("verifier");

pub struct Verifier;

impl Verifier {
    pub const FAILURE_EVIDENCE_KIND: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("FailureEvidenceKind"),
    };
    pub const FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("failure_evidence_kind_leader_evidence"),
    };
    pub const FAILURE_EVIDENCE_KIND_TOOL_EVIDENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("failure_evidence_kind_tool_evidence"),
    };
    pub const VERIFIER_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("verifier_config"),
    };
    pub const VERIFIER_MODE_AUTHENTICATED_COMMUNICATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("verifier_mode_authenticated_communication"),
    };
    pub const VERIFIER_MODE_NONE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("verifier_mode_none"),
    };
    pub const VERIFIER_MODE_TOOL_VERIFIER_CONTRACT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("verifier_mode_tool_verifier_contract"),
    };

    pub fn failure_evidence_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        evidence_kind: &FailureEvidenceKind,
    ) -> sui::tx::Argument {
        let ident = match evidence_kind {
            FailureEvidenceKind::ToolEvidence => Self::FAILURE_EVIDENCE_KIND_TOOL_EVIDENCE,
            FailureEvidenceKind::LeaderEvidence => Self::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE,
        };
        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    pub fn verifier_mode_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        mode: &VerifierMode,
    ) -> sui::tx::Argument {
        let ident = match mode {
            VerifierMode::None => Self::VERIFIER_MODE_NONE,
            VerifierMode::LeaderRegisteredKey | VerifierMode::LeaderNautilusEnclave => {
                Self::VERIFIER_MODE_AUTHENTICATED_COMMUNICATION
            }
            VerifierMode::ToolVerifierContract => Self::VERIFIER_MODE_TOOL_VERIFIER_CONTRACT,
        };
        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    pub fn verifier_config(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        config: &VerifierConfig,
    ) -> anyhow::Result<sui::tx::Argument> {
        let mode = Self::verifier_mode_from_enum(tx, interface_pkg_id, &config.mode);
        let method = super::move_std::Ascii::ascii_string_from_str(tx, &config.method)?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::VERIFIER_CONFIG.module,
                Self::VERIFIER_CONFIG.name,
            ),
            vec![mode, method],
        ))
    }
}

pub struct Execution;

impl Execution {
    /// Bind the leader registry to Nexus Workflow.
    ///
    /// `nexus_workflow::execution::bind_leader_registry_workflow`
    pub const BIND_LEADER_REGISTRY_WORKFLOW: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_MODULE,
        name: sui::types::Identifier::from_static("bind_leader_registry_workflow"),
    };
    pub const DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_MODULE,
        name: sui::types::Identifier::from_static("DAGExecution"),
    };
}

pub struct ExecutionEntries;

impl ExecutionEntries {
    pub const ADVANCE_FOR_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("AdvanceForAgentExecution"),
    };
    pub const ADVANCE_FOR_DEFAULT_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("AdvanceForDefaultAgentExecution"),
    };
    pub const BEGIN_AGENT_FUNDED_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("begin_agent_funded_agent_execution"),
    };
    pub const BEGIN_DEFAULT_DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("begin_default_dag_execution"),
    };
    pub const BEGIN_USER_FUNDED_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("begin_user_funded_agent_execution"),
    };
    pub const REQUEST_NETWORK_TO_EXECUTE_WALKS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("request_network_to_execute_walks"),
    };
    pub const REQUEST_WALK_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("request_walk_execution"),
    };
    pub const REQUEST_WALK_EXECUTION_FOR_WALK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_ENTRIES_MODULE,
        name: sui::types::Identifier::from_static("request_walk_execution_for_walk"),
    };
}

pub struct ExecutionSubmission;

impl ExecutionSubmission {
    pub const AUTHORIZE_WALK_SUBMISSION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_SUBMISSION_MODULE,
        name: sui::types::Identifier::from_static("authorize_walk_submission"),
    };
    pub const LEADER_STAMP_WORKSHEET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_SUBMISSION_MODULE,
        name: sui::types::Identifier::from_static("leader_stamp_worksheet"),
    };
    pub const NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: EXECUTION_SUBMISSION_MODULE,
            name: sui::types::Identifier::from_static(
                "new_authenticated_offchain_request_evidence_v1",
            ),
        };
    pub const RELEASE_VERTEX_AUTHORIZATION_FOR_ONCHAIN_WALK: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: EXECUTION_SUBMISSION_MODULE,
            name: sui::types::Identifier::from_static(
                "release_vertex_authorization_for_onchain_walk",
            ),
        };
    pub const SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_SUBMISSION_MODULE,
        name: sui::types::Identifier::from_static("submit_off_chain_tool_result_for_walk_v1"),
    };
    pub const SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: EXECUTION_SUBMISSION_MODULE,
            name: sui::types::Identifier::from_static(
                "submit_off_chain_tool_result_for_walk_without_verifier_v1",
            ),
        };
    pub const SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_SUBMISSION_MODULE,
        name: sui::types::Identifier::from_static("submit_on_chain_tool_result_for_walk_v1"),
    };
    pub const WORKSHEET_FOR_TOOL_RESULT_SUBMISSION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_SUBMISSION_MODULE,
        name: sui::types::Identifier::from_static("worksheet_for_tool_result_submission"),
    };
}

pub struct ExecutionResolution;

impl ExecutionResolution {
    pub const ABORT_EXPIRED_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_RESOLUTION_MODULE,
        name: sui::types::Identifier::from_static("abort_expired_execution"),
    };
}

pub struct ExecutionSettlement;

impl ExecutionSettlement {
    pub const ACCOMPLISH_TAP_EXECUTION_PAYMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_SETTLEMENT_MODULE,
        name: sui::types::Identifier::from_static("accomplish_tap_execution_payment"),
    };
    pub const ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: EXECUTION_SETTLEMENT_MODULE,
            name: sui::types::Identifier::from_static(
                "accomplish_tap_execution_payment_from_agent_vault",
            ),
        };
    pub const REFUND_TAP_EXECUTION_PAYMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EXECUTION_SETTLEMENT_MODULE,
        name: sui::types::Identifier::from_static("refund_tap_execution_payment"),
    };
    pub const REFUND_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: EXECUTION_SETTLEMENT_MODULE,
            name: sui::types::Identifier::from_static(
                "refund_tap_execution_payment_from_agent_vault",
            ),
        };
}

pub struct Gas;

const GAS_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("gas");

impl Gas {
    /// Derive and share a `ToolGas` object while setting the initial invocation price.
    ///
    /// `nexus_workflow::gas::create_tool_gas_and_share`
    pub const CREATE_TOOL_GAS_AND_SHARE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("create_tool_gas_and_share"),
    };
    /// De-escalate an OverTool owner cap into OverGas.
    ///
    /// `nexus_workflow::gas::deescalate`
    pub const DEESCALATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("deescalate"),
    };
    /// Finalize payment settlement by transferring funds to a tool vault.
    ///
    /// `nexus_workflow::gas::finalize_payment_state_for_vertex`
    pub const FINALIZE_PAYMENT_STATE_FOR_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("finalize_payment_state_for_vertex"),
    };
    /// GasService type for lookups.
    ///
    /// `nexus_workflow::gas::GasService`
    pub const GAS_SERVICE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("GasService"),
    };
    /// Lock execution payment for a tool in the current execution.
    ///
    /// `nexus_workflow::gas::lock_payment_state_for_tool`
    pub const LOCK_PAYMENT_STATE_FOR_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("lock_payment_state_for_tool"),
    };
    /// OverGas owner cap generic.
    ///
    /// `nexus_workflow::gas::OverGas`
    pub const OVER_GAS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("OverGas"),
    };
    /// Refund payment for a vertex in a tool's context.
    ///
    /// `nexus_workflow::gas::refund_payment_state_for_vertex`
    pub const REFUND_PAYMENT_STATE_FOR_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("refund_payment_state_for_vertex"),
    };
    /// Create an agent scope.
    ///
    /// `nexus_workflow::gas::scope_agent`
    pub const SCOPE_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("scope_agent"),
    };
    /// Create an Execution scope.
    ///
    /// `nexus_workflow::gas::scope_execution`
    pub const SCOPE_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("scope_execution"),
    };
    /// Create an InvokerAddress scope.
    ///
    /// `nexus_workflow::gas::scope_invoker_address`
    pub const SCOPE_INVOKER_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("scope_invoker_address"),
    };
    /// Create a WorksheetType scope.
    ///
    /// `nexus_workflow::gas::scope_worksheet_type`
    pub const SCOPE_WORKSHEET_TYPE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("scope_worksheet_type"),
    };
    /// Settle payment for a vertex using the DAG pending-settlement directive.
    ///
    /// `nexus_workflow::gas::settle_payment_state_for_vertex`
    pub const SETTLE_PAYMENT_STATE_FOR_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("settle_payment_state_for_vertex"),
    };
    /// Set a tool invocation cost in MIST.
    ///
    /// `nexus_workflow::gas::set_single_invocation_cost_mist`
    pub const SET_SINGLE_INVOCATION_COST_MIST: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("set_single_invocation_cost_mist"),
    };
    /// Snapshot all DAG tool costs into a TAP execution payment.
    ///
    /// `nexus_workflow::gas::snapshot_dag_tool_costs`
    pub const SNAPSHOT_DAG_TOOL_COSTS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("snapshot_dag_tool_costs"),
    };
}

// == `nexus_workflow::gas_extension` ==

pub struct GasExtension;

const GAS_EXTENSION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("gas_extension");

impl GasExtension {
    /// Abort an expired DAG execution using the matching ToolGas.
    ///
    /// `nexus_workflow::gas_extension::abort_expired_execution_with_tool_gas`
    pub const ABORT_EXPIRED_EXECUTION_WITH_TOOL_GAS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("abort_expired_execution_with_tool_gas"),
    };
    /// Buy an expiry gas extension ticket.
    ///
    /// `nexus_workflow::gas_extension::buy_expiry_gas_ticket`
    pub const BUY_EXPIRY_GAS_TICKET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("buy_expiry_gas_ticket"),
    };
    /// Buy a limited invocations gas extension ticket.
    ///
    /// `nexus_workflow::gas_extension::buy_limited_invocations_gas_ticket`
    pub const BUY_LIMITED_INVOCATIONS_GAS_TICKET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("buy_limited_invocations_gas_ticket"),
    };
    /// Disable expiry gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::disable_expiry`
    pub const DISABLE_EXPIRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("disable_expiry"),
    };
    /// Disable limited invocations gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::disable_limited_invocations`
    pub const DISABLE_LIMITED_INVOCATIONS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("disable_limited_invocations"),
    };
    /// Enable expiry gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::enable_expiry`
    pub const ENABLE_EXPIRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("enable_expiry"),
    };
    /// Enable limited invocations gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::enable_limited_invocations`
    pub const ENABLE_LIMITED_INVOCATIONS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("enable_limited_invocations"),
    };
}

/// Helper to turn a `ModuleAndNameIdent` into a `sui::types::TypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(
    workflow_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        workflow_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_type_tag() {
        let rng = &mut rand::thread_rng();
        let workflow_pkg_id = sui::types::Address::generate(rng);
        let ident = ModuleAndNameIdent {
            module: sui::types::Identifier::from_static("foo"),
            name: sui::types::Identifier::from_static("bar"),
        };

        let tag = into_type_tag(workflow_pkg_id, ident);

        assert_eq!(
            tag,
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                workflow_pkg_id,
                sui::types::Identifier::from_static("foo"),
                sui::types::Identifier::from_static("bar"),
                vec![],
            )))
        )
    }
}
