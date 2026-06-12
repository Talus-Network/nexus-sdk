//! Agent object inputs for programmable transaction builders.

use crate::sui;

/// Already-resolved agent object input accepted by SDK transaction builders.
///
/// The type separates ownership classification from the Move borrow a builder
/// needs. Call [`AgentInput::mutable_argument`] for `&mut Agent` calls and
/// [`AgentInput::immutable_argument`] for `&Agent` calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentInput {
    Owned(sui::types::ObjectReference),
    Shared(sui::types::ObjectReference),
    Immutable(sui::types::ObjectReference),
}

impl AgentInput {
    /// Return the object id carried by this input.
    pub fn object_id(&self) -> sui::types::Address {
        *self.object_ref().object_id()
    }

    /// Return the underlying Sui object reference.
    pub fn object_ref(&self) -> &sui::types::ObjectReference {
        match self {
            Self::Owned(object) | Self::Shared(object) | Self::Immutable(object) => object,
        }
    }

    /// Export this object as a mutable transaction argument.
    ///
    /// Immutable objects are rejected locally because Move calls requiring
    /// `&mut Agent` cannot borrow them mutably.
    pub fn mutable_argument(
        self,
        tx: &mut sui::tx::TransactionBuilder,
    ) -> anyhow::Result<sui::tx::Argument> {
        match self {
            Self::Owned(object) => Ok(tx.object(sui::tx::ObjectInput::owned(
                *object.object_id(),
                object.version(),
                *object.digest(),
            ))),
            Self::Shared(object) => Ok(tx.object(sui::tx::ObjectInput::shared(
                *object.object_id(),
                object.version(),
                true,
            ))),
            Self::Immutable(object) => Err(anyhow::anyhow!(
                "agent '{}' is immutable and cannot be used where a mutable agent reference is required",
                object.object_id()
            )),
        }
    }

    /// Export this object as an immutable transaction argument.
    pub fn immutable_argument(
        self,
        tx: &mut sui::tx::TransactionBuilder,
    ) -> anyhow::Result<sui::tx::Argument> {
        match self {
            Self::Owned(object) => Ok(tx.object(sui::tx::ObjectInput::owned(
                *object.object_id(),
                object.version(),
                *object.digest(),
            ))),
            Self::Shared(object) => Ok(tx.object(sui::tx::ObjectInput::shared(
                *object.object_id(),
                object.version(),
                false,
            ))),
            Self::Immutable(object) => Ok(tx.object(sui::tx::ObjectInput::immutable(
                *object.object_id(),
                object.version(),
                *object.digest(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks};

    struct TxInspector {
        tx: sui::types::Transaction,
    }

    impl TxInspector {
        fn new(tx: sui::types::Transaction) -> Self {
            Self { tx }
        }

        fn inputs(&self) -> &Vec<sui::types::Input> {
            let sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction { inputs, .. },
            ) = &self.tx.kind
            else {
                panic!("expected PTB transaction kind, got {:?}", self.tx.kind);
            };

            inputs
        }

        fn input(&self, argument: &sui::types::Argument) -> &sui::types::Input {
            let sui::types::Argument::Input(index) = argument else {
                panic!("expected Argument::Input, got {argument:?}");
            };

            self.inputs()
                .get(*index as usize)
                .unwrap_or_else(|| panic!("missing input for index {index}"))
        }
    }

    #[test]
    fn mutable_argument_accepts_owned_agent() {
        let object = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let _arg = AgentInput::Owned(object.clone())
            .mutable_argument(&mut tx)
            .expect("owned input is mutable");
        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));

        let sui::types::Input::ImmutableOrOwned(actual) = &inspector.inputs()[0] else {
            panic!("expected owned input, got {:?}", inspector.inputs()[0]);
        };
        assert_eq!(actual, &object);
    }

    #[test]
    fn mutable_argument_accepts_shared_agent() {
        let object = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let _arg: sui_transaction_builder::Argument = AgentInput::Shared(object.clone())
            .mutable_argument(&mut tx)
            .expect("shared input is mutable");
        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));

        let sui::types::Input::Shared(shared) = &inspector.inputs()[0] else {
            panic!("expected shared input, got {:?}", inspector.inputs()[0]);
        };
        assert_eq!(shared.object_id(), *object.object_id());
        assert_eq!(shared.version(), object.version());
        assert!(shared.mutability().is_mutable());
    }

    #[test]
    fn mutable_argument_rejects_immutable_agent() {
        let object = sui_mocks::mock_sui_object_ref();
        let object_id = *object.object_id();
        let mut tx = sui::tx::TransactionBuilder::new();
        let error = AgentInput::Immutable(object)
            .mutable_argument(&mut tx)
            .expect_err("immutable input is not mutable");

        assert!(
            error.to_string().contains(&object_id.to_string()),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn immutable_argument_borrows_shared_agent_immutably() {
        let object = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let _arg = AgentInput::Shared(object.clone())
            .immutable_argument(&mut tx)
            .expect("shared input can be immutable");
        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));

        let sui::types::Input::Shared(shared) = &inspector.inputs()[0] else {
            panic!("expected shared input, got {:?}", inspector.inputs()[0]);
        };
        assert_eq!(shared.object_id(), *object.object_id());
        assert_eq!(shared.version(), object.version());
        assert!(!shared.mutability().is_mutable());
    }
}
