mod gas_add_budget;
mod gas_expiry_buy_ticket;
mod gas_expiry_disable;
mod gas_expiry_enable;

use {
    crate::prelude::*,
    gas_add_budget::*,
    gas_expiry_buy_ticket::*,
    gas_expiry_disable::*,
    gas_expiry_enable::*,
};

#[derive(Subcommand)]
pub(crate) enum GasCommand {
    #[command(about = "Add a SUI coin as gas budget")]
    AddBudget {
        #[arg(
            long = "coin",
            short = 'c',
            help = "Owned SUI coin object ID to use as budget",
            value_name = "OBJECT_ID"
        )]
        coin: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(subcommand, about = "Manage the expiry gas ticket extension")]
    Expiry(ExpiryCommand),
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
        owner_cap: sui::ObjectID,
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
        owner_cap: sui::ObjectID,
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
            long = "duration",
            short = 'd',
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
        coin: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Handle the provided gas command. The [GasCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: GasCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus gas add-budget` ==
        GasCommand::AddBudget { coin, gas } => {
            add_gas_budget(coin, gas.sui_gas_coin, gas.sui_gas_budget).await
        }

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
    }
}
