//! SDK conveniences for generated workflow and verifier bindings.
//!
//! Workflow code turns generated events and execution objects into SDK lookup keys, display
//! values, and timeout decisions. These helpers keep that translation local to generated types so
//! BCS layout and runtime behavior remain coupled to the Move ABI.

use crate::{
    move_bindings::{
        interface::graph::{PostFailureAction, RuntimeVertex},
        workflow::{
            execution::{DAGExecution, DAGWalk},
            execution_events::RequestWalkExecutionEvent,
            execution_failure::WorkflowFailureClass,
        },
    },
    sui,
    types::{RequestWalkContext, SkillRevisionLookupKey},
};

impl RequestWalkExecutionEvent {
    pub fn skill_revision_key(&self) -> Option<SkillRevisionLookupKey> {
        Some(SkillRevisionLookupKey {
            agent_id: self.agent_id.into(),
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
        })
    }

    pub fn to_context(&self) -> anyhow::Result<Option<RequestWalkContext>> {
        Ok(Some(RequestWalkContext {
            agent_id: self.agent_id.into(),
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
            scheduled_task_id: self.scheduled_task_id.as_option().map(|id| id.bytes),
            scheduled_occurrence_index: self.scheduled_occurrence_index.copied_option(),
        }))
    }
}

impl PostFailureAction {
    pub const fn as_str(&self) -> &'static str {
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

impl DAGExecution {
    pub fn dag_id(&self) -> sui::types::Address {
        self.dag.bytes
    }

    pub fn agent_id_address(&self) -> sui::types::Address {
        self.agent_id.bytes
    }

    pub fn scheduled_task_id_address(&self) -> Option<sui::types::Address> {
        self.scheduled_task_id.as_option().map(|id| id.bytes)
    }

    pub fn scheduled_occurrence_index_value(&self) -> Option<u64> {
        self.scheduled_occurrence_index.copied_option()
    }

    pub fn skill_revision_key(&self) -> Option<SkillRevisionLookupKey> {
        Some(SkillRevisionLookupKey {
            agent_id: self.agent_id_address(),
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
        })
    }

    pub fn to_context(&self) -> anyhow::Result<Option<RequestWalkContext>> {
        Ok(Some(RequestWalkContext {
            agent_id: self.agent_id_address(),
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
            scheduled_task_id: self.scheduled_task_id_address(),
            scheduled_occurrence_index: self.scheduled_occurrence_index_value(),
        }))
    }
}

impl DAGWalk {
    pub fn timeout_expired_vertex(&self, clock_ms: u64) -> Option<&RuntimeVertex> {
        match self {
            Self::Active {
                next_vertex,
                timeout_ms,
                created_at,
                ..
            }
            | Self::PendingSettlement {
                next_vertex,
                timeout_ms,
                created_at,
                ..
            } if clock_ms >= created_at.saturating_add(timeout_ms.saturating_mul(2)) => {
                Some(next_vertex)
            }
            _ => None,
        }
    }

    pub fn abortable_timeout_expired_vertex(&self, clock_ms: u64) -> Option<&RuntimeVertex> {
        match self {
            Self::Active {
                next_vertex,
                timeout_ms,
                created_at,
                ..
            } if clock_ms >= created_at.saturating_add(timeout_ms.saturating_mul(2)) => {
                Some(next_vertex)
            }
            _ => None,
        }
    }

    pub fn expired_active_vertex(&self, clock_ms: u64) -> Option<&RuntimeVertex> {
        self.abortable_timeout_expired_vertex(clock_ms)
    }
}
