//! SDK projections for generated execution payment values.
//!
//! [`crate::move_bindings::interface::payment::ExecutionPayment`] remains the persisted payment
//! object shape. Settlement code uses these helpers to read object identity, derive
//! [`crate::types::SkillRevisionLookupKey`], and inspect lock counts without copying payment data
//! into another model.

use {
    crate::{
        move_bindings::interface::{
            graph::RuntimeVertex,
            payment::{ExecutionPayment, ExecutionPaymentVertexLock},
        },
        sui,
        ToolFqn,
    },
    sha2::{Digest as _, Sha256},
};

fn canonical_vertex_lock_key(
    execution: sui::types::Address,
    vertex: &RuntimeVertex,
    tool_fqn: &ToolFqn,
) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"nexus.payment.vertex.v1");
    bytes.extend(bcs::to_bytes(&execution)?);
    bytes.extend(bcs::to_bytes(vertex)?);
    bytes.extend(tool_fqn.to_string().as_bytes());
    Ok(Sha256::digest(bytes).to_vec())
}

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

    /// Derive the canonical key used by the on-chain Tool payment lock.
    pub fn vertex_lock_key(
        execution: sui::types::Address,
        vertex: &RuntimeVertex,
        tool_fqn: &ToolFqn,
    ) -> anyhow::Result<Vec<u8>> {
        canonical_vertex_lock_key(execution, vertex, tool_fqn)
    }

    /// Return whether this payment contains the exact lock for one Tool vertex.
    pub fn has_vertex_lock(
        &self,
        execution: sui::types::Address,
        vertex: &RuntimeVertex,
        tool_fqn: &ToolFqn,
    ) -> anyhow::Result<bool> {
        let vertex_key = Self::vertex_lock_key(execution, vertex, tool_fqn)?;
        let tool_fqn = tool_fqn.to_string().into_bytes();
        Ok(self
            .locked_vertices
            .iter()
            .any(|lock: &ExecutionPaymentVertexLock| {
                lock.vertex_key == vertex_key && lock.tool_fqn == tool_fqn
            }))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            fqn,
            move_bindings::{
                interface::{
                    graph::RuntimeVertex,
                    payment::{
                        ExecutionPaymentFinalState,
                        ExecutionPaymentVertexLock,
                        PaymentSourceKind,
                        SkillPaymentPolicy,
                        VertexExecutionPaymentSettlementKind,
                    },
                    version::InterfaceVersion,
                },
                move_std::type_name::TypeName,
                sui_framework::{balance::Balance, object::UID, vec_map::VecMap},
            },
            sui,
        },
        sha2::Sha256,
    };

    fn canonical_vertex_key(
        execution: sui::types::Address,
        vertex: &RuntimeVertex,
        tool_fqn: &crate::ToolFqn,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"nexus.payment.vertex.v1");
        bytes.extend(bcs::to_bytes(&execution).expect("execution serializes"));
        bytes.extend(bcs::to_bytes(vertex).expect("vertex serializes"));
        bytes.extend(tool_fqn.to_string().as_bytes());
        Sha256::digest(bytes).to_vec()
    }

    fn payment_with_lock(
        execution: sui::types::Address,
        vertex: &RuntimeVertex,
        tool_fqn: &crate::ToolFqn,
    ) -> ExecutionPayment {
        ExecutionPayment {
            id: UID::new(sui::types::Address::from_static("0xe")),
            protocol_version: 1,
            execution_id: execution,
            agent_id: crate::move_bindings::sui_framework::object::ID::new(
                sui::types::Address::from_static("0xa"),
            ),
            skill_id: 11,
            interface_revision: InterfaceVersion::new(7),
            payment_policy: SkillPaymentPolicy::UserFunded,
            source_kind: PaymentSourceKind::user_funded(sui::types::Address::from_static("0x1")),
            max_budget_mist: 10,
            gas_budget_mist: 10,
            priority_fee_reserve_mist: 0,
            locked_budget_mist: 1,
            funds: Balance {
                value: 10,
                phantom_t0: std::marker::PhantomData,
            },
            consumed: 0,
            tool_fee_charged: 0,
            priority_fee_charged: 0,
            priority_fee_percentage: 0,
            accomplished: false,
            refunded: false,
            final_state: ExecutionPaymentFinalState::Pending,
            tool_cost_snapshot: VecMap { contents: vec![] },
            locked_vertices: vec![ExecutionPaymentVertexLock {
                vertex_key: canonical_vertex_key(execution, vertex, tool_fqn),
                tool_fqn: tool_fqn.to_string().into_bytes(),
                amount: 1,
                settlement_kind: VertexExecutionPaymentSettlementKind::Paid,
            }],
        }
    }

    #[test]
    fn exact_vertex_lock_predicate_matches_only_execution_vertex_and_tool() {
        let execution = sui::types::Address::from_static("0x99");
        let vertex = RuntimeVertex::WithIterator {
            vertex: TypeName::new("tool").into(),
            iteration: 2,
            out_of: 3,
        };
        let tool_fqn = fqn!("example.test.tool@1");
        let payment = payment_with_lock(execution, &vertex, &tool_fqn);

        assert_eq!(
            ExecutionPayment::vertex_lock_key(execution, &vertex, &tool_fqn)
                .expect("canonical key derives"),
            canonical_vertex_key(execution, &vertex, &tool_fqn)
        );
        assert!(payment
            .has_vertex_lock(execution, &vertex, &tool_fqn)
            .unwrap());
        assert!(!payment
            .has_vertex_lock(execution, &vertex, &fqn!("example.test.other@1"))
            .unwrap());
        assert!(!payment
            .has_vertex_lock(sui::types::Address::from_static("0x98"), &vertex, &tool_fqn)
            .unwrap());
    }

    #[test]
    fn exact_vertex_lock_predicate_rejects_empty_lock_set() {
        let execution = sui::types::Address::from_static("0x99");
        let vertex = RuntimeVertex::plain("tool");
        let tool_fqn = fqn!("example.test.tool@1");
        let mut payment = payment_with_lock(execution, &vertex, &tool_fqn);
        payment.locked_vertices.clear();

        assert!(!payment
            .has_vertex_lock(execution, &vertex, &tool_fqn)
            .unwrap());
    }

    #[test]
    fn exact_vertex_lock_predicate_rejects_corrupt_raw_key() {
        let execution = sui::types::Address::from_static("0x99");
        let vertex = RuntimeVertex::plain("tool");
        let tool_fqn = fqn!("example.test.tool@1");
        let mut payment = payment_with_lock(execution, &vertex, &tool_fqn);
        payment.locked_vertices[0].vertex_key[0] ^= 1;

        assert!(!payment
            .has_vertex_lock(execution, &vertex, &tool_fqn)
            .unwrap());
    }

    #[test]
    fn exact_vertex_lock_predicate_rejects_corrupt_tool_identity() {
        let execution = sui::types::Address::from_static("0x99");
        let vertex = RuntimeVertex::plain("tool");
        let tool_fqn = fqn!("example.test.tool@1");
        let mut payment = payment_with_lock(execution, &vertex, &tool_fqn);
        payment.locked_vertices[0].tool_fqn = b"example.test.other@1".to_vec();

        assert!(!payment
            .has_vertex_lock(execution, &vertex, &tool_fqn)
            .unwrap());
    }
}
