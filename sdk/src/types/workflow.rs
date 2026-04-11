use {
    super::nexus_data::NexusData,
    crate::{
        sui,
        types::{parse_published_move_enum_value, parse_string_value, strip_fields_owned},
    },
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureEvidenceKind {
    #[serde(alias = "ToolEvidence")]
    ToolEvidence,
    #[serde(alias = "LeaderEvidence")]
    LeaderEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationSubmissionKind {
    #[serde(rename = "success", alias = "Success")]
    Success,
    #[serde(rename = "err_eval", alias = "ErrEval")]
    ErrEval,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationVerdict {
    #[serde(rename = "accepted", alias = "Accepted")]
    Accepted,
    #[serde(rename = "invalid_leader_proof", alias = "InvalidLeaderProof")]
    InvalidLeaderProof,
    #[serde(rename = "invalid_tool_proof", alias = "InvalidToolProof")]
    InvalidToolProof,
    #[serde(rename = "policy_bypass_allowed", alias = "PolicyBypassAllowed")]
    PolicyBypassAllowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerificationSubmissionRole {
    #[serde(rename = "leader", alias = "Leader")]
    Leader,
    #[serde(rename = "tool", alias = "Tool")]
    Tool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerifierDecisionV1 {
    #[serde(rename = "accept", alias = "Accept")]
    Accept,
    #[serde(rename = "reject", alias = "Reject")]
    Reject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VerifierMode {
    #[serde(rename = "none", alias = "None")]
    None,
    #[serde(rename = "leader_registered_key", alias = "LeaderRegisteredKey")]
    LeaderRegisteredKey,
    #[serde(rename = "leader_nautilus_enclave", alias = "LeaderNautilusEnclave")]
    LeaderNautilusEnclave,
    #[serde(rename = "tool_verifier_contract", alias = "ToolVerifierContract")]
    ToolVerifierContract,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct VerifierConfig {
    pub mode: VerifierMode,
    pub method: String,
}

impl VerifierConfig {
    pub fn is_none(&self) -> bool {
        self.mode == VerifierMode::None && self.method.is_empty()
    }
}

impl Default for VerifierConfig {
    fn default() -> Self {
        Self {
            mode: VerifierMode::None,
            method: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierContractResultV1 {
    pub method: String,
    pub decision: VerifierDecisionV1,
    pub submission_kind: VerificationSubmissionKind,
    pub failure_evidence_kind: FailureEvidenceKind,
    pub payload_or_reason_hash: Vec<u8>,
    pub credential: Vec<u8>,
    pub detail: Vec<u8>,
}

impl VerifierContractResultV1 {
    pub fn to_bcs_bytes(&self) -> bcs::Result<Vec<u8>> {
        bcs::to_bytes(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalVerifierSubmitEvidenceV1 {
    pub result: VerifierContractResultV1,
    pub communication_evidence: Vec<u8>,
}

impl ExternalVerifierSubmitEvidenceV1 {
    pub fn to_bcs_bytes(&self) -> bcs::Result<Vec<u8>> {
        bcs::to_bytes(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalVerifierSubmitEvidenceV2 {
    pub result: VerifierContractResultV1,
    pub communication_evidence: Vec<u8>,
}

impl ExternalVerifierSubmitEvidenceV2 {
    pub fn to_bcs_bytes(&self) -> bcs::Result<Vec<u8>> {
        bcs::to_bytes(self)
    }
}

impl From<ExternalVerifierSubmitEvidenceV1> for ExternalVerifierSubmitEvidenceV2 {
    fn from(value: ExternalVerifierSubmitEvidenceV1) -> Self {
        Self {
            result: value.result,
            communication_evidence: value.communication_evidence,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedToolOutputPortV1 {
    pub port: String,
    pub data: NexusData,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedToolOutputV1 {
    pub output_variant: String,
    pub output_ports_data: Vec<PreparedToolOutputPortV1>,
}

impl PreparedToolOutputV1 {
    pub fn to_bcs_bytes(&self) -> bcs::Result<Vec<u8>> {
        bcs::to_bytes(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffChainSubmissionProofV1 {
    None,
    RegisteredKey {
        verifier_credential: Vec<u8>,
        communication_evidence: Vec<u8>,
    },
    ExternalVerifier {
        evidence: ExternalVerifierSubmitEvidenceV2,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OffChainToolResultAuxiliaryV1 {
    pub reported_failure_evidence_kind: Option<FailureEvidenceKind>,
    pub proof: OffChainSubmissionProofV1,
}

impl OffChainToolResultAuxiliaryV1 {
    pub fn to_bcs_bytes(&self) -> bcs::Result<Vec<u8>> {
        bcs::to_bytes(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnChainToolResultSubmissionV1 {
    pub observed_output: PreparedToolOutputV1,
    pub raw_failure_evidence_kind: Option<FailureEvidenceKind>,
    pub submitted_failure_reason: Option<Vec<u8>>,
    pub tool_witness_id: sui::types::Address,
}

impl OnChainToolResultSubmissionV1 {
    pub fn to_bcs_bytes(&self) -> bcs::Result<Vec<u8>> {
        bcs::to_bytes(self)
    }
}

impl<'de> Deserialize<'de> for VerifierConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Standard {
            mode: VerifierMode,
            method: String,
        }

        if !deserializer.is_human_readable() {
            let parsed = Standard::deserialize(deserializer)?;
            return Ok(Self {
                mode: parsed.mode,
                method: parsed.method,
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        if let Ok(parsed) = serde_json::from_value::<Standard>(value.clone()) {
            return Ok(Self {
                mode: parsed.mode,
                method: parsed.method,
            });
        }

        let value = strip_fields_owned(value);
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("VerifierConfig must be an object"))?;

        let mode_value = object
            .get("mode")
            .ok_or_else(|| serde::de::Error::custom("VerifierConfig missing mode"))?;
        let mode = parse_published_move_enum_value::<VerifierMode>(mode_value)
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| serde::de::Error::custom("VerifierConfig mode did not parse"))?;

        let method_value = object
            .get("method")
            .ok_or_else(|| serde::de::Error::custom("VerifierConfig missing method"))?;
        let method = parse_string_value(method_value)
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| serde::de::Error::custom("VerifierConfig method did not parse"))?;

        Ok(Self { mode, method })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkflowFailureClass {
    #[serde(rename = "retryable", alias = "Retryable")]
    Retryable,
    #[serde(rename = "terminal_tool_failure", alias = "TerminalToolFailure")]
    TerminalToolFailure,
    #[serde(
        rename = "terminal_submission_failure",
        alias = "TerminalSubmissionFailure"
    )]
    TerminalSubmissionFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTerminalRecord {
    pub vertex: crate::types::RuntimeVertex,
    pub failure_class: WorkflowFailureClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PostFailureAction {
    #[serde(rename = "terminate", alias = "Terminate")]
    Terminate,
    #[serde(
        rename = "continue",
        alias = "Continue",
        alias = "TransientContinue",
        alias = "transient_continue"
    )]
    TransientContinue,
}

impl PostFailureAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminate => "terminate",
            Self::TransientContinue => "continue",
        }
    }
}

impl std::fmt::Display for PostFailureAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Display for WorkflowFailureClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Retryable => "retryable",
            Self::TerminalToolFailure => "terminal_tool_failure",
            Self::TerminalSubmissionFailure => "terminal_submission_failure",
        };

        f.write_str(value)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::PublishedMoveEnum};

    #[test]
    fn test_post_failure_action_serde() {
        let action: PostFailureAction = serde_json::from_str("\"continue\"").unwrap();
        assert_eq!(action, PostFailureAction::TransientContinue);
        assert_eq!(
            serde_json::to_string(&PostFailureAction::Terminate).unwrap(),
            "\"terminate\""
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
    fn test_workflow_failure_class_published_move_enum_serde() {
        let wrapped: PublishedMoveEnum<WorkflowFailureClass> =
            serde_json::from_str("{\"_variant_name\":\"TerminalToolFailure\"}").unwrap();
        assert_eq!(wrapped.0, WorkflowFailureClass::TerminalToolFailure);

        let string: PublishedMoveEnum<WorkflowFailureClass> =
            serde_json::from_str("\"terminal_submission_failure\"").unwrap();
        assert_eq!(string.0, WorkflowFailureClass::TerminalSubmissionFailure);
    }

    #[test]
    fn test_post_failure_action_published_move_enum_serde() {
        let tagged: PublishedMoveEnum<PostFailureAction> =
            serde_json::from_str("{\"@variant\":\"TransientContinue\"}").unwrap();
        assert_eq!(tagged.0, PostFailureAction::TransientContinue);

        let string: PublishedMoveEnum<PostFailureAction> =
            serde_json::from_str("\"Terminate\"").unwrap();
        assert_eq!(string.0, PostFailureAction::Terminate);
    }

    #[test]
    fn test_verifier_mode_published_move_enum_serde() {
        let tagged: PublishedMoveEnum<VerifierMode> =
            serde_json::from_str("{\"@variant\":\"LeaderRegisteredKey\"}").unwrap();
        assert_eq!(tagged.0, VerifierMode::LeaderRegisteredKey);

        let string: PublishedMoveEnum<VerifierMode> =
            serde_json::from_str("\"tool_verifier_contract\"").unwrap();
        assert_eq!(string.0, VerifierMode::ToolVerifierContract);
    }

    #[test]
    fn test_verifier_config_deserializes_wrapped_move_json() {
        let parsed: VerifierConfig = serde_json::from_value(serde_json::json!({
            "fields": {
                "mode": {
                    "fields": {
                        "@variant": "LeaderRegisteredKey"
                    }
                },
                "method": {
                    "bytes": "signed_http_v1"
                }
            }
        }))
        .unwrap();

        assert_eq!(
            parsed,
            VerifierConfig {
                mode: VerifierMode::LeaderRegisteredKey,
                method: "signed_http_v1".to_string(),
            }
        );
    }

    #[test]
    fn test_verifier_config_deserializes_plain_json() {
        let parsed: VerifierConfig = serde_json::from_value(serde_json::json!({
            "mode": "tool_verifier_contract",
            "method": "demo_verifier_v1"
        }))
        .unwrap();

        assert_eq!(
            parsed,
            VerifierConfig {
                mode: VerifierMode::ToolVerifierContract,
                method: "demo_verifier_v1".to_string(),
            }
        );
    }

    #[test]
    fn test_verification_verdict_published_move_enum_serde() {
        let tagged: PublishedMoveEnum<VerificationVerdict> =
            serde_json::from_str("{\"_variant_name\":\"InvalidLeaderProof\"}").unwrap();
        assert_eq!(tagged.0, VerificationVerdict::InvalidLeaderProof);

        let accepted_tagged: PublishedMoveEnum<VerificationVerdict> =
            serde_json::from_str("{\"_variant_name\":\"Accepted\"}").unwrap();
        assert_eq!(accepted_tagged.0, VerificationVerdict::Accepted);

        let accepted_string: PublishedMoveEnum<VerificationVerdict> =
            serde_json::from_str("\"accepted\"").unwrap();
        assert_eq!(accepted_string.0, VerificationVerdict::Accepted);

        let invalid_tool_proof_tagged: PublishedMoveEnum<VerificationVerdict> =
            serde_json::from_str("{\"@variant\":\"InvalidToolProof\"}").unwrap();
        assert_eq!(
            invalid_tool_proof_tagged.0,
            VerificationVerdict::InvalidToolProof
        );

        let invalid_tool_proof_string: PublishedMoveEnum<VerificationVerdict> =
            serde_json::from_str("\"invalid_tool_proof\"").unwrap();
        assert_eq!(
            invalid_tool_proof_string.0,
            VerificationVerdict::InvalidToolProof
        );

        let policy_bypass_allowed_tagged: PublishedMoveEnum<VerificationVerdict> =
            serde_json::from_str("{\"@variant\":\"PolicyBypassAllowed\"}").unwrap();
        assert_eq!(
            policy_bypass_allowed_tagged.0,
            VerificationVerdict::PolicyBypassAllowed
        );

        let policy_bypass_allowed_string: PublishedMoveEnum<VerificationVerdict> =
            serde_json::from_str("\"policy_bypass_allowed\"").unwrap();
        assert_eq!(
            policy_bypass_allowed_string.0,
            VerificationVerdict::PolicyBypassAllowed
        );
    }

    #[test]
    fn test_failure_evidence_kind_published_move_enum_serde() {
        let tagged: PublishedMoveEnum<FailureEvidenceKind> =
            serde_json::from_str("{\"_variant_name\":\"ToolEvidence\"}").unwrap();
        assert_eq!(tagged.0, FailureEvidenceKind::ToolEvidence);

        let string: PublishedMoveEnum<FailureEvidenceKind> =
            serde_json::from_str("\"leader_evidence\"").unwrap();
        assert_eq!(string.0, FailureEvidenceKind::LeaderEvidence);
    }

    #[test]
    fn test_verification_verdict_serde() {
        let accepted: VerificationVerdict = serde_json::from_str("\"Accepted\"").unwrap();
        assert_eq!(accepted, VerificationVerdict::Accepted);
        assert_eq!(
            serde_json::to_string(&VerificationVerdict::Accepted).unwrap(),
            "\"accepted\""
        );

        let invalid_tool_proof: VerificationVerdict =
            serde_json::from_str("\"InvalidToolProof\"").unwrap();
        assert_eq!(invalid_tool_proof, VerificationVerdict::InvalidToolProof);
        assert_eq!(
            serde_json::to_string(&VerificationVerdict::InvalidToolProof).unwrap(),
            "\"invalid_tool_proof\""
        );

        let policy_bypass_allowed: VerificationVerdict =
            serde_json::from_str("\"PolicyBypassAllowed\"").unwrap();
        assert_eq!(
            policy_bypass_allowed,
            VerificationVerdict::PolicyBypassAllowed
        );
        assert_eq!(
            serde_json::to_string(&VerificationVerdict::PolicyBypassAllowed).unwrap(),
            "\"policy_bypass_allowed\""
        );
    }

    #[test]
    fn test_verifier_decision_v1_serde() {
        let accepted: VerifierDecisionV1 = serde_json::from_str("\"Accept\"").unwrap();
        assert_eq!(accepted, VerifierDecisionV1::Accept);
        assert_eq!(
            serde_json::to_string(&VerifierDecisionV1::Reject).unwrap(),
            "\"reject\""
        );
    }

    #[test]
    fn test_external_verifier_submit_evidence_v1_bcs_round_trip() {
        let value = ExternalVerifierSubmitEvidenceV1 {
            result: VerifierContractResultV1 {
                method: "demo_verifier_v1".to_string(),
                decision: VerifierDecisionV1::Accept,
                submission_kind: VerificationSubmissionKind::Success,
                failure_evidence_kind: FailureEvidenceKind::ToolEvidence,
                payload_or_reason_hash: vec![1, 2, 3],
                credential: vec![4, 5],
                detail: vec![6, 7],
            },
            communication_evidence: vec![8, 9, 10],
        };

        let bytes = value.to_bcs_bytes().unwrap();
        let parsed: ExternalVerifierSubmitEvidenceV1 = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, value);
    }

    #[test]
    fn test_tool_result_auxiliary_v1_bcs_serializes() {
        let value = OffChainToolResultAuxiliaryV1 {
            reported_failure_evidence_kind: Some(FailureEvidenceKind::ToolEvidence),
            proof: OffChainSubmissionProofV1::ExternalVerifier {
                evidence: ExternalVerifierSubmitEvidenceV2 {
                    result: VerifierContractResultV1 {
                        method: "demo_verifier_v1".to_string(),
                        decision: VerifierDecisionV1::Accept,
                        submission_kind: VerificationSubmissionKind::Success,
                        failure_evidence_kind: FailureEvidenceKind::ToolEvidence,
                        payload_or_reason_hash: vec![1, 2, 3],
                        credential: vec![4, 5],
                        detail: vec![6, 7],
                    },
                    communication_evidence: vec![8, 9, 10],
                },
            },
        };

        let bytes = value.to_bcs_bytes().unwrap();
        assert!(!bytes.is_empty());
    }
}
