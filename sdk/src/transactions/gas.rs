use crate::{
    idents::{sui_framework, workflow},
    sui,
};

/// PTB template to add gas budget to a transaction.
pub fn add_budget(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    gas_service: &sui::ObjectRef,
    invoker_address: sui::ObjectID,
    coin: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: gas_service.object_id,
        initial_shared_version: gas_service.version,
        mutable: true,
    })?;

    // `scope: Scope`
    let scope =
        workflow::Gas::scope_invoker_address_from_object_id(tx, workflow_pkg_id, invoker_address)?;

    // `balance: Balance<SUI>`
    let coin = tx.obj(sui::ObjectArg::ImmOrOwnedObject(coin.to_object_ref()))?;
    let sui = sui_framework::into_type_tag(sui_framework::Sui::SUI);

    let balance = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::Coin::INTO_BALANCE.module.into(),
        sui_framework::Coin::INTO_BALANCE.name.into(),
        vec![sui],
        vec![coin],
    );

    // `nexus_workflow::gas::add_gas_budget`
    Ok(tx.programmable_move_call(
        workflow_pkg_id,
        workflow::Gas::ADD_GAS_BUDGET.module.into(),
        workflow::Gas::ADD_GAS_BUDGET.name.into(),
        vec![],
        vec![gas_service, scope, balance],
    ))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::test_utils::sui_mocks::{mock_sui_coin, mock_sui_object_ref},
    };
    #[test]
    fn test_add_budget() {
        let workflow_pkg_id = sui::ObjectID::random();
        let gas_service = mock_sui_object_ref();
        let invoker_address = sui::ObjectID::random();
        let coin = mock_sui_object_ref();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        add_budget(
            &mut tx,
            workflow_pkg_id,
            &gas_service,
            invoker_address,
            &coin,
        )
        .unwrap();
        let tx = tx.finish();

        let sui::Command::MoveCall(call) = &tx.commands.last().unwrap() else {
            panic!("Expected last command to be a MoveCall to add gas budget");
        };

        assert_eq!(call.package, workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::Gas::ADD_GAS_BUDGET.module.to_string(),
        );
        assert_eq!(
            call.function,
            workflow::Gas::ADD_GAS_BUDGET.name.to_string()
        );
    }
}
