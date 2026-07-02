//! Companion helpers for generated `nexus_interface::payment` types.

use crate::move_bindings::interface::payment::ExecutionPayment;

impl ExecutionPayment {
    pub fn payment_id(&self) -> crate::sui::types::Address {
        self.id.id.bytes
    }

    pub fn skill_revision_key(&self) -> crate::types::SkillRevisionLookupKey {
        crate::types::SkillRevisionLookupKey {
            agent_id: self.agent_id.bytes,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }

    pub fn locks(&self) -> u64 {
        self.locked_vertices.len() as u64
    }
}
