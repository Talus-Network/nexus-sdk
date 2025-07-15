mod gas_add_budget;
mod tickets;

use {
    crate::prelude::*,
    gas_add_budget::*,
    tickets::{expiry::*, limited_invocations::*},
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

    #[command(
        subcommand,
        about = "Manage the limited invocations gas ticket extension"
    )]
    LimitedInvocations(LimitedInvocationsCommand),
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
        owner_cap: Option<sui::ObjectID>,
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
        owner_cap: Option<sui::ObjectID>,
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
        coin: sui::ObjectID,
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
        owner_cap: Option<sui::ObjectID>,
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
        owner_cap: Option<sui::ObjectID>,
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
        coin: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Handle the provided gas command. The [GasCommand] instance is passed from
/// [crate::main].
///
/// This function is marked as `#[cfg_attr(coverage_nightly, coverage(off))]` to
/// disable coverage for the entire function. This is because `nexus-sdk`
/// is not yet fully covered, and we don't want to fail the CI check.
///
/// TODO: Remove this once tests are added for this function.
#[cfg_attr(coverage_nightly, coverage(off))]
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
    }
}
