use {
    crate::{
        sui,
        types::{deserialize_sui_u64, strip_fields_owned},
    },
    serde::{Deserialize, Deserializer},
    serde_json::Value,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityFeeAccount {
    pub share: u64,
}

#[derive(Deserialize)]
struct PriorityFeeAccountRaw {
    #[serde(deserialize_with = "deserialize_sui_u64")]
    share: u64,
}

impl<'de> Deserialize<'de> for PriorityFeeAccount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return PriorityFeeAccountRaw::deserialize(deserializer)
                .map(|raw| Self { share: raw.share });
        }

        let value = strip_fields_owned(Value::deserialize(deserializer)?);
        serde_json::from_value::<PriorityFeeAccountRaw>(value)
            .map(|raw| Self { share: raw.share })
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityFeeVaultState {
    pub sui_balance: u64,
    pub us_balance: u64,
    pub exchange_rate: u64,
    pub total_share: u64,
    pub leader_accounts: Vec<(sui::types::Address, PriorityFeeAccount)>,
    pub tap_agent_registered: bool,
    pub tap_agent_operator: sui::types::Address,
}

#[derive(Deserialize)]
struct PriorityFeeVaultStateRaw {
    #[serde(deserialize_with = "deserialize_balance_u64")]
    sui_balance: u64,
    #[serde(deserialize_with = "deserialize_balance_u64")]
    us_balance: u64,
    #[serde(deserialize_with = "deserialize_sui_u64")]
    exchange_rate: u64,
    #[serde(deserialize_with = "deserialize_sui_u64")]
    total_share: u64,
    #[serde(deserialize_with = "deserialize_leader_accounts")]
    leader_accounts: Vec<(sui::types::Address, PriorityFeeAccount)>,
    #[serde(default)]
    tap_agent_registered: bool,
    #[serde(default = "zero_address")]
    tap_agent_operator: sui::types::Address,
}

impl From<PriorityFeeVaultStateRaw> for PriorityFeeVaultState {
    fn from(raw: PriorityFeeVaultStateRaw) -> Self {
        Self {
            sui_balance: raw.sui_balance,
            us_balance: raw.us_balance,
            exchange_rate: raw.exchange_rate,
            total_share: raw.total_share,
            leader_accounts: raw.leader_accounts,
            tap_agent_registered: raw.tap_agent_registered,
            tap_agent_operator: raw.tap_agent_operator,
        }
    }
}

impl<'de> Deserialize<'de> for PriorityFeeVaultState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return PriorityFeeVaultStateRaw::deserialize(deserializer).map(Self::from);
        }

        let value = strip_fields_owned(Value::deserialize(deserializer)?);
        serde_json::from_value::<PriorityFeeVaultStateRaw>(value)
            .map(Self::from)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityFeeWithdrawalQuote {
    pub share_to_withdraw: u64,
    pub us_out: u64,
    pub us_refunded: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorityFeeSuiDrainQuote {
    pub exchange_rate: u64,
    pub sui_out: u64,
}

impl PriorityFeeVaultState {
    pub fn leader_share(&self, leader_cap_id: sui::types::Address) -> Option<u64> {
        self.leader_accounts
            .iter()
            .find_map(|(account, value)| (*account == leader_cap_id).then_some(value.share))
    }

    pub fn quote_withdrawal(&self, share_to_withdraw: u64) -> Option<PriorityFeeWithdrawalQuote> {
        if self.sui_balance != 0
            || self.us_balance == 0
            || self.total_share == 0
            || share_to_withdraw == 0
            || share_to_withdraw > self.total_share
        {
            return None;
        }

        let us_out = if share_to_withdraw == self.total_share {
            self.us_balance
        } else {
            let us_out = (u128::from(self.us_balance) * u128::from(share_to_withdraw)
                / u128::from(self.total_share)) as u64;
            if us_out == 0 {
                return None;
            }
            us_out
        };

        Some(PriorityFeeWithdrawalQuote {
            share_to_withdraw,
            us_out,
            us_refunded: self.us_balance.checked_sub(us_out)?,
        })
    }

    pub fn quote_leader_withdrawal(
        &self,
        leader_cap_id: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Option<PriorityFeeWithdrawalQuote> {
        let leader_share = self.leader_share(leader_cap_id)?;
        if share_to_withdraw > leader_share {
            return None;
        }
        self.quote_withdrawal(share_to_withdraw)
    }

    pub fn quote_sui_drain(&self) -> Option<PriorityFeeSuiDrainQuote> {
        if self.exchange_rate == 0 || self.sui_balance == 0 {
            return None;
        }
        Some(PriorityFeeSuiDrainQuote {
            exchange_rate: self.exchange_rate,
            sui_out: self.sui_balance,
        })
    }
}

fn deserialize_balance_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return deserialize_sui_u64(deserializer);
    }

    let value = Value::deserialize(deserializer)?;
    parse_move_u64(value).map_err(serde::de::Error::custom)
}

fn zero_address() -> sui::types::Address {
    sui::types::Address::ZERO
}

fn deserialize_leader_accounts<'de, D>(
    deserializer: D,
) -> Result<Vec<(sui::types::Address, PriorityFeeAccount)>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct VecMap<K, V> {
        contents: Vec<VecMapEntry<K, V>>,
    }

    #[derive(Deserialize)]
    struct VecMapEntry<K, V> {
        key: K,
        value: V,
    }

    if !deserializer.is_human_readable() {
        let accounts =
            VecMap::<sui::types::Address, PriorityFeeAccount>::deserialize(deserializer)?;
        return Ok(accounts
            .contents
            .into_iter()
            .map(|entry| (entry.key, entry.value))
            .collect());
    }

    let value = strip_fields_owned(Value::deserialize(deserializer)?);
    let contents = match value {
        Value::Object(mut object) => object
            .remove("contents")
            .or_else(|| object.remove("vec"))
            .unwrap_or(Value::Array(Vec::new())),
        Value::Array(contents) => Value::Array(contents),
        other => {
            return Err(serde::de::Error::custom(format!(
                "expected VecMap: {other}"
            )))
        }
    };

    let Value::Array(entries) = contents else {
        return Err(serde::de::Error::custom("expected VecMap contents array"));
    };

    entries
        .into_iter()
        .map(|entry| {
            let entry = strip_fields_owned(entry);
            serde_json::from_value::<VecMapEntry<sui::types::Address, PriorityFeeAccount>>(entry)
                .map(|entry| (entry.key, entry.value))
                .map_err(serde::de::Error::custom)
        })
        .collect()
}

fn parse_move_u64(value: Value) -> Result<u64, String> {
    match strip_fields_owned(value) {
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| "expected unsigned integer".to_string()),
        Value::String(value) => value
            .parse::<u64>()
            .map_err(|err| format!("invalid u64 value: {err}")),
        Value::Object(mut object) => {
            for key in ["value", "balance", "inner"] {
                if let Some(value) = object.remove(key) {
                    return parse_move_u64(value);
                }
            }
            Err("expected u64-compatible Move JSON value".to_string())
        }
        other => Err(format!("expected u64-compatible Move JSON value: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_fee_vault_state_decodes_move_json_and_quotes() {
        let leader = sui::types::Address::from_static("0x42");
        let state: PriorityFeeVaultState = serde_json::from_value(serde_json::json!({
            "fields": {
                "sui_balance": { "fields": { "value": "0" } },
                "us_balance": { "fields": { "value": "90" } },
                "exchange_rate": "3",
                "total_share": "30",
                "leader_accounts": {
                    "fields": {
                        "contents": [
                            {
                                "fields": {
                                    "key": "0x42",
                                    "value": { "fields": { "share": "10" } }
                                }
                            }
                        ]
                    }
                },
                "tap_agent_registered": true,
                "tap_agent_operator": "0x7"
            }
        }))
        .expect("vault JSON decodes");

        assert_eq!(state.leader_share(leader), Some(10));
        assert_eq!(
            state.quote_leader_withdrawal(leader, 10),
            Some(PriorityFeeWithdrawalQuote {
                share_to_withdraw: 10,
                us_out: 30,
                us_refunded: 60,
            })
        );
        assert_eq!(
            state.quote_sui_drain(),
            None,
            "no SUI liquidity means no public-drain quote"
        );
    }

    #[test]
    fn priority_fee_vault_state_quotes_sui_drain_when_rate_and_liquidity_exist() {
        let state = PriorityFeeVaultState {
            sui_balance: 123,
            us_balance: 0,
            exchange_rate: 10,
            total_share: 123,
            leader_accounts: Vec::new(),
            tap_agent_registered: true,
            tap_agent_operator: sui::types::Address::from_static("0x7"),
        };

        assert_eq!(
            state.quote_sui_drain(),
            Some(PriorityFeeSuiDrainQuote {
                exchange_rate: 10,
                sui_out: 123,
            })
        );
    }

    #[test]
    fn priority_fee_vault_state_rejects_sui_drain_without_rate() {
        let state = PriorityFeeVaultState {
            sui_balance: 123,
            us_balance: 0,
            exchange_rate: 0,
            total_share: 123,
            leader_accounts: Vec::new(),
            tap_agent_registered: false,
            tap_agent_operator: sui::types::Address::ZERO,
        };

        assert_eq!(state.quote_sui_drain(), None);
    }
}
