mod actions;

use {crate::prelude::*, actions::*};

fn parse_priority_fee_batch_size(value: &str) -> Result<usize, String> {
    let value = value
        .parse::<usize>()
        .map_err(|error| format!("invalid batch size '{value}': {error}"))?;
    if !(1..=nexus_sdk::nexus::network::MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE).contains(&value) {
        return Err(format!(
            "batch size must be in 1..={}",
            nexus_sdk::nexus::network::MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE
        ));
    }
    Ok(value)
}

/// Nexus network administration and fee commands.
#[derive(Subcommand)]
pub(crate) enum NetworkCommand {
    #[command(about = "Configure the complete leader registry policy")]
    ConfigureLeaderRegistry {
        #[arg(
            long = "unbonding-duration-ms",
            help = "Delay before requested leader stake can be claimed",
            value_name = "MILLISECONDS"
        )]
        unbonding_duration_ms: u64,
        #[arg(
            long = "min-stake-us",
            help = "Minimum leader stake in `$US` atomic units",
            value_name = "AMOUNT"
        )]
        min_stake_us: u64,
        #[arg(
            long = "max-transaction-budget-mist",
            help = "Maximum transaction budget available to one leader",
            value_name = "MIST"
        )]
        max_transaction_budget_mist: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Configure the priority fee vault exchange rate")]
    ConfigurePriorityFeeVault {
        #[arg(
            long = "exchange-rate-million-mists-us",
            help = "$US atomic units per 1,000,000 MIST",
            value_name = "RATE"
        )]
        exchange_rate_million_mists_us: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Swap an owned `$US` coin for SUI from the priority fee vault")]
    SwapUsForSui {
        #[arg(
            long = "us-coin",
            help = "Owned Coin<US> object ID",
            value_name = "OBJECT_ID"
        )]
        us_coin: sui::types::Address,
        #[arg(
            long = "min-sui-out",
            help = "Minimum SUI output accepted",
            value_name = "MIST",
            default_value_t = 0
        )]
        min_sui_out: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Drain all available SUI from the priority fee vault")]
    DrainPriorityFeeVaultSui {
        #[arg(
            long = "us-coin",
            help = "Owned Coin<US> object ID used to buy all available vault SUI",
            value_name = "OBJECT_ID"
        )]
        us_coin: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Withdraw a leader's `$US` priority fee share")]
    WithdrawPriorityFee {
        #[arg(
            long = "leader-cap",
            help = "Leader capability object ID",
            value_name = "OBJECT_ID"
        )]
        leader_cap: sui::types::Address,
        #[arg(
            long = "share-to-withdraw",
            help = "SUI denominated share to withdraw; defaults to the full leader share",
            value_name = "SHARE"
        )]
        share_to_withdraw: Option<u64>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Collect one leader capability's priority fee deposits")]
    CollectPriorityFees {
        #[arg(
            long = "leader-cap-id",
            help = "Leader capability whose initially visible deposits will be collected",
            value_name = "OBJECT_ID"
        )]
        leader_cap_id: sui::types::Address,
        #[arg(
            long = "batch-size",
            help = "Maximum deposits per transaction",
            value_name = "COUNT",
            default_value_t = nexus_sdk::nexus::network::MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE,
            value_parser = parse_priority_fee_batch_size
        )]
        batch_size: usize,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: NetworkCommand) -> AnyResult<(), NexusCliError> {
    match command {
        NetworkCommand::ConfigureLeaderRegistry {
            unbonding_duration_ms,
            min_stake_us,
            max_transaction_budget_mist,
            gas,
        } => {
            configure_leader_registry(
                unbonding_duration_ms,
                min_stake_us,
                max_transaction_budget_mist,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        NetworkCommand::ConfigurePriorityFeeVault {
            exchange_rate_million_mists_us,
            gas,
        } => {
            configure_priority_fee_vault(
                exchange_rate_million_mists_us,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        NetworkCommand::SwapUsForSui {
            us_coin,
            min_sui_out,
            gas,
        } => swap_us_for_sui(us_coin, min_sui_out, gas.sui_gas_coin, gas.sui_gas_budget).await,
        NetworkCommand::DrainPriorityFeeVaultSui { us_coin, gas } => {
            drain_priority_fee_vault_sui(us_coin, gas.sui_gas_coin, gas.sui_gas_budget).await
        }
        NetworkCommand::WithdrawPriorityFee {
            leader_cap,
            share_to_withdraw,
            gas,
        } => {
            withdraw_priority_fee(
                leader_cap,
                share_to_withdraw,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        NetworkCommand::CollectPriorityFees {
            leader_cap_id,
            batch_size,
            gas,
        } => {
            collect_priority_fees(
                leader_cap_id,
                batch_size,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn parses_network_fee_commands() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "network",
            "configure-priority-fee-vault",
            "--exchange-rate-million-mists-us",
            "7",
        ])
        .expect("network fee configuration should parse");
        assert!(matches!(
            cli.command,
            crate::Command::Network(NetworkCommand::ConfigurePriorityFeeVault {
                exchange_rate_million_mists_us: 7,
                ..
            })
        ));
    }

    #[test]
    fn parses_complete_leader_registry_configuration() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "network",
            "configure-leader-registry",
            "--unbonding-duration-ms",
            "86400000",
            "--min-stake-us",
            "1000000000",
            "--max-transaction-budget-mist",
            "10000000000",
        ])
        .expect("complete leader registry configuration should parse");

        assert!(matches!(
            cli.command,
            crate::Command::Network(NetworkCommand::ConfigureLeaderRegistry {
                unbonding_duration_ms: 86_400_000,
                min_stake_us: 1_000_000_000,
                max_transaction_budget_mist: 10_000_000_000,
                ..
            })
        ));
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "network",
            "configure-leader-registry",
            "--min-stake-us",
            "1000000000",
        ])
        .is_err());
    }

    #[test]
    fn parses_leader_priority_fee_collection_with_default_batch_size() {
        let leader_cap_id = "0x701";
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "network",
            "collect-priority-fees",
            "--leader-cap-id",
            leader_cap_id,
        ])
        .expect("leader priority fee collection should parse");
        assert!(matches!(
            cli.command,
            crate::Command::Network(NetworkCommand::CollectPriorityFees {
                leader_cap_id: parsed_leader_cap_id,
                batch_size: nexus_sdk::nexus::network::MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE,
                ..
            }) if parsed_leader_cap_id == sui::types::Address::from_static(leader_cap_id)
        ));
    }

    #[test]
    fn priority_fee_collection_requires_a_leader_and_bounded_batch_size() {
        assert!(
            crate::Cli::try_parse_from(["nexus", "network", "collect-priority-fees",]).is_err()
        );
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "network",
            "collect-priority-fees",
            "--deposit",
            "0x701",
        ])
        .is_err());
        assert!(crate::Cli::try_parse_from(
            ["nexus", "network", "collect-priority-fees", "--all",]
        )
        .is_err());
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "network",
            "collect-priority-fees",
            "--leader-cap-id",
            "0x701",
            "--batch-size",
            "129",
        ])
        .is_err());
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "network",
            "collect-priority-fees",
            "--leader-cap-id",
            "0x701",
            "--batch-size",
            "1",
        ])
        .is_ok());
    }
}
