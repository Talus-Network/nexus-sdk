//! SDK projections for the generated registry-owned priority-fee vault.

use crate::{
    move_bindings::{registry::priority_fee_vault::PriorityFeeVault, sui_framework::object::ID},
    sui,
    types::{PriorityFeeSuiDrainQuote, PriorityFeeWithdrawalQuote},
};

impl PriorityFeeVault {
    pub fn leader_share(&self, leader_cap_id: sui::types::Address) -> Option<u64> {
        self.leader_accounts
            .get(&ID::new(leader_cap_id))
            .map(|account| account.share)
    }

    pub fn quote_withdrawal(&self, share_to_withdraw: u64) -> Option<PriorityFeeWithdrawalQuote> {
        let sui_balance = self.sui_balance.value;
        let us_balance = self.us_balance.value;
        if sui_balance != 0
            || us_balance == 0
            || self.total_share == 0
            || share_to_withdraw == 0
            || share_to_withdraw > self.total_share
        {
            return None;
        }

        let us_out = if share_to_withdraw == self.total_share {
            us_balance
        } else {
            let us_out = (u128::from(us_balance) * u128::from(share_to_withdraw)
                / u128::from(self.total_share)) as u64;
            if us_out == 0 {
                return None;
            }
            us_out
        };

        Some(PriorityFeeWithdrawalQuote {
            share_to_withdraw,
            us_out,
            us_refunded: us_balance.checked_sub(us_out)?,
        })
    }

    pub fn quote_leader_withdrawal(
        &self,
        leader_cap_id: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Option<PriorityFeeWithdrawalQuote> {
        if share_to_withdraw > self.leader_share(leader_cap_id)? {
            return None;
        }
        self.quote_withdrawal(share_to_withdraw)
    }

    pub fn quote_sui_drain(&self) -> Option<PriorityFeeSuiDrainQuote> {
        if self.exchange_rate == 0 || self.sui_balance.value == 0 {
            return None;
        }
        Some(PriorityFeeSuiDrainQuote {
            exchange_rate: self.exchange_rate,
            sui_out: self.sui_balance.value,
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::move_bindings::{
            registry::priority_fee_vault::PriorityFeeAccount,
            sui_framework::{
                balance::Balance,
                object::UID,
                sui::SUI,
                vec_map::{Entry, VecMap},
            },
            talus::us::US,
        },
    };

    fn vault(
        sui_balance: u64,
        us_balance: u64,
        exchange_rate: u64,
        total_share: u64,
    ) -> PriorityFeeVault {
        PriorityFeeVault::new(
            UID::new(sui::types::Address::from_static("0x1")),
            Balance::<SUI>::new(sui_balance),
            Balance::<US>::new(us_balance),
            exchange_rate,
            total_share,
            VecMap::new(vec![Entry::new(
                ID::new(sui::types::Address::from_static("0x42")),
                PriorityFeeAccount::new(10),
            )]),
        )
    }

    #[test]
    fn generated_vault_quotes_leader_withdrawal() {
        let state = vault(0, 90, 3, 30);

        assert_eq!(
            state.quote_leader_withdrawal(sui::types::Address::from_static("0x42"), 10),
            Some(PriorityFeeWithdrawalQuote {
                share_to_withdraw: 10,
                us_out: 30,
                us_refunded: 60,
            })
        );
    }

    #[test]
    fn generated_vault_quotes_sui_drain() {
        assert_eq!(
            vault(123, 0, 10, 123).quote_sui_drain(),
            Some(PriorityFeeSuiDrainQuote {
                exchange_rate: 10,
                sui_out: 123,
            })
        );
        assert_eq!(vault(123, 0, 0, 123).quote_sui_drain(), None);
    }
}
