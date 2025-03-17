//! Defining a simple enumeration of all possible Sui networks.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuiNet {
    #[default]
    Localnet,
    Devnet,
    Testnet,
    Mainnet,
}
