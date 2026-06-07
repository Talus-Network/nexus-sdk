mod priority;
mod tickets;

use {
    crate::prelude::*,
    priority::*,
    tickets::{expiry::*, limited_invocations::*},
};

#[derive(Subcommand)]
pub(crate) enum GasCommand {
    #[command(subcommand, about = "Manage the expiry gas ticket extension")]
    Expiry(ExpiryCommand),

    #[command(
        subcommand,
        about = "Manage the limited invocations gas ticket extension"
    )]
    LimitedInvocations(LimitedInvocationsCommand),

    #[command(about = "Configure the priority fee vault exchange rate and TAP agent")]
    ConfigurePriorityFeeVault {
        #[arg(
            long = "exchange-rate",
            help = "$US units per SUI unit",
            value_name = "RATE"
        )]
        exchange_rate: u64,
        #[arg(
            long = "tap-agent-operator",
            help = "Operator address for the vault embedded TAP agent",
            value_name = "ADDRESS"
        )]
        tap_agent_operator: sui::types::Address,
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

    #[command(
        about = "Drain all currently available SUI from the priority fee vault using an owned `$US` coin"
    )]
    DrainPriorityFeeVaultSui {
        #[arg(
            long = "us-coin",
            help = "Owned Coin<US> object ID used to buy all currently available vault SUI",
            value_name = "OBJECT_ID"
        )]
        us_coin: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Withdraw a leader's `$US` priority-fee share")]
    WithdrawPriorityFee {
        #[arg(
            long = "leader-cap",
            help = "Leader cap object ID",
            value_name = "OBJECT_ID"
        )]
        leader_cap: sui::types::Address,
        #[arg(
            long = "share-to-withdraw",
            help = "SUI-denominated leader share to withdraw; defaults to the leader's full vault share",
            value_name = "SHARE"
        )]
        share_to_withdraw: Option<u64>,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum ExpiryCommand {
    #[command(about = "Enable the expiry gas ticket extension")]
    Enable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "The OwnerCap<OverGas> object ID that must be owned by the sender.",
            value_name = "OBJECT_ID"
        )]
        owner_cap: Option<sui::types::Address>,
        #[arg(
            long = "cost-per-minute",
            short = 'c',
            help = "The cost per minute in MIST.",
            value_name = "MIST"
        )]
        cost_per_minute: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Disable the expiry gas ticket extension")]
    Disable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "The OwnerCap<OverGas> object ID that must be owned by the sender.",
            value_name = "OBJECT_ID"
        )]
        owner_cap: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Buy an expiry gas ticket for the specified tool")]
    BuyTicket {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "minutes",
            short = 'm',
            help = "The duration of the ticket in minutes.",
            value_name = "MINUTES"
        )]
        minutes: u64,
        #[arg(
            long = "coin",
            short = 'c',
            help = "Owned SUI coin object ID to use to pay for the ticket",
            value_name = "OBJECT_ID"
        )]
        coin: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum LimitedInvocationsCommand {
    #[command(about = "Enable the limited invocations gas ticket extension")]
    Enable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "The OwnerCap<OverGas> object ID that must be owned by the sender.",
            value_name = "OBJECT_ID"
        )]
        owner_cap: Option<sui::types::Address>,
        #[arg(
            long = "cost-per-invocation",
            short = 'c',
            help = "The cost per invocation in MIST.",
            value_name = "MIST"
        )]
        cost_per_invocation: u64,
        #[arg(
            long = "min-invocations",
            help = "The minimum number of invocations required for a ticket.",
            value_name = "COUNT"
        )]
        min_invocations: u64,
        #[arg(
            long = "max-invocations",
            help = "The maximum number of invocations allowed for a ticket.",
            value_name = "COUNT"
        )]
        max_invocations: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Disable the limited invocations gas ticket extension")]
    Disable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "The OwnerCap<OverGas> object ID that must be owned by the sender.",
            value_name = "OBJECT_ID"
        )]
        owner_cap: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Buy a limited invocations gas ticket for the specified tool")]
    BuyTicket {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "invocations",
            short = 'i',
            help = "The number of invocations the ticket should cover.",
            value_name = "COUNT"
        )]
        invocations: u64,
        #[arg(
            long = "coin",
            short = 'c',
            help = "Owned SUI coin object ID to use to pay for the ticket",
            value_name = "OBJECT_ID"
        )]
        coin: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Handle the provided gas command. The [GasCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: GasCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus gas expiry` ==
        GasCommand::Expiry(command) => match command {
            // == `$ nexus gas expiry enable` ==
            ExpiryCommand::Enable {
                tool_fqn,
                owner_cap,
                cost_per_minute,
                gas,
            } => {
                enable_expiry_extension(
                    tool_fqn,
                    owner_cap,
                    cost_per_minute,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }

            // == `$ nexus gas expiry disable` ==
            ExpiryCommand::Disable {
                tool_fqn,
                owner_cap,
                gas,
            } => {
                disable_expiry_extension(tool_fqn, owner_cap, gas.sui_gas_coin, gas.sui_gas_budget)
                    .await
            }

            // == `$ nexus gas expiry buy-ticket` ==
            ExpiryCommand::BuyTicket {
                tool_fqn,
                minutes,
                coin,
                gas,
            } => {
                buy_expiry_gas_ticket(
                    tool_fqn,
                    minutes,
                    coin,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }
        },

        // == `$ nexus gas limited-invocations` ==
        GasCommand::LimitedInvocations(command) => match command {
            // == `$ nexus gas limited-invocations enable` ==
            LimitedInvocationsCommand::Enable {
                tool_fqn,
                owner_cap,
                cost_per_invocation,
                min_invocations,
                max_invocations,
                gas,
            } => {
                enable_limited_invocations_extension(
                    tool_fqn,
                    owner_cap,
                    cost_per_invocation,
                    min_invocations,
                    max_invocations,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }

            // == `$ nexus gas limited-invocations disable` ==
            LimitedInvocationsCommand::Disable {
                tool_fqn,
                owner_cap,
                gas,
            } => {
                disable_limited_invocations_extension(
                    tool_fqn,
                    owner_cap,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }

            // == `$ nexus gas limited-invocations buy-ticket` ==
            LimitedInvocationsCommand::BuyTicket {
                tool_fqn,
                invocations,
                coin,
                gas,
            } => {
                buy_limited_invocations_gas_ticket(
                    tool_fqn,
                    invocations,
                    coin,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }
        },

        GasCommand::ConfigurePriorityFeeVault {
            exchange_rate,
            tap_agent_operator,
            gas,
        } => {
            configure_priority_fee_vault(
                exchange_rate,
                tap_agent_operator,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        GasCommand::SwapUsForSui {
            us_coin,
            min_sui_out,
            gas,
        } => swap_us_for_sui(us_coin, min_sui_out, gas.sui_gas_coin, gas.sui_gas_budget).await,

        GasCommand::DrainPriorityFeeVaultSui { us_coin, gas } => {
            drain_priority_fee_vault_sui(us_coin, gas.sui_gas_coin, gas.sui_gas_budget).await
        }

        GasCommand::WithdrawPriorityFee {
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
    fn parses_drain_priority_fee_vault_sui_command() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "gas",
            "drain-priority-fee-vault-sui",
            "--us-coin",
            "0x2",
            "--sui-gas-budget",
            "12345",
        ])
        .expect("drain command should parse");

        let crate::Command::Gas(GasCommand::DrainPriorityFeeVaultSui { us_coin, gas }) =
            cli.command
        else {
            panic!("expected gas drain command");
        };

        assert_eq!(us_coin, sui::types::Address::from_static("0x2"));
        assert_eq!(gas.sui_gas_budget, 12345);
    }
}
