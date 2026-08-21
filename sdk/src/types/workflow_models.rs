use {
    crate::{
        move_bindings::{
            interface::{
                graph::RuntimeVertex,
                verifier::VerifierMethodId,
                version::InterfaceVersion,
            },
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
    pub task_id: sui::types::Address,
    pub occurrence_id: u64,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalVerifierRuntimeCall {
    pub method_id: VerifierMethodId,
    pub witness_id: sui::types::Address,
    pub immutable_shared_objects: Vec<sui::types::ObjectReference>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTerminalRecord {
    pub vertex: RuntimeVertex,
    pub failure_class: WorkflowFailureClass,
}

#[cfg(test)]
mod tests {
    use {super::*, crate::move_bindings::interface::graph::PostFailureAction};

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
}
