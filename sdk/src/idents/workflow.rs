use crate::{
    idents::{pure_arg, ModuleAndNameIdent},
    sui,
    types::{
        EdgeKind,
        FailureEvidenceKind,
        PostFailureAction,
        RuntimeVertex,
        VerifierConfig,
        VerifierMode,
    },
    ToolFqn,
};

// == `nexus_workflow::dag` ==

pub struct Dag;

const DAG_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("dag");

impl Dag {
    /// Abort an expired DAG execution.
    ///
    /// `nexus_workflow::dag::abort_expired_execution`
    pub const ABORT_EXPIRED_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("abort_expired_execution"),
    };
    /// Accomplish an invoker-funded TAP payment owned by DAGExecution.
    ///
    /// `nexus_workflow::dag::accomplish_tap_execution_payment`
    pub const ACCOMPLISH_TAP_EXECUTION_PAYMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("accomplish_tap_execution_payment"),
    };
    /// Accomplish an agent-vault-funded TAP payment owned by DAGExecution.
    ///
    /// `nexus_workflow::dag::accomplish_tap_execution_payment_from_agent_vault`
    pub const ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "accomplish_tap_execution_payment_from_agent_vault",
            ),
        };
    /// The AgentExecutionConfig struct type.
    ///
    /// `nexus_workflow::dag::AgentExecutionConfig`
    pub const AGENT_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("AgentExecutionConfig"),
    };
    /// Begin DAG execution through an explicit agent path using a prepared config.
    ///
    /// `nexus_workflow::dag::begin_agent_execution_with_config`
    pub const BEGIN_AGENT_EXECUTION_WITH_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("begin_agent_execution_with_config"),
    };
    /// Witness type used by scheduler tasks that execute a registered agent target.
    ///
    /// `nexus_workflow::dag::BeginAgentExecutionWitness`
    pub const BEGIN_AGENT_EXECUTION_WITNESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("BeginAgentExecutionWitness"),
    };
    /// `nexus_workflow::dag::BEGIN_DAG_EXECUTION_WITH_CONFIG`
    pub const BEGIN_DAG_EXECUTION_WITH_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("begin_dag_execution_with_config"),
    };
    /// Witness type used by scheduler tasks that execute through the default agent target.
    ///
    /// `nexus_workflow::dag::BeginDefaultAgentExecutionWitness`
    pub const BEGIN_DEFAULT_AGENT_EXECUTION_WITNESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("BeginDefaultAgentExecutionWitness"),
    };
    /// Create a workflow-owned vertex authorization grant under a DAGExecution.
    ///
    /// `nexus_workflow::dag::create_vertex_authorization_grant`
    pub const CREATE_VERTEX_AUTHORIZATION_GRANT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("create_vertex_authorization_grant"),
    };
    /// The DAG struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::DAG`
    pub const DAG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("DAG"),
    };
    /// The DAGExecution struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::DAGExecution`
    pub const DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("DAGExecution"),
    };
    /// The DagExecutionConfig struct type.
    ///
    /// `nexus_workflow::dag::DagExecutionConfig`
    pub const DAG_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("DagExecutionConfig"),
    };
    /// Create a break edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_break`
    pub const EDGE_KIND_BREAK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_break"),
    };
    /// Create a collect edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_collect`
    pub const EDGE_KIND_COLLECT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_collect"),
    };
    /// Create a do-while edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_do_while`
    pub const EDGE_KIND_DO_WHILE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_do_while"),
    };
    /// Create a for-each edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_for_each`
    pub const EDGE_KIND_FOR_EACH: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_for_each"),
    };
    /// Create a normal edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_normal`
    pub const EDGE_KIND_NORMAL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_normal"),
    };
    /// Create a static edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_static`
    pub const EDGE_KIND_STATIC: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_static"),
    };
    /// The EntryGroup struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::EntryGroup`
    pub const ENTRY_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("EntryGroup"),
    };
    /// Create an EntryGroup from an ASCII string.
    ///
    /// `nexus_workflow::dag::entry_group_from_string`
    pub const ENTRY_GROUP_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("entry_group_from_string"),
    };
    /// The FailureEvidenceKind enum. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::FailureEvidenceKind`
    pub const FAILURE_EVIDENCE_KIND: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("FailureEvidenceKind"),
    };
    /// Create a leader-evidence failure evidence kind.
    ///
    /// `nexus_workflow::dag::failure_evidence_kind_leader_evidence`
    pub const FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("failure_evidence_kind_leader_evidence"),
    };
    /// Create a tool-evidence failure evidence kind.
    ///
    /// `nexus_workflow::dag::failure_evidence_kind_tool_evidence`
    pub const FAILURE_EVIDENCE_KIND_TOOL_EVIDENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("failure_evidence_kind_tool_evidence"),
    };
    /// The InputPort struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::InputPort`
    pub const INPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("InputPort"),
    };
    /// Create an InputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::input_port_from_string`
    pub const INPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("input_port_from_string"),
    };
    /// Stamp a TAP worksheet as the leader before tool execution.
    ///
    /// `nexus_workflow::dag::leader_stamp_tap_worksheet`
    pub const LEADER_STAMP_TAP_WORKSHEET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("leader_stamp_tap_worksheet"),
    };
    /// Stamp a worksheet as the leader before tool execution.
    ///
    /// `nexus_workflow::dag::leader_stamp_worksheet`
    pub const LEADER_STAMP_WORKSHEET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("leader_stamp_worksheet"),
    };
    /// Stamp a worksheet as the leader during a dry run.
    ///
    /// `nexus_workflow::dag::leader_stamp_worksheet_for_dry_run`
    pub const LEADER_STAMP_WORKSHEET_FOR_DRY_RUN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("leader_stamp_worksheet_for_dry_run"),
    };
    /// Mint a workflow vertex authorization check cap for the current on-chain walk.
    ///
    /// `nexus_workflow::dag::mint_vertex_authorization_check_cap_for_onchain_walk`
    pub const MINT_VERTEX_AUTHORIZATION_CHECK_CAP_FOR_ONCHAIN_WALK: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "mint_vertex_authorization_check_cap_for_onchain_walk",
            ),
        };
    /// Create a new DAG object.
    ///
    /// `nexus_workflow::dag::new`
    pub const NEW: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("new"),
    };
    /// Create a new explicit agent execution config value.
    ///
    /// `nexus_workflow::dag::new_agent_execution_config`
    pub const NEW_AGENT_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("new_agent_execution_config"),
    };
    /// Build authenticated verifier request evidence from workflow execution and leader cap.
    ///
    /// `nexus_workflow::dag::new_authenticated_offchain_request_evidence_v1`
    pub const NEW_AUTHENTICATED_OFFCHAIN_REQUEST_EVIDENCE_V1: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "new_authenticated_offchain_request_evidence_v1",
            ),
        };
    /// Create a new DAG execution config value.
    ///
    /// `nexus_workflow::dag::new_dag_execution_config`
    pub const NEW_DAG_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("new_dag_execution_config"),
    };
    /// Serialize an on-chain tool-result submission envelope from DAG output arguments.
    ///
    /// `nexus_workflow::dag::on_chain_tool_result_submission_v1_bytes`
    pub const ON_CHAIN_TOOL_RESULT_SUBMISSION_V1_BYTES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("on_chain_tool_result_submission_v1_bytes"),
    };
    /// The OutputPort struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::OutputPort`
    pub const OUTPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("OutputPort"),
    };
    /// Create an OutputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::output_port_from_string`
    pub const OUTPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("output_port_from_string"),
    };
    /// The OutputVariant struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::OutputVariant`
    pub const OUTPUT_VARIANT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("OutputVariant"),
    };
    /// Create an OutputVariant from an ASCII string.
    ///
    /// `nexus_workflow::dag::output_variant_from_string`
    pub const OUTPUT_VARIANT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("output_variant_from_string"),
    };
    /// Create a terminate post-failure action.
    ///
    /// `nexus_workflow::dag::post_failure_action_terminate`
    pub const POST_FAILURE_ACTION_TERMINATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("post_failure_action_terminate"),
    };
    /// Create a transient-continue post-failure action.
    ///
    /// `nexus_workflow::dag::post_failure_action_transient_continue`
    pub const POST_FAILURE_ACTION_TRANSIENT_CONTINUE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("post_failure_action_transient_continue"),
    };
    /// Prepare a registered TAP agent DAG execution using a scheduled occurrence payment.
    ///
    /// `nexus_workflow::dag::prepare_agent_execution_from_scheduled_payment`
    pub const PREPARE_AGENT_EXECUTION_FROM_SCHEDULED_PAYMENT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "prepare_agent_execution_from_scheduled_payment",
            ),
        };
    /// Scheduler entry point to invoke the default agent execution route
    /// using a durable TAP scheduled payment reserve.
    ///
    /// `nexus_workflow::dag::prepare_default_agent_execution_from_scheduled_payment`
    pub const PREPARE_DEFAULT_AGENT_EXECUTION_FROM_SCHEDULED_PAYMENT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "prepare_default_agent_execution_from_scheduled_payment",
            ),
        };
    /// Scheduler entry point to invoke the default agent execution route.
    ///
    /// `nexus_workflow::dag::prepare_default_agent_execution_from_scheduler`
    pub const PREPARE_DEFAULT_AGENT_EXECUTION_FROM_SCHEDULER: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "prepare_default_agent_execution_from_scheduler",
            ),
        };
    /// Stamp the worksheet with the execution ID before onchain tool execution.
    ///
    /// `nexus_workflow::dag::pre_stamp_execution`
    pub const PRE_STAMP_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("pre_stamp_execution"),
    };
    /// Stamp TAP execution context before tool execution.
    ///
    /// `nexus_workflow::dag::pre_stamp_tap_execution`
    pub const PRE_STAMP_TAP_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("pre_stamp_tap_execution"),
    };
    /// Refund a scheduled agent-vault-funded TAP payment owned by DAGExecution.
    ///
    /// `nexus_workflow::dag::refund_scheduled_tap_execution_payment_from_agent_vault`
    pub const REFUND_SCHEDULED_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "refund_scheduled_tap_execution_payment_from_agent_vault",
            ),
        };
    /// Refund an invoker-funded TAP payment owned by DAGExecution.
    ///
    /// `nexus_workflow::dag::refund_tap_execution_payment`
    pub const REFUND_TAP_EXECUTION_PAYMENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("refund_tap_execution_payment"),
    };
    /// Refund an agent-vault-funded TAP payment owned by DAGExecution.
    ///
    /// `nexus_workflow::dag::refund_tap_execution_payment_from_agent_vault`
    pub const REFUND_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "refund_tap_execution_payment_from_agent_vault",
            ),
        };
    /// Register a scheduler execution policy config for a registered TAP agent execution.
    ///
    /// `nexus_workflow::dag::register_begin_agent_execution`
    pub const REGISTER_BEGIN_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("register_begin_agent_execution"),
    };
    /// Register scheduler execution config for default agent execution.
    ///
    /// `nexus_workflow::dag::register_begin_default_agent_execution`
    pub const REGISTER_BEGIN_DEFAULT_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("register_begin_default_agent_execution"),
    };
    /// Function to call to continue to the next vertex in the given walk.
    ///
    /// `nexus_workflow::dag::request_network_to_execute_walks`
    pub const REQUEST_NETWORK_TO_EXECUTE_WALKS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("request_network_to_execute_walks"),
    };
    /// Returns a new hot potato object RequestWalkExecution.
    ///
    /// `nexus_workflow::dag::request_walk_execution`
    pub const REQUEST_WALK_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("request_walk_execution"),
    };
    /// Returns a new hot potato object RequestWalkExecution for one active walk.
    ///
    /// `nexus_workflow::dag::request_walk_execution_for_walk`
    pub const REQUEST_WALK_EXECUTION_FOR_WALK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("request_walk_execution_for_walk"),
    };
    /// Create a `RuntimeVertex::Plain` from a string.
    ///
    /// `nexus_workflow::dag::runtime_vertex_plain_from_string`
    pub const RUNTIME_VERTEX_PLAIN_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("runtime_vertex_plain_from_string"),
    };
    /// Create a `RuntimeVertex::WithIterator` from a string.
    ///
    /// `nexus_workflow::dag::runtime_vertex_with_iterator_from_string`
    pub const RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("runtime_vertex_with_iterator_from_string"),
    };
    /// Canonical off-chain tool-result submission entrypoint.
    ///
    /// `nexus_workflow::dag::submit_off_chain_tool_result_for_walk_v1`
    pub const SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_V1: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("submit_off_chain_tool_result_for_walk_v1"),
    };
    /// Canonical no-verifier off-chain tool-result submission entrypoint.
    ///
    /// `nexus_workflow::dag::submit_off_chain_tool_result_for_walk_without_verifier_v1`
    pub const SUBMIT_OFF_CHAIN_TOOL_RESULT_FOR_WALK_WITHOUT_VERIFIER_V1: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: DAG_MODULE,
            name: sui::types::Identifier::from_static(
                "submit_off_chain_tool_result_for_walk_without_verifier_v1",
            ),
        };
    /// Canonical on-chain tool-result submission entrypoint.
    ///
    /// `nexus_workflow::dag::submit_on_chain_tool_result_for_walk_v1`
    pub const SUBMIT_ON_CHAIN_TOOL_RESULT_FOR_WALK_V1: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("submit_on_chain_tool_result_for_walk_v1"),
    };
    /// The TAP authorization grant reference struct type.
    ///
    /// `nexus_workflow::dag::TapAuthorizationGrantRef`
    pub const TAP_AUTHORIZATION_GRANT_REF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("TapAuthorizationGrantRef"),
    };
    /// Create a TAP authorization grant reference value.
    ///
    /// `nexus_workflow::dag::tap_authorization_grant_ref`
    pub const TAP_AUTHORIZATION_GRANT_REF_CONSTRUCTOR: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("tap_authorization_grant_ref"),
    };
    /// Convert TaggedOutput to DAG types.
    ///
    /// `nexus_workflow::dag::tagged_output_to_dag_types`
    pub const TOOL_OUTPUT_TO_DAG_TYPES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("tagged_output_to_dag_types"),
    };
    /// Create a verifier config.
    ///
    /// `nexus_workflow::dag::verifier_config`
    pub const VERIFIER_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("verifier_config"),
    };
    /// Create the authenticated communication verifier mode.
    ///
    /// `nexus_workflow::dag::verifier_mode_authenticated_communication`
    pub const VERIFIER_MODE_AUTHENTICATED_COMMUNICATION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("verifier_mode_authenticated_communication"),
    };
    /// Create the `VerifierMode::None` variant.
    ///
    /// `nexus_workflow::dag::verifier_mode_none`
    pub const VERIFIER_MODE_NONE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("verifier_mode_none"),
    };
    /// Create the `VerifierMode::ToolVerifierContract` variant.
    ///
    /// `nexus_workflow::dag::verifier_mode_tool_verifier_contract`
    pub const VERIFIER_MODE_TOOL_VERIFIER_CONTRACT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("verifier_mode_tool_verifier_contract"),
    };
    /// The Vertex struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::Vertex`
    pub const VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("Vertex"),
    };
    /// Create a Vertex from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_from_string`
    pub const VERTEX_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("vertex_from_string"),
    };
    /// Create a new off-chain NodeIdent from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_off_chain`
    pub const VERTEX_OFF_CHAIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("vertex_off_chain"),
    };
    /// Create a new onchain NodeIdent from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_on_chain`
    pub const VERTEX_ON_CHAIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("vertex_on_chain"),
    };
    /// Configure the DAG-wide default leader verifier policy.
    ///
    /// `nexus_workflow::dag::with_default_leader_verifier`
    pub const WITH_DEFAULT_LEADER_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_default_leader_verifier"),
    };
    /// Configure the DAG-wide default tool verifier policy.
    ///
    /// `nexus_workflow::dag::with_default_tool_verifier`
    pub const WITH_DEFAULT_TOOL_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_default_tool_verifier"),
    };
    /// Add a default value to a DAG. Default value is a Vertex + InputPort pair
    /// with NexusData as the value.
    ///
    /// `nexus_workflow::dag::with_default_value`
    pub const WITH_DEFAULT_VALUE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_default_value"),
    };
    /// Add an Edge to a DAG.
    ///
    /// `nexus_workflow::dag::with_edge`
    pub const WITH_EDGE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_edge"),
    };
    /// Mark a vertex as an entry vertex and assign it to a group.
    ///
    /// `nexus_workflow::dag::with_entry_in_group`
    pub const WITH_ENTRY_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_in_group"),
    };
    /// Add an input port as an entry input port and assign it to a group.
    ///
    /// `nexus_workflow::dag::with_entry_port_in_group`
    pub const WITH_ENTRY_PORT_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_port_in_group"),
    };
    /// Add an entry vertex to a DAG. Entry vertex is just a Vertex with its
    /// required InputPorts specified.
    ///
    /// `nexus_workflow::dag::with_entry_vertex`
    pub const WITH_ENTRY_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_vertex"),
    };
    /// Add an output to a DAG.
    ///
    /// `nexus_workflow::dag::with_output`
    pub const WITH_OUTPUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_output"),
    };
    /// Configure a DAG-level default post-failure action.
    ///
    /// `nexus_workflow::dag::with_post_failure_action`
    pub const WITH_POST_FAILURE_ACTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_post_failure_action"),
    };
    /// Add a Vertex to a DAG.
    ///
    /// `nexus_workflow::dag::with_vertex`
    pub const WITH_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex"),
    };
    /// Configure the vertex-level leader verifier policy.
    ///
    /// `nexus_workflow::dag::with_vertex_leader_verifier`
    pub const WITH_VERTEX_LEADER_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_leader_verifier"),
    };
    /// Configure a vertex-level post-failure action override.
    ///
    /// `nexus_workflow::dag::with_vertex_post_failure_action`
    pub const WITH_VERTEX_POST_FAILURE_ACTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_post_failure_action"),
    };
    /// Configure the vertex-level tool verifier policy.
    ///
    /// `nexus_workflow::dag::with_vertex_tool_verifier`
    pub const WITH_VERTEX_TOOL_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_tool_verifier"),
    };

    /// Create an EntryGroup from a string.
    pub fn entry_group_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::ENTRY_GROUP_FROM_STRING.module,
                Self::ENTRY_GROUP_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an InputPort from a string.
    pub fn input_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::INPUT_PORT_FROM_STRING.module,
                Self::INPUT_PORT_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an OutputPort from a string.
    pub fn output_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::OUTPUT_PORT_FROM_STRING.module,
                Self::OUTPUT_PORT_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an OutputVariant from a string.
    pub fn output_variant_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::OUTPUT_VARIANT_FROM_STRING.module,
                Self::OUTPUT_VARIANT_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create a Vertex from a string.
    pub fn vertex_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_FROM_STRING.module,
                Self::VERTEX_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create a new off-chain NodeIdent from a string.
    pub fn off_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_OFF_CHAIN.module,
                Self::VERTEX_OFF_CHAIN.name,
                vec![],
            ),
            vec![str],
        ))
    }

    pub fn on_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_ON_CHAIN.module,
                Self::VERTEX_ON_CHAIN.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an edge kind from an enum variant.
    pub fn edge_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        edge_kind: &EdgeKind,
    ) -> sui::types::Argument {
        let ident = match edge_kind {
            EdgeKind::Normal => Self::EDGE_KIND_NORMAL,
            EdgeKind::ForEach => Self::EDGE_KIND_FOR_EACH,
            EdgeKind::Collect => Self::EDGE_KIND_COLLECT,
            EdgeKind::DoWhile => Self::EDGE_KIND_DO_WHILE,
            EdgeKind::Break => Self::EDGE_KIND_BREAK,
            EdgeKind::Static => Self::EDGE_KIND_STATIC,
        };

        tx.move_call(
            sui::tx::Function::new(workflow_pkg_id, ident.module, ident.name, vec![]),
            vec![],
        )
    }

    /// Create a post-failure action from an enum variant.
    pub fn post_failure_action_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        action: &PostFailureAction,
    ) -> sui::types::Argument {
        let ident = match action {
            PostFailureAction::Terminate => Self::POST_FAILURE_ACTION_TERMINATE,
            PostFailureAction::TransientContinue => Self::POST_FAILURE_ACTION_TRANSIENT_CONTINUE,
        };

        tx.move_call(
            sui::tx::Function::new(workflow_pkg_id, ident.module, ident.name, vec![]),
            vec![],
        )
    }

    /// Create a failure evidence kind from an enum variant.
    pub fn failure_evidence_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        evidence_kind: &FailureEvidenceKind,
    ) -> sui::types::Argument {
        let ident = match evidence_kind {
            FailureEvidenceKind::ToolEvidence => Self::FAILURE_EVIDENCE_KIND_TOOL_EVIDENCE,
            FailureEvidenceKind::LeaderEvidence => Self::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE,
        };

        tx.move_call(
            sui::tx::Function::new(workflow_pkg_id, ident.module, ident.name, vec![]),
            vec![],
        )
    }

    /// Create a verifier mode from an enum variant.
    pub fn verifier_mode_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        mode: &VerifierMode,
    ) -> sui::types::Argument {
        let ident = match mode {
            VerifierMode::None => Self::VERIFIER_MODE_NONE,
            VerifierMode::LeaderRegisteredKey | VerifierMode::LeaderNautilusEnclave => {
                Self::VERIFIER_MODE_AUTHENTICATED_COMMUNICATION
            }
            VerifierMode::ToolVerifierContract => Self::VERIFIER_MODE_TOOL_VERIFIER_CONTRACT,
        };

        tx.move_call(
            sui::tx::Function::new(workflow_pkg_id, ident.module, ident.name, vec![]),
            vec![],
        )
    }

    /// Create a verifier config value from the Rust mirror.
    pub fn verifier_config(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        config: &VerifierConfig,
    ) -> anyhow::Result<sui::types::Argument> {
        let mode = Self::verifier_mode_from_enum(tx, workflow_pkg_id, &config.mode);
        let method = super::move_std::Ascii::ascii_string_from_str(tx, &config.method)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERIFIER_CONFIG.module,
                Self::VERIFIER_CONFIG.name,
                vec![],
            ),
            vec![mode, method],
        ))
    }

    /// Create a runtime vertex from an enum variant
    pub fn runtime_vertex_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        runtime_vertex: &RuntimeVertex,
    ) -> anyhow::Result<sui::types::Argument> {
        match runtime_vertex {
            RuntimeVertex::Plain { vertex } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;

                Ok(tx.move_call(
                    sui::tx::Function::new(
                        workflow_pkg_id,
                        Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.module,
                        Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.name,
                        vec![],
                    ),
                    vec![name],
                ))
            }
            RuntimeVertex::WithIterator {
                vertex,
                iteration,
                out_of,
            } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;

                let iteration = tx.input(pure_arg(iteration)?);
                let out_of = tx.input(pure_arg(out_of)?);

                Ok(tx.move_call(
                    sui::tx::Function::new(
                        workflow_pkg_id,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.module,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.name,
                        vec![],
                    ),
                    vec![name, iteration, out_of],
                ))
            }
        }
    }
}

// == `nexus_workflow::tool_registry` ==

pub struct ToolRegistry;

const TOOL_REGISTRY_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("tool_registry");

impl ToolRegistry {
    /// Claim collateral for a tool. The function call returns Balance<SUI>.
    ///
    /// `nexus_workflow::tool_registry::claim_collateral`
    pub const CLAIM_COLLATERAL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("claim_collateral"),
    };
    /// Claim collateral for a tool and transfer the balance to the tx sender.
    ///
    /// `nexus_workflow::tool_registry::claim_collateral_for_self`
    pub const CLAIM_COLLATERAL_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("claim_collateral_for_self"),
    };
    /// OverSlashing struct type. Used to fetch caps for slashing tools.
    ///
    /// `nexus_workflow::tool_registry::OverSlashing`
    pub const OVER_SLASHING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("OverSlashing"),
    };
    /// OverTool struct type. Used for fetching tool owner caps.
    ///
    /// `nexus_workflow::tool_registry::OverTool`
    pub const OVER_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("OverTool"),
    };
    /// Register an off-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_workflow::tool_registry::register_off_chain_tool`
    pub const REGISTER_OFF_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_off_chain_tool"),
    };
    /// Register an on-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_workflow::tool_registry::register_on_chain_tool`
    pub const REGISTER_ON_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_on_chain_tool"),
    };
    /// Register a cap-gated on-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_workflow::tool_registry::register_on_chain_tool_with_workflow_authorization_cap`
    pub const REGISTER_ON_CHAIN_TOOL_WITH_WORKFLOW_AUTHORIZATION_CAP: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: TOOL_REGISTRY_MODULE,
            name: sui::types::Identifier::from_static(
                "register_on_chain_tool_with_workflow_authorization_cap",
            ),
        };
    /// Tool struct type. Used for fetching tool info.
    ///
    /// `nexus_workflow::tool_registry::Tool`
    pub const TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("Tool"),
    };
    /// The ToolRegistry struct type.
    ///
    /// `nexus_workflow::tool_registry::ToolRegistry`
    pub const TOOL_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("ToolRegistry"),
    };
    /// Unregister a tool.
    ///
    /// `nexus_workflow::tool_registry::unregister`
    pub const UNREGISTER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("unregister"),
    };
    /// Update a tool's timeout.
    ///
    /// `nexus_workflow::tool_registry::update_tool_timeout`
    pub const UPDATE_TOOL_TIMEOUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("update_tool_timeout"),
    };
}

// == `nexus_workflow::leader_cap` ==

pub struct LeaderCap;

const LEADER_CAP_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("leader_cap");

impl LeaderCap {
    /// Create N leader caps for self and the provided addresses.
    ///
    /// `nexus_workflow::leader_cap::create_for_self_and_addresses`
    pub const CREATE_FOR_SELF_AND_ADDRESSES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::types::Identifier::from_static("create_for_self_and_addresses"),
    };
    /// This is used as a generic argument for
    /// [crate::idents::primitives::OwnerCap::CLONEABLE_OWNER_CAP].
    ///
    /// `nexus_workflow::leader_cap::OverNetwork`
    pub const OVER_NETWORK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::types::Identifier::from_static("OverNetwork"),
    };
}

// == `nexus_workflow::gas` ==

pub struct Gas;

const GAS_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("gas");

impl Gas {
    /// Derive a `ToolGas` object while setting the initial invocation price.
    ///
    /// `nexus_workflow::gas::create_tool_gas`
    pub const CREATE_TOOL_GAS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("create_tool_gas"),
    };
    /// Same as `CREATE_TOOL_GAS` but object is shared.
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

// == `nexus_workflow::leader` ==

pub struct Leader;

const LEADER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("leader");

impl Leader {
    /// Activate a leader and claim ownership of its `Active` state with a fresh token.
    ///
    /// `nexus_workflow::leader::activate_and_claim`
    pub const ACTIVATE_AND_CLAIM: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("activate_and_claim"),
    };
    /// Allow an address to request leader capabilities.
    ///
    /// `nexus_workflow::leader::allow_address`
    pub const ALLOW_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("allow_address"),
    };
    /// Disallow an address from requesting leader capabilities.
    ///
    /// `nexus_workflow::leader::disallow_address`
    pub const DISALLOW_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("disallow_address"),
    };
    /// Create empty metadata for a leader.
    ///
    /// `nexus_workflow::leader::empty_metadata`
    pub const EMPTY_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("empty_metadata"),
    };
    /// Admin capability type for modifying leader allowlist.
    ///
    /// `nexus_workflow::leader::LeaderCapabilitiesAdminCap`
    pub const LEADER_CAPABILITIES_ADMIN_CAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("LeaderCapabilitiesAdminCap"),
    };
    /// LeaderRegistry type for lookups.
    ///
    /// `nexus_workflow::leader::LeaderRegistry`
    pub const LEADER_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("LeaderRegistry"),
    };
    /// Create metadata with the provided map.
    ///
    /// `nexus_workflow::leader::new_metadata`
    pub const NEW_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("new_metadata"),
    };
    /// Register the caller as a leader and stake.
    ///
    /// `nexus_workflow::leader::register`
    pub const REGISTER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("register"),
    };
    /// Stake SUI into a leader's pool.
    ///
    /// `nexus_workflow::leader::stake`
    pub const STAKE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("stake"),
    };
    /// Create `LeaderStatus::ACTIVE`.
    ///
    /// `nexus_workflow::leader::status_active`
    pub const STATUS_ACTIVE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("status_active"),
    };
    /// Create `LeaderStatus::SLASHED`.
    ///
    /// `nexus_workflow::leader::status_slashed`
    pub const STATUS_SLASHED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("status_slashed"),
    };
    /// Create `LeaderStatus::SUSPENDED`.
    ///
    /// `nexus_workflow::leader::status_suspended`
    pub const STATUS_SUSPENDED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("status_suspended"),
    };
    /// Suspend a leader only if the caller still holds the on-record claim token.
    ///
    /// `nexus_workflow::leader::suspend_if_token`
    pub const SUSPEND_IF_TOKEN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_MODULE,
        name: sui::types::Identifier::from_static("suspend_if_token"),
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
