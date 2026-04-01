//! Defines a struct that wraps an ID of a shared object along with its desired
//! reference type (immutable or mutable) for the purposes of a TAP.

use {
    crate::sui,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedObjectRef {
    pub id: sui::types::Address,
    pub ref_mut: bool,
}

impl SharedObjectRef {
    /// Create a new immutable shared object reference.
    pub fn new_imm(id: sui::types::Address) -> Self {
        Self { id, ref_mut: false }
    }

    /// Create a new mutable shared object reference.
    pub fn new_mut(id: sui::types::Address) -> Self {
        Self { id, ref_mut: true }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks};

    #[test]
    fn test_new_imm_creates_immutable_ref() {
        let addr = sui_mocks::mock_sui_address();
        let shared_ref = SharedObjectRef::new_imm(addr);
        assert_eq!(shared_ref.id, addr);
        assert!(!shared_ref.ref_mut);
    }

    #[test]
    fn test_new_mut_creates_mutable_ref() {
        let addr = sui_mocks::mock_sui_address();
        let shared_ref = SharedObjectRef::new_mut(addr);
        assert_eq!(shared_ref.id, addr);
        assert!(shared_ref.ref_mut);
    }

    #[test]
    fn test_equality() {
        let addr = sui_mocks::mock_sui_address();
        let imm1 = SharedObjectRef::new_imm(addr);
        let imm2 = SharedObjectRef::new_imm(addr);
        let mut1 = SharedObjectRef::new_mut(addr);
        assert_eq!(imm1, imm2);
        assert_ne!(imm1, mut1);
    }

    #[test]
    fn test_clone() {
        let addr = sui_mocks::mock_sui_address();
        let shared_ref = SharedObjectRef::new_mut(addr);
        let cloned = shared_ref.clone();
        assert_eq!(shared_ref, cloned);
    }

    #[test]
    fn test_serde_roundtrip() {
        let addr = sui_mocks::mock_sui_address();
        let shared_ref = SharedObjectRef::new_imm(addr);
        let serialized = serde_json::to_string(&shared_ref).unwrap();
        let deserialized: SharedObjectRef = serde_json::from_str(&serialized).unwrap();
        assert_eq!(shared_ref, deserialized);
    }
}
