use crate::{
    crypto::x3dh::{InitialMessage, PreKeyBundle},
    idents::{pure_arg, sui_framework, workflow},
    sui,
    types::NexusObjects,
};

/// PTB to claim a pre_key for the tx sender. Note that one must have uploaded
/// gas budget before calling this function for rate limiting purposes. Also
/// rate limited per address.
pub fn claim_pre_key_for_self(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut PreKeyVault`
    let pre_key_vault = tx.input(sui::tx::Input::shared(
        *objects.pre_key_vault.object_id(),
        objects.pre_key_vault.version(),
        true,
    ));

    // `gas_service: &GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        false,
    ));

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus_workflow::pre_key_vault::claim_pre_key_for_self`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF.module,
            workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF.name,
            vec![],
        ),
        vec![pre_key_vault, gas_service, clock],
    ))
}

/// PTB to fulfill a user's pending pre_key request with the provided bundle.
pub fn fulfill_pre_key_for_user(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    crypto_cap: &sui::types::ObjectReference,
    requested_by: sui::types::Address,
    pre_key: &PreKeyBundle,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut PreKeyVault`
    let pre_key_vault = tx.input(sui::tx::Input::shared(
        *objects.pre_key_vault.object_id(),
        objects.pre_key_vault.version(),
        true,
    ));

    // `owner_cap: &CloneableOwnerCap<OverCrypto>`
    let crypto_cap = tx.input(sui::tx::Input::shared(
        *crypto_cap.object_id(),
        crypto_cap.version(),
        false,
    ));

    // `requested_by: address`
    let requested_by = sui_framework::Address::address_from_type(tx, requested_by)?;

    // `pre_key_bytes: vector<u8>`
    let pre_key_bytes = tx.input(pure_arg(&bincode::serialize(pre_key)?)?);

    // `nexus_workflow::pre_key_vault::fulfill_pre_key_for_user`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER.module,
            workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER.name,
            vec![],
        ),
        vec![pre_key_vault, crypto_cap, requested_by, pre_key_bytes],
    ))
}

/// PTB template to claim gas from the requester and fulfill a requested pre key.
// This PTB template maps directly to the underlying Move call parameters, so keep
// the argument list and silence the clippy lint.
#[allow(clippy::too_many_arguments)]
pub fn claim_and_fulfill_pre_key_for_user(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    crypto_cap: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    leader_address: sui::types::Address,
    requested_by: sui::types::Address,
    pre_key: &PreKeyBundle,
    mist_gas_budget_to_claim: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `gas_service: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `amount: u64`
    let amount = tx.input(pure_arg(&mist_gas_budget_to_claim)?);

    // `requested_by: address`
    let requested_by_arg = sui_framework::Address::address_from_type(tx, requested_by)?;

    // `leader_cap: &CloneableOwnerCap<OverNetwork>`
    let leader_cap_obj = tx.input(sui::tx::Input::shared(
        *leader_cap.object_id(),
        leader_cap.version(),
        false,
    ));

    // Claim gas from the requester into a temporary balance.
    let balance = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Gas::CLAIM_LEADER_GAS_FOR_PRE_KEY.module,
            workflow::Gas::CLAIM_LEADER_GAS_FOR_PRE_KEY.name,
            vec![],
        ),
        vec![gas_service, requested_by_arg, leader_cap_obj, amount],
    );

    // Convert the balance into a Coin<SUI>.
    let sui = sui_framework::into_type_tag(sui_framework::Sui::SUI);
    let coin = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Coin::FROM_BALANCE.module,
            sui_framework::Coin::FROM_BALANCE.name,
            vec![sui],
        ),
        vec![balance],
    );

    // `address`
    let address = sui_framework::Address::address_from_type(tx, leader_address)?;

    // `Coin<SUI>`
    let coin_sui = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::gas_coin()));

    // Transfer the claimed gas back to the leader's wallet.
    // `sui::transfer::public_transfer()`
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_TRANSFER.module,
            sui_framework::Transfer::PUBLIC_TRANSFER.name,
            vec![coin_sui],
        ),
        vec![coin, address],
    );

    fulfill_pre_key_for_user(tx, objects, crypto_cap, requested_by, pre_key)
}

/// PTB template to associate a claimed pre key with the sender address while
/// sending an initial message.
pub fn associate_pre_key_with_sender(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    initial_message: InitialMessage,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut PreKeyVault`
    let pre_key_vault = tx.input(sui::tx::Input::shared(
        *objects.pre_key_vault.object_id(),
        objects.pre_key_vault.version(),
        true,
    ));

    // `initial_message: vector<u8>`
    let initial_message = tx.input(pure_arg(&bincode::serialize(&initial_message)?)?);

    // `nexus_workflow::pre_key_vault::associate_pre_key`
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::PreKeyVault::ASSOCIATE_PRE_KEY.module,
            workflow::PreKeyVault::ASSOCIATE_PRE_KEY.name,
            vec![],
        ),
        vec![pre_key_vault, initial_message],
    ))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks, x25519_dalek::PublicKey};

    #[test]
    fn test_claim_pre_key_for_self() {
        let objects = sui_mocks::mock_nexus_objects();

        let mut tx = sui::tx::TransactionBuilder::new();
        tx.set_sender(sui::types::Address::from_static("0x1"));
        tx.set_gas_budget(1000);
        tx.set_gas_price(1000);
        let gas = sui_mocks::mock_sui_object_ref();
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas.object_id(),
            gas.version(),
            *gas.digest(),
        )]);
        claim_pre_key_for_self(&mut tx, &objects).unwrap();
        let tx = tx.finish().expect("Transaction should build");
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to claim pre_key for self");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF.module
        );
        assert_eq!(
            call.function,
            workflow::PreKeyVault::CLAIM_PRE_KEY_FOR_SELF.name
        );
    }

    #[test]
    fn test_fulfill_pre_key_for_user() {
        let rng = &mut rand::thread_rng();
        let objects = sui_mocks::mock_nexus_objects();
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let identity = crate::crypto::x3dh::IdentityKey::generate();
        let spk_secret = x25519_dalek::StaticSecret::from([1; 32]);
        let pre_key = PreKeyBundle::new(&identity, 1, &spk_secret, None, None);

        let mut tx = sui::tx::TransactionBuilder::new();
        fulfill_pre_key_for_user(
            &mut tx,
            &objects,
            &owner_cap,
            sui::types::Address::generate(rng),
            &pre_key,
        )
        .unwrap();
        tx.set_sender(sui::types::Address::from_static("0x1"));
        tx.set_gas_budget(1000);
        tx.set_gas_price(1000);
        let gas = sui_mocks::mock_sui_object_ref();
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas.object_id(),
            gas.version(),
            *gas.digest(),
        )]);
        let tx = tx.finish().expect("Transaction should build");
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to fulfill pre_key for user");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER.module
        );
        assert_eq!(
            call.function,
            workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER.name
        );
    }

    #[test]
    fn test_claim_and_fulfill_pre_key_for_user() {
        let rng = &mut rand::thread_rng();
        let objects = sui_mocks::mock_nexus_objects();
        let crypto_cap = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let leader_address = sui_mocks::mock_sui_address();
        let identity = crate::crypto::x3dh::IdentityKey::generate();
        let spk_secret = x25519_dalek::StaticSecret::from([1; 32]);
        let pre_key = PreKeyBundle::new(&identity, 1, &spk_secret, None, None);
        let requested_by = sui::types::Address::generate(rng);

        let mut tx = sui::tx::TransactionBuilder::new();
        claim_and_fulfill_pre_key_for_user(
            &mut tx,
            &objects,
            &crypto_cap,
            &leader_cap,
            leader_address,
            requested_by,
            &pre_key,
            1,
        )
        .unwrap();
        tx.set_sender(sui::types::Address::from_static("0x1"));
        tx.set_gas_budget(1000);
        tx.set_gas_price(1000);
        let gas = sui_mocks::mock_sui_object_ref();
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas.object_id(),
            gas.version(),
            *gas.digest(),
        )]);
        let tx = tx.finish().expect("Transaction should build");
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let move_calls: Vec<_> = commands
            .iter()
            .filter_map(|cmd| {
                if let sui::types::Command::MoveCall(call) = cmd {
                    Some(call)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(move_calls.len(), 4);

        assert_eq!(
            move_calls[0].function,
            workflow::Gas::CLAIM_LEADER_GAS_FOR_PRE_KEY.name
        );
        assert_eq!(
            move_calls[1].function,
            sui_framework::Coin::FROM_BALANCE.name
        );
        assert_eq!(move_calls[2].function, sui_framework::Coin::JOIN.name);
        assert_eq!(
            move_calls[3].function,
            workflow::PreKeyVault::FULFILL_PRE_KEY_FOR_USER.name
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

        let mut tx = sui::tx::TransactionBuilder::new();
        associate_pre_key_with_sender(&mut tx, &objects, initial_message).unwrap();
        tx.set_sender(sui::types::Address::from_static("0x1"));
        tx.set_gas_budget(1000);
        tx.set_gas_price(1000);
        let gas = sui_mocks::mock_sui_object_ref();
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas.object_id(),
            gas.version(),
            *gas.digest(),
        )]);
        let tx = tx.finish().expect("Transaction should build");
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        let sui::types::Command::MoveCall(call) = &commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to associate pre_key with sender");
        };

        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::PreKeyVault::ASSOCIATE_PRE_KEY.module);
        assert_eq!(call.function, workflow::PreKeyVault::ASSOCIATE_PRE_KEY.name);
    }
}
