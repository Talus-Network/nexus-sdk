use crate::{
    crypto::x3dh::{InitialMessage, PreKeyBundle},
    idents::workflow,
    sui,
    types::NexusObjects,
};

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

/// PTB to fulfill a user's pending pre_key request with the provided bundle.
pub fn fulfill_pre_key_for_user(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    owner_cap: &sui::ObjectRef,
    requested_by: sui::Address,
    pre_key: &PreKeyBundle,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut PreKeyVault`
    let pre_key_vault = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.pre_key_vault.object_id,
        initial_shared_version: objects.pre_key_vault.version,
        mutable: true,
    })?;

    // `owner_cap: &CloneableOwnerCap<OverCrypto>`
    let owner_cap = tx.obj(sui::ObjectArg::ImmOrOwnedObject(owner_cap.to_object_ref()))?;

    // `requested_by: address`
    let requested_by = tx.pure(requested_by)?;

    // `pre_key_bytes: vector<u8>`
    let pre_key_bytes = tx.pure(bincode::serialize(pre_key)?)?;

    // `nexus_workflow::pre_key_vault::fulfill_pre_key_for_user`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER
            .module
            .into(),
        workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER.name.into(),
        vec![],
        vec![pre_key_vault, owner_cap, requested_by, pre_key_bytes],
    ))
}

/// PTB template to associate a claimed pre key with the sender address while
/// sending an initial message.
pub fn associate_pre_key_with_sender(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    initial_message: InitialMessage,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut PreKeyVault`
    let pre_key_vault = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.pre_key_vault.object_id,
        initial_shared_version: objects.pre_key_vault.version,
        mutable: true,
    })?;

    // `initial_message: vector<u8>`
    let initial_message = tx.pure(bincode::serialize(&initial_message)?)?;

    // `nexus_workflow::pre_key_vault::associate_pre_key`
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::PreKeyVault::ASSOCIATE_PRE_KEY.module.into(),
        workflow::PreKeyVault::ASSOCIATE_PRE_KEY.name.into(),
        vec![],
        vec![pre_key_vault, initial_message],
    ))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks, x25519_dalek::PublicKey};

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
    fn test_fulfill_pre_key_for_user() {
        let objects = sui_mocks::mock_nexus_objects();
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let identity = crate::crypto::x3dh::IdentityKey::generate();
        let spk_secret = x25519_dalek::StaticSecret::from([1; 32]);
        let pre_key = PreKeyBundle::new(&identity, 1, &spk_secret, None, None);

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        fulfill_pre_key_for_user(
            &mut tx,
            &objects,
            &owner_cap,
            sui::Address::random_for_testing_only(),
            &pre_key,
        )
        .unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to fulfill pre_key for user");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER
                .module
                .to_string(),
        );
        assert_eq!(
            call.function,
            workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER
                .name
                .to_string()
        );
    }

    #[test]
    fn test_associate_pre_key_with_sender() {
        let objects = sui_mocks::mock_nexus_objects();
        let initial_message = InitialMessage {
            ika_pub: PublicKey::from([0; 32]),
            ek_pub: PublicKey::from([0; 32]),
            spk_id: 1,
            otpk_id: Some(1),
            nonce: [0; 24],
            ciphertext: vec![0; 32],
        };

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        associate_pre_key_with_sender(&mut tx, &objects, initial_message).unwrap();
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
