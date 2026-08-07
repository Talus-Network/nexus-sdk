mod actions;

use {crate::prelude::*, actions::*};

/// Nexus network fee commands.
#[derive(Subcommand)]
pub(crate) enum NetworkCommand {
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
}

pub(crate) async fn handle(command: NetworkCommand) -> AnyResult<(), NexusCliError> {
    match command {
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
}
