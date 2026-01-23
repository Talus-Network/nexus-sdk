//! Programmable transaction builders for `nexus_workflow::network_auth`.

use crate::{
    idents::{move_std, pure_arg, sui_framework, workflow},
    sui,
    types::NexusObjects,
    ToolFqn,
};

/// Create a new off-chain tool key binding and register the first key.
///
/// This is used when the binding object does not yet exist.
///
/// The created `KeyBinding` object is transferred to `sender`.
#[allow(clippy::too_many_arguments)]
pub fn create_tool_binding_and_register_key(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    sender: sui::types::Address,
    tool_fqn: &ToolFqn,
    owner_cap_over_tool: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
    description: Option<Vec<u8>>,
) -> anyhow::Result<()> {
    // `registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `owner_cap: &CloneableOwnerCap<OverTool>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap_over_tool.object_id(),
        owner_cap_over_tool.version(),
        *owner_cap_over_tool.digest(),
    ));

    // `fqn: AsciiString` (consumed by the Move call)
    let fqn_for_binding = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `proof: ProofOfIdentity`
    let proof_for_binding = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.module,
            workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.name,
            vec![],
        ),
        vec![tool_registry, owner_cap, fqn_for_binding],
    );

    // `registry: &mut NetworkAuth`
    let network_auth = tx.input(sui::tx::Input::shared(
        *objects.network_auth.object_id(),
        objects.network_auth.version(),
        true,
    ));

    // `description: Option<vector<u8>>`
    let description = tx.input(pure_arg(&description)?);

    // `binding: KeyBinding`
    let binding = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::CREATE_BINDING.module,
            workflow::NetworkAuth::CREATE_BINDING.name,
            vec![],
        ),
        vec![network_auth, proof_for_binding, description],
    );

    let fqn_for_key = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // Need a fresh proof for key registration (the previous one was consumed by create_binding).
    let proof_for_key = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.module,
            workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.name,
            vec![],
        ),
        vec![tool_registry, owner_cap, fqn_for_key],
    );

    // `public_key: vector<u8>`
    let public_key = tx.input(pure_arg(&public_key.to_vec())?);
    // `signature: vector<u8>`
    let signature = tx.input(pure_arg(&pop_signature.to_vec())?);

    // `proof_of_key: ProofOfKey`
    let proof_of_key = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.name,
            vec![],
        ),
        vec![binding, proof_for_key, public_key, signature],
    );

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus_workflow::network_auth::register_key(binding, &proof, proof_of_key, clock)`
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::REGISTER_KEY.module,
            workflow::NetworkAuth::REGISTER_KEY.name,
            vec![],
        ),
        vec![binding, proof_for_key, proof_of_key, clock],
    );

    // Transfer the newly created binding back to the sender.
    let address = sui_framework::Address::address_from_type(tx, sender)?;
    tx.transfer_objects(vec![binding], address);

    Ok(())
}

/// Register a new key on an existing off-chain tool key binding.
///
/// This is used for rotation when the `KeyBinding` already exists.
#[allow(clippy::too_many_arguments)]
pub fn register_tool_key_on_existing_binding(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    binding: &sui::types::ObjectReference,
    tool_fqn: &ToolFqn,
    owner_cap_over_tool: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
) -> anyhow::Result<()> {
    // `binding: &mut KeyBinding` (owned object)
    let binding = tx.input(sui::tx::Input::owned(
        *binding.object_id(),
        binding.version(),
        *binding.digest(),
    ));

    // `registry: &ToolRegistry`
    let tool_registry = tx.input(sui::tx::Input::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `owner_cap: &CloneableOwnerCap<OverTool>`
    let owner_cap = tx.input(sui::tx::Input::owned(
        *owner_cap_over_tool.object_id(),
        owner_cap_over_tool.version(),
        *owner_cap_over_tool.digest(),
    ));

    // `fqn: AsciiString`
    let fqn = move_std::Ascii::ascii_string_from_str(tx, tool_fqn.to_string())?;

    // `proof: ProofOfIdentity`
    let proof = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.module,
            workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.name,
            vec![],
        ),
        vec![tool_registry, owner_cap, fqn],
    );

    let public_key = tx.input(pure_arg(&public_key.to_vec())?);
    let signature = tx.input(pure_arg(&pop_signature.to_vec())?);

    let proof_of_key = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.name,
            vec![],
        ),
        vec![binding, proof, public_key, signature],
    );

    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::REGISTER_KEY.module,
            workflow::NetworkAuth::REGISTER_KEY.name,
            vec![],
        ),
        vec![binding, proof, proof_of_key, clock],
    );

    Ok(())
}

/// Create a new leader key binding and register the first key.
///
/// This is used when the binding object does not yet exist.
///
/// The created `KeyBinding` object is transferred to `sender`.
#[allow(clippy::too_many_arguments)]
pub fn create_leader_binding_and_register_key(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    sender: sui::types::Address,
    leader_cap_over_network: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
    description: Option<Vec<u8>>,
) -> anyhow::Result<()> {
    // `leader_cap: &CloneableOwnerCap<OverNetwork>`
    let leader_cap = tx.input(sui::tx::Input::shared(
        *leader_cap_over_network.object_id(),
        leader_cap_over_network.version(),
        false,
    ));

    // `proof: ProofOfIdentity`
    let proof_for_binding = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::PROVE_LEADER.module,
            workflow::NetworkAuth::PROVE_LEADER.name,
            vec![],
        ),
        vec![leader_cap],
    );

    // `registry: &mut NetworkAuth`
    let network_auth = tx.input(sui::tx::Input::shared(
        *objects.network_auth.object_id(),
        objects.network_auth.version(),
        true,
    ));

    // `description: Option<vector<u8>>`
    let description = tx.input(pure_arg(&description)?);

    // `binding: KeyBinding`
    let binding = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::CREATE_BINDING.module,
            workflow::NetworkAuth::CREATE_BINDING.name,
            vec![],
        ),
        vec![network_auth, proof_for_binding, description],
    );

    // Need a fresh proof for key registration (the previous one was consumed by create_binding).
    let proof_for_key = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::PROVE_LEADER.module,
            workflow::NetworkAuth::PROVE_LEADER.name,
            vec![],
        ),
        vec![leader_cap],
    );

    // `public_key: vector<u8>`
    let public_key = tx.input(pure_arg(&public_key.to_vec())?);
    // `signature: vector<u8>`
    let signature = tx.input(pure_arg(&pop_signature.to_vec())?);

    // `proof_of_key: ProofOfKey`
    let proof_of_key = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.name,
            vec![],
        ),
        vec![binding, proof_for_key, public_key, signature],
    );

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `nexus_workflow::network_auth::register_key(binding, &proof, proof_of_key, clock)`
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::REGISTER_KEY.module,
            workflow::NetworkAuth::REGISTER_KEY.name,
            vec![],
        ),
        vec![binding, proof_for_key, proof_of_key, clock],
    );

    // Transfer the newly created binding back to the sender.
    let address = sui_framework::Address::address_from_type(tx, sender)?;
    tx.transfer_objects(vec![binding], address);

    Ok(())
}

/// Register a new key on an existing leader key binding.
///
/// This is used for rotation when the `KeyBinding` already exists.
#[allow(clippy::too_many_arguments)]
pub fn register_leader_key_on_existing_binding(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    binding: &sui::types::ObjectReference,
    leader_cap_over_network: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
) -> anyhow::Result<()> {
    // `binding: &mut KeyBinding` (owned object)
    let binding = tx.input(sui::tx::Input::owned(
        *binding.object_id(),
        binding.version(),
        *binding.digest(),
    ));

    // `leader_cap: &CloneableOwnerCap<OverNetwork>`
    let leader_cap = tx.input(sui::tx::Input::shared(
        *leader_cap_over_network.object_id(),
        leader_cap_over_network.version(),
        false,
    ));

    // `proof: ProofOfIdentity`
    let proof = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::PROVE_LEADER.module,
            workflow::NetworkAuth::PROVE_LEADER.name,
            vec![],
        ),
        vec![leader_cap],
    );

    let public_key = tx.input(pure_arg(&public_key.to_vec())?);
    let signature = tx.input(pure_arg(&pop_signature.to_vec())?);

    let proof_of_key = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
            workflow::NetworkAuth::NEW_PROOF_OF_KEY.name,
            vec![],
        ),
        vec![binding, proof, public_key, signature],
    );

    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::NetworkAuth::REGISTER_KEY.module,
            workflow::NetworkAuth::REGISTER_KEY.name,
            vec![],
        ),
        vec![binding, proof, proof_of_key, clock],
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{fqn, idents::workflow, test_utils::sui_mocks},
    };

    fn count_move_calls(
        commands: &[sui::types::Command],
        pkg: sui::types::Address,
        module: sui::types::Identifier,
        function: sui::types::Identifier,
    ) -> usize {
        commands
            .iter()
            .filter(|cmd| {
                matches!(
                    cmd,
                    sui::types::Command::MoveCall(call)
                        if call.package == pkg
                            && call.module == module
                            && call.function == function
                )
            })
            .count()
    }

    fn has_transfer(commands: &[sui::types::Command]) -> bool {
        commands
            .iter()
            .any(|cmd| matches!(cmd, sui::types::Command::TransferObjects(_)))
    }

    #[test]
    fn test_create_tool_binding_and_register_key_builds_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let sender = sui_mocks::mock_sui_address();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let public_key = [7u8; 32];
        let pop_signature = [9u8; 64];
        let description = Some(b"rotation-1".to_vec());

        let mut tx = sui::tx::TransactionBuilder::new();
        create_tool_binding_and_register_key(
            &mut tx,
            &objects,
            sender,
            &tool_fqn,
            &owner_cap,
            public_key,
            pop_signature,
            description,
        )
        .expect("Failed to build PTB for tool binding creation");

        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        assert!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.module,
                workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.name
            ) >= 2
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::CREATE_BINDING.module,
                workflow::NetworkAuth::CREATE_BINDING.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::REGISTER_KEY.module,
                workflow::NetworkAuth::REGISTER_KEY.name
            ),
            1
        );
        assert!(has_transfer(&commands));
    }

    #[test]
    fn test_register_tool_key_on_existing_binding_builds_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let binding = sui_mocks::mock_sui_object_ref();
        let owner_cap = sui_mocks::mock_sui_object_ref();
        let tool_fqn = fqn!("xyz.test.tool@1");
        let public_key = [11u8; 32];
        let pop_signature = [22u8; 64];

        let mut tx = sui::tx::TransactionBuilder::new();
        register_tool_key_on_existing_binding(
            &mut tx,
            &objects,
            &binding,
            &tool_fqn,
            &owner_cap,
            public_key,
            pop_signature,
        )
        .expect("Failed to build PTB for tool key registration");

        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.module,
                workflow::NetworkAuth::PROVE_OFFCHAIN_TOOL.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::REGISTER_KEY.module,
                workflow::NetworkAuth::REGISTER_KEY.name
            ),
            1
        );
        assert!(!has_transfer(&commands));
    }

    #[test]
    fn test_create_leader_binding_and_register_key_builds_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let sender = sui_mocks::mock_sui_address();
        let public_key = [5u8; 32];
        let pop_signature = [6u8; 64];
        let description = Some(b"leader-key".to_vec());

        let mut tx = sui::tx::TransactionBuilder::new();
        create_leader_binding_and_register_key(
            &mut tx,
            &objects,
            sender,
            &leader_cap,
            public_key,
            pop_signature,
            description,
        )
        .expect("Failed to build PTB for leader binding creation");

        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        assert!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::PROVE_LEADER.module,
                workflow::NetworkAuth::PROVE_LEADER.name
            ) >= 2
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::CREATE_BINDING.module,
                workflow::NetworkAuth::CREATE_BINDING.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::REGISTER_KEY.module,
                workflow::NetworkAuth::REGISTER_KEY.name
            ),
            1
        );
        assert!(has_transfer(&commands));
    }

    #[test]
    fn test_register_leader_key_on_existing_binding_builds_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let binding = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let public_key = [3u8; 32];
        let pop_signature = [4u8; 64];

        let mut tx = sui::tx::TransactionBuilder::new();
        register_leader_key_on_existing_binding(
            &mut tx,
            &objects,
            &binding,
            &leader_cap,
            public_key,
            pop_signature,
        )
        .expect("Failed to build PTB for leader key registration");

        let tx = sui_mocks::mock_finish_transaction(tx);
        let sui::types::TransactionKind::ProgrammableTransaction(
            sui::types::ProgrammableTransaction { commands, .. },
        ) = tx.kind
        else {
            panic!("Expected a ProgrammableTransaction");
        };

        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::PROVE_LEADER.module,
                workflow::NetworkAuth::PROVE_LEADER.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.module,
                workflow::NetworkAuth::NEW_PROOF_OF_KEY.name
            ),
            1
        );
        assert_eq!(
            count_move_calls(
                &commands,
                objects.workflow_pkg_id,
                workflow::NetworkAuth::REGISTER_KEY.module,
                workflow::NetworkAuth::REGISTER_KEY.name
            ),
            1
        );
        assert!(!has_transfer(&commands));
    }
}
