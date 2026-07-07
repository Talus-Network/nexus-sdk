use {
    crate::{
        move_bindings::{
            interface::{
                graph::RuntimeVertex,
                verifier::{OffchainResponseEvidence, VerificationSubmissionKind},
                version::InterfaceVersion,
            },
            primitives::shared_object::SharedObjectRef,
            workflow::execution_failure::WorkflowFailureClass,
        },
        sui,
        types::{AgentId, SkillId, SkillRevisionLookupKey},
    },
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestWalkContext {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceVersion,
    pub scheduled_task_id: Option<sui::types::Address>,
    pub scheduled_occurrence_index: Option<u64>,
}

impl RequestWalkContext {
    pub fn skill_revision_key(&self) -> SkillRevisionLookupKey {
        SkillRevisionLookupKey {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }
}

/// Request-side metadata for active verifier submissions.
///
/// The active PTB derives execution, vertex, and leader identity from
/// authenticated Move objects instead of accepting those IDs from the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticatedOffchainRequestEvidence {
    pub walk_index: u64,
    pub tool_fqn: String,
    pub request_hash: Vec<u8>,
    pub request_signature: Vec<u8>,
}

/// Active verifier input consumed by SDK submit builders.
///
/// Move constructs the full verifier request evidence from `DAGExecution`,
/// `leader_cap`, and this request metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticatedOffchainVerifierEvidence {
    pub submission_kind: VerificationSubmissionKind,
    pub payload_or_reason_hash: Vec<u8>,
    pub transport_proof: Vec<u8>,
    pub request: AuthenticatedOffchainRequestEvidence,
    pub response: OffchainResponseEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalVerifierRuntimeCall {
    pub package_address: sui::types::Address,
    pub module_name: String,
    pub function_name: String,
    pub witness: sui::types::ObjectReference,
    pub shared_objects: Vec<(SharedObjectRef, sui::types::ObjectReference)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTerminalRecord {
    pub vertex: RuntimeVertex,
    pub failure_class: WorkflowFailureClass,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::move_bindings::{
            interface::{
                graph::PostFailureAction,
                verifier::{
                    ExternalVerifierSubmitEvidence,
                    FailureEvidenceKind,
                    OffChainToolResultAuxiliary,
                    OffChainVerifierProof,
                    OffchainRequestEvidence,
                    OffchainVerifierEvidence,
                    VerificationVerdict,
                    VerifierConfig,
                    VerifierContractResult,
                    VerifierDecision,
                    VerifierMode,
                },
            },
            move_std::option::Option as MoveOption,
        },
    };

    fn id(bytes: sui::types::Address) -> crate::move_bindings::sui_framework::object::ID {
        crate::move_bindings::sui_framework::object::ID::new(bytes)
    }

    #[test]
    fn test_post_failure_action_bcs_roundtrip() {
        let action = PostFailureAction::TransientContinue;
        assert_eq!(
            bcs::from_bytes::<PostFailureAction>(&bcs::to_bytes(&action).unwrap()).unwrap(),
            action
        );
    }

    #[test]
    fn test_post_failure_action_display() {
        assert_eq!(PostFailureAction::TransientContinue.to_string(), "continue");
        assert_eq!(PostFailureAction::Terminate.to_string(), "terminate");
    }

    #[test]
    fn test_workflow_failure_class_display() {
        assert_eq!(WorkflowFailureClass::Retryable.to_string(), "retryable");
        assert_eq!(
            WorkflowFailureClass::TerminalToolFailure.to_string(),
            "terminal_tool_failure"
        );
        assert_eq!(
            WorkflowFailureClass::TerminalSubmissionFailure.to_string(),
            "terminal_submission_failure"
        );
    }

    #[test]
    fn test_verifier_config_uses_generated_layout() {
        let value = VerifierConfig {
            mode: VerifierMode::ToolVerifierContract,
            method: "demo_verifier_v1".into(),
        };

        assert_eq!(
            bcs::from_bytes::<VerifierConfig>(&bcs::to_bytes(&value).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn test_move_option_bcs_roundtrips_published_move_enum_payload() {
        let value = MoveOption::from_option(Some(FailureEvidenceKind::ToolEvidence));
        assert_eq!(
            bcs::from_bytes::<MoveOption<FailureEvidenceKind>>(&bcs::to_bytes(&value).unwrap())
                .unwrap(),
            value
        );
    }

    #[test]
    fn test_verification_verdict_bcs_roundtrip() {
        for value in [
            VerificationVerdict::Accepted,
            VerificationVerdict::InvalidToolProof,
        ] {
            assert_eq!(
                bcs::from_bytes::<VerificationVerdict>(&bcs::to_bytes(&value).unwrap()).unwrap(),
                value
            );
        }
    }

    #[test]
    fn test_verifier_decision_v1_bcs_roundtrip() {
        for value in [VerifierDecision::Accept, VerifierDecision::Reject] {
            assert_eq!(
                bcs::from_bytes::<VerifierDecision>(&bcs::to_bytes(&value).unwrap()).unwrap(),
                value
            );
        }
    }

    #[test]
    fn test_external_verifier_submit_evidence_v1_bcs_round_trip() {
        let value = ExternalVerifierSubmitEvidence {
            result: VerifierContractResult {
                method: "demo_verifier_v1".into(),
                decision: VerifierDecision::Accept,
                submission_kind: VerificationSubmissionKind::Success,
                failure_evidence_kind: FailureEvidenceKind::ToolEvidence,
                payload_or_reason_hash: vec![1, 2, 3],
                credential: vec![4, 5],
                detail: vec![6, 7],
            },
            communication_evidence: vec![8, 9, 10],
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        let parsed: ExternalVerifierSubmitEvidence = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn test_offchain_verifier_evidence_v1_bcs_round_trip() {
        let value = OffchainVerifierEvidence {
            submission_kind: VerificationSubmissionKind::Success,
            payload_or_reason_hash: vec![1, 2, 3],
            transport_proof: vec![4, 5, 6],
            request: OffchainRequestEvidence {
                execution: id(sui::types::Address::ZERO),
                walk_index: 7,
                vertex: "verified".into(),
                tool_fqn: "example.tool@1".into(),
                leader_cap_id: id(sui::types::Address::ZERO),
                request_hash: vec![8, 9],
                request_signature: vec![10, 11],
            },
            response: OffchainResponseEvidence {
                status_code: 200,
                response_hash: vec![12, 13],
                response_signature: vec![14, 15],
                normalized_err_eval_reason_hash: MoveOption::from_option(Some(vec![16, 17])),
            },
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        let parsed: OffchainVerifierEvidence = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn test_authenticated_offchain_verifier_evidence_v1_bcs_round_trip() {
        let value = AuthenticatedOffchainVerifierEvidence {
            submission_kind: VerificationSubmissionKind::Success,
            payload_or_reason_hash: vec![1, 2, 3],
            transport_proof: vec![4, 5, 6],
            request: AuthenticatedOffchainRequestEvidence {
                walk_index: 7,
                tool_fqn: "example.tool@1".to_string(),
                request_hash: vec![8, 9],
                request_signature: vec![10, 11],
            },
            response: OffchainResponseEvidence {
                status_code: 200,
                response_hash: vec![12, 13],
                response_signature: vec![14, 15],
                normalized_err_eval_reason_hash: MoveOption::from_option(Some(vec![16, 17])),
            },
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        let parsed: AuthenticatedOffchainVerifierEvidence = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn test_off_chain_tool_result_auxiliary_success_bcs_serializes() {
        let value = OffChainToolResultAuxiliary {
            reported_failure_evidence_kind: MoveOption::from_option(None),
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        let parsed: OffChainToolResultAuxiliary = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(
            parsed.reported_failure_evidence_kind,
            MoveOption::from_option(None)
        );
    }

    #[test]
    fn test_off_chain_tool_result_auxiliary_err_eval_bcs_serializes() {
        let value = OffChainToolResultAuxiliary {
            reported_failure_evidence_kind: MoveOption::from_option(Some(
                FailureEvidenceKind::ToolEvidence,
            )),
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        let parsed: OffChainToolResultAuxiliary = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(
            parsed.reported_failure_evidence_kind,
            MoveOption::from_option(Some(FailureEvidenceKind::ToolEvidence))
        );
    }

    #[test]
    fn test_off_chain_verifier_proof_v1_bcs_serializes() {
        let value = OffChainVerifierProof::ExternalVerifier {
            evidence: ExternalVerifierSubmitEvidence {
                result: VerifierContractResult {
                    method: "demo_verifier_v1".into(),
                    decision: VerifierDecision::Accept,
                    submission_kind: VerificationSubmissionKind::Success,
                    failure_evidence_kind: FailureEvidenceKind::ToolEvidence,
                    payload_or_reason_hash: vec![1, 2, 3],
                    credential: vec![4, 5],
                    detail: vec![6, 7],
                },
                communication_evidence: vec![8, 9, 10],
            },
        };

        let bytes = bcs::to_bytes(&value).unwrap();
        let parsed: OffChainVerifierProof = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, value);
    }
}
