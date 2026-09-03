//! Sui gas coin transaction helpers.

use crate::{move_boundary, sui, types::NexusContext};

/// Move SUI from the transaction gas coin into an address balance.
///
/// The selected gas coin must cover both `amount_mist` and the transaction gas
/// budget. The remainder stays in the gas coin.
pub fn deposit_sui_to_address_balance(
    objects: &NexusContext,
    amount_mist: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    deposit_sui_to_address_balances(objects, &[(amount_mist, recipient)])
}

/// Move SUI from the transaction gas coin into several address balances.
///
/// The selected gas coin must cover every deposit and the transaction gas
/// budget. One coin split produces every deposit coin, then each coin is
/// consumed by the framework funds call for its recipient.
pub fn deposit_sui_to_address_balances(
    objects: &NexusContext,
    deposits: &[(u64, sui::types::Address)],
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    anyhow::ensure!(!deposits.is_empty(), "At least one deposit is required");

    move_boundary::ptb(objects, |transaction| {
        let amounts = deposits
            .iter()
            .map(|(amount, _)| transaction.arg(amount))
            .collect::<Result<Vec<_>, _>>()?;
        let gas = transaction.gas();
        let split = transaction.split_coins(gas, amounts)?;
        for (index, (_, recipient)) in deposits.iter().enumerate() {
            let index = u16::try_from(index)?;
            let coin = transaction.nested_result(split, index)?;
            transaction.send_sui_to_address_balance(coin, *recipient)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::test_utils::sui_mocks,
        sui::types::{Argument, Command},
    };

    #[test]
    fn batch_deposit_splits_once_and_consumes_every_coin() {
        let context = sui_mocks::mock_nexus_context();
        let recipients = [
            sui::types::Address::from_static("0x11"),
            sui::types::Address::from_static("0x12"),
            sui::types::Address::from_static("0x13"),
        ];
        let transaction = deposit_sui_to_address_balances(
            &context,
            &[
                (10, recipients[0]),
                (20, recipients[1]),
                (30, recipients[2]),
            ],
        )
        .expect("batch deposit should build");

        let Command::SplitCoins(split) = &transaction.commands[0] else {
            panic!("first command must split the gas coin");
        };
        assert_eq!(split.coin, Argument::Gas);
        assert_eq!(split.amounts.len(), recipients.len());
        for (index, _) in recipients.iter().enumerate() {
            let Command::MoveCall(call) = &transaction.commands[index + 1] else {
                panic!("deposit coin must be consumed by a Move call");
            };
            assert_eq!(call.package, sui::types::Address::TWO);
            assert_eq!(call.module.as_str(), "coin");
            assert_eq!(call.function.as_str(), "send_funds");
            assert_eq!(
                call.arguments[0],
                Argument::NestedResult(0, u16::try_from(index).unwrap())
            );
            let Argument::Input(recipient_input) = call.arguments[1] else {
                panic!("recipient must be a pure input");
            };
            assert_eq!(
                usize::from(recipient_input),
                recipients.len() + index,
                "each funds call must use its matching recipient input"
            );
        }
    }

    #[test]
    fn batch_deposit_rejects_an_empty_request() {
        let error =
            deposit_sui_to_address_balances(&sui_mocks::mock_nexus_context(), &[]).unwrap_err();

        assert!(error.to_string().contains("At least one deposit"));
    }
}
