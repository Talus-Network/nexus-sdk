//! Off-chain quote values derived from the generated priority-fee vault state.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityFeeWithdrawalQuote {
    pub share_to_withdraw: u64,
    pub us_out: u64,
    pub us_refunded: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityFeeSuiDrainQuote {
    pub exchange_rate_sui_us: u64,
    pub sui_out: u64,
}
