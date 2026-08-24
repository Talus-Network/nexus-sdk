//! SDK projections for generated execution payment values.
//!
//! [`crate::move_bindings::interface::payment::ExecutionPayment`] remains the persisted payment
//! object shape. Settlement code uses these helpers to read object identity, derive
//! [`crate::types::SkillRevisionLookupKey`], and inspect lock counts without copying payment data
//! into another model.

use crate::move_bindings::interface::payment::{ExecutionPayment, ExecutionPaymentInnerV1};

impl ExecutionPayment {
    pub fn payment_id(&self) -> crate::sui::types::Address {
        self.id.id.bytes
    }
}

impl ExecutionPaymentInnerV1 {
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
