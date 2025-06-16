use crate::{
    crypto::x3dh::{InitialMessage, PreKeyBundle},
    idents::{move_std, workflow},
    sui,
    types::NexusObjects,
};

/// PTB template to replenish public pre_keys in the pre_key vault.
pub fn replenish_pre_keys(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    owner_cap: &sui::ObjectRef,
    pre_keys: &Vec<PreKeyBundle>,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut PreKeyVault`
    let pre_key_vault = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.pre_key_vault.object_id,
        initial_shared_version: objects.pre_key_vault.version,
        mutable: true,
    })?;

    // `owner_cap: &CloneableOwnerCap<OverCrypto>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `PreKey`
    let pre_key_type =
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::PreKeyVault::PRE_KEY);

    // `vector::<PreKey>::empty`
    let pre_key_vector = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        move_std::Vector::EMPTY.module.into(),
        move_std::Vector::EMPTY.name.into(),
        vec![pre_key_type.clone()],
        vec![],
    );

    for pre_key in pre_keys {
        // `bytes: vector<u8>`
        let pre_key_bytes = tx.pure(bincode::serialize(pre_key)?)?;

        // `pre_key: PreKey`
        let new_pre_key = tx.programmable_move_call(
            objects.workflow_pkg_id,
            workflow::PreKeyVault::PRE_KEY_FROM_BYTES.module.into(),
            workflow::PreKeyVault::PRE_KEY_FROM_BYTES.name.into(),
            vec![],
            vec![pre_key_bytes],
        );

        // `vector::<PreKey>::push_back`
        tx.programmable_move_call(
            sui::MOVE_STDLIB_PACKAGE_ID,
            move_std::Vector::PUSH_BACK.module.into(),
            move_std::Vector::PUSH_BACK.name.into(),
            vec![pre_key_type.clone()],
            vec![pre_key_vector, new_pre_key],
        );
    }

    // `nexus_workflow::pre_key_vault::replenish_pre_keys`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::PreKeyVault::REPLENISH_PRE_KEYS.module.into(),
        workflow::PreKeyVault::REPLENISH_PRE_KEYS.name.into(),
        vec![],
        vec![pre_key_vault, owner_cap, pre_key_vector],
    ))
}

/// PTB to claim a pre_key for the tx sender. Note that one must have uploaded
/// gas budget before calling this function for rate limiting purposes. Also
/// rate limited per address.
pub fn claim_pre_key_for_self(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut PreKeyVault`
    let pre_key_vault = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.pre_key_vault.object_id,
        initial_shared_version: objects.pre_key_vault.version,
        mutable: true,
    })?;

    // `gas_service: &GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: false,
    })?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    // `nexus_workflow::pre_key_vault::claim_pre_key_for_self`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF.module.into(),
        workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF.name.into(),
        vec![],
        vec![pre_key_vault, gas_service, clock],
    ))
}

/// PTB template to associate a claimed pre key with the sender address while
/// sending an initial message.
pub fn associate_pre_key_with_sender(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    pre_key: &sui::ObjectRef,
    initial_message: InitialMessage,
) -> anyhow::Result<sui::Argument> {
    // `pre_key: PreKey`
    let pre_key = tx.obj(sui::ObjectArg::ImmOrOwnedObject(pre_key.to_object_ref()))?;

    // `initial_message: vector<u8>`
    let initial_message = tx.pure(bincode::serialize(&initial_message)?)?;

    // `nexus_workflow::pre_key_vault::associate_pre_key`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::PreKeyVault::ASSOCIATE_PRE_KEY.module.into(),
        workflow::PreKeyVault::ASSOCIATE_PRE_KEY.name.into(),
        vec![],
        vec![pre_key, initial_message],
    ))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks, x25519_dalek::PublicKey};

    #[test]
    fn test_replenish_pre_keys() {
        let objects = sui_mocks::mock_nexus_objects();
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        replenish_pre_keys(&mut tx, &objects, &owner_cap, &vec![]).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to replenish pre_keys");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::PreKeyVault::REPLENISH_PRE_KEYS.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::PreKeyVault::REPLENISH_PRE_KEYS.name.to_string()
        );
    }

    #[test]
    fn test_claim_pre_key_for_self() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        claim_pre_key_for_self(&mut tx, &objects).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to claim pre_key for self");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF
                .name
                .to_string()
        );
    }

    #[test]
    fn test_associate_pre_key_with_sender() {
        let objects = sui_mocks::mock_nexus_objects();
        let pre_key = sui_mocks::mock_sui_object_ref();
        let initial_message = InitialMessage {
            ika_pub: PublicKey::from([0; 32]),
            ek_pub: PublicKey::from([0; 32]),
            spk_id: 1,
            otpk_id: Some(1),
            nonce: [0; 24],
            ciphertext: vec![0; 32],
        };

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        associate_pre_key_with_sender(&mut tx, &objects, &pre_key, initial_message).unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to associate pre_key with sender");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::PreKeyVault::ASSOCIATE_PRE_KEY.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::PreKeyVault::ASSOCIATE_PRE_KEY.name.to_string()
        );
    }
}
