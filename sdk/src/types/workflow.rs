use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureEvidenceKind {
    #[serde(rename = "tool_evidence", alias = "ToolEvidence")]
    ToolEvidence,
    #[serde(rename = "leader_evidence", alias = "LeaderEvidence")]
    LeaderEvidence,
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
}
