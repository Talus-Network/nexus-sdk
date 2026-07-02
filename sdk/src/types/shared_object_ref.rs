//! SDK helpers for generated `nexus_primitives::shared_object::SharedObjectRef`.

pub use crate::types::primitives::shared_object::SharedObjectRef;
use crate::{sui, types::sui_framework::object::ID};

impl SharedObjectRef {
    /// Create a new immutable shared object reference.
    pub fn new_imm(id: sui::types::Address) -> Self {
        Self {
            id: ID { bytes: id },
            ref_mut: false,
        }
    }

    /// Create a new mutable shared object reference.
    pub fn new_mut(id: sui::types::Address) -> Self {
        Self {
            id: ID { bytes: id },
            ref_mut: true,
        }
    }

    pub fn id_address(&self) -> sui::types::Address {
        self.id.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_set_mutability_and_id_address() {
        let immutable_id = sui::types::Address::from_static("0x123");
        let mutable_id = sui::types::Address::from_static("0x456");

        let immutable = SharedObjectRef::new_imm(immutable_id);
        let mutable = SharedObjectRef::new_mut(mutable_id);

        assert_eq!(immutable.id_address(), immutable_id);
        assert!(!immutable.ref_mut);
        assert_eq!(mutable.id_address(), mutable_id);
        assert!(mutable.ref_mut);
    }
}
