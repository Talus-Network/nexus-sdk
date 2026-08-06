//! Sui gas coin transaction helpers.

use crate::{move_boundary, sui, types::NexusObjects};

/// Move SUI from the transaction gas coin into an address balance.
///
/// The selected gas coin must cover both `amount_mist` and the transaction gas
/// budget. The remainder stays in the gas coin.
pub fn deposit_sui_to_address_balance(
    objects: &NexusObjects,
    amount_mist: u64,
    recipient: sui::types::Address,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    move_boundary::ptb(objects, |transaction| {
        let amount = transaction.arg(&amount_mist)?;
        let gas = transaction.gas();
        let split = transaction.split_coins(gas, vec![amount])?;
        let coin = transaction.nested_result(split, 0)?;
        transaction.send_sui_to_address_balance(coin, recipient)?;
        Ok(())
    })
}
