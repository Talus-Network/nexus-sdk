use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    std::num::NonZeroU64,
};

/// Commands for configuring and buying tool payment tickets.
#[derive(Subcommand)]
pub(crate) enum CashierCommand {
    #[command(subcommand, about = "Manage expiry payment tickets")]
    Expiry(ExpiryTicketCommand),

    #[command(subcommand, about = "Manage payment tickets with invocation limits")]
    LimitedInvocations(LimitedInvocationsTicketCommand),
}

/// Commands for time limited tool payment tickets.
#[derive(Subcommand)]
pub(crate) enum ExpiryTicketCommand {
    #[command(about = "Enable expiry payment tickets for a tool")]
    Enable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "cashier-admin",
            short = 'a',
            help = "Tool cashier admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        cashier_admin: Option<sui::types::Address>,
        #[arg(
            long = "cost-per-minute",
            short = 'c',
            help = "Ticket cost per minute in MIST",
            value_name = "MIST"
        )]
        cost_per_minute: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Disable expiry payment tickets for a tool")]
    Disable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "cashier-admin",
            short = 'a',
            help = "Tool cashier admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        cashier_admin: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Buy an expiry payment ticket for a tool")]
    BuyTicket {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long,
            short = 'm',
            help = "Positive ticket duration in minutes",
            value_name = "MINUTES"
        )]
        minutes: NonZeroU64,
        #[arg(
            long = "payment-coin",
            short = 'c',
            help = "Owned Coin<SUI> object used to pay for the ticket",
            value_name = "OBJECT_ID"
        )]
        payment_coin: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Commands for invocation limited tool payment tickets.
#[derive(Subcommand)]
pub(crate) enum LimitedInvocationsTicketCommand {
    #[command(about = "Enable payment tickets with invocation limits for a tool")]
    Enable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "cashier-admin",
            short = 'a',
            help = "Tool cashier admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        cashier_admin: Option<sui::types::Address>,
        #[arg(
            long = "cost-per-invocation",
            short = 'c',
            help = "Ticket cost per invocation in MIST",
            value_name = "MIST"
        )]
        cost_per_invocation: u64,
        #[arg(
            long = "min-invocations",
            help = "Minimum invocations purchasable in one ticket",
            value_name = "COUNT"
        )]
        min_invocations: u64,
        #[arg(
            long = "max-invocations",
            help = "Maximum invocations purchasable in one ticket",
            value_name = "COUNT"
        )]
        max_invocations: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Disable payment tickets with invocation limits for a tool")]
    Disable {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "cashier-admin",
            short = 'a',
            help = "Tool cashier admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        cashier_admin: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Buy a payment ticket with an invocation limit for a tool")]
    BuyTicket {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long,
            short = 'i',
            help = "Positive number of invocations to purchase",
            value_name = "COUNT"
        )]
        invocations: NonZeroU64,
        #[arg(
            long = "payment-coin",
            short = 'c',
            help = "Owned Coin<SUI> object used to pay for the ticket",
            value_name = "OBJECT_ID"
        )]
        payment_coin: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

async fn cashier_admin(
    tool_fqn: &ToolFqn,
    explicit: Option<sui::types::Address>,
) -> AnyResult<sui::types::Address, NexusCliError> {
    if let Some(cashier_admin) = explicit {
        return Ok(cashier_admin);
    }

    let conf = CliConf::load().await.unwrap_or_default();
    conf.tools
        .get(tool_fqn)
        .and_then(|tool| tool.cashier_admin)
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No tool cashier admin capability was provided for tool '{tool_fqn}'. Pass --cashier-admin or register the tool with this CLI first."
            ))
        })
}

fn validate_payment_coin(
    payment_coin: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
) -> AnyResult<(), NexusCliError> {
    if Some(payment_coin) == sui_gas_coin {
        return Err(NexusCliError::Any(anyhow!(
            "Payment coin '{payment_coin}' cannot also be the Sui gas coin. Use another coin or address balance gas."
        )));
    }
    Ok(())
}

fn emit_result(
    action: &str,
    tool_fqn: &ToolFqn,
    digest: &sui::types::Digest,
) -> AnyResult<(), NexusCliError> {
    notify_success!(
        "Transaction digest: {digest}",
        digest = digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({
        "action": action,
        "tool_fqn": tool_fqn,
        "digest": digest,
    }))?;
    Ok(())
}

async fn enable_expiry(
    tool_fqn: ToolFqn,
    explicit_cashier_admin: Option<sui::types::Address>,
    cost_per_minute: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Enabling expiry payment tickets for tool '{tool_fqn}'");
    let cashier_admin = cashier_admin(&tool_fqn, explicit_cashier_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .enable_expiry_tickets(&tool_fqn, cashier_admin, cost_per_minute)
        .await
    {
        Ok(response) => {
            progress.success();
            response
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    emit_result("enable_expiry_tickets", &tool_fqn, &response.tx_digest)
}

async fn disable_expiry(
    tool_fqn: ToolFqn,
    explicit_cashier_admin: Option<sui::types::Address>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Disabling expiry payment tickets for tool '{tool_fqn}'");
    let cashier_admin = cashier_admin(&tool_fqn, explicit_cashier_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .disable_expiry_tickets(&tool_fqn, cashier_admin)
        .await
    {
        Ok(response) => {
            progress.success();
            response
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    emit_result("disable_expiry_tickets", &tool_fqn, &response.tx_digest)
}

async fn buy_expiry(
    tool_fqn: ToolFqn,
    minutes: NonZeroU64,
    payment_coin: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Buying an expiry payment ticket for tool '{tool_fqn}'");
    validate_payment_coin(payment_coin, gas.sui_gas_coin)?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .buy_expiry_ticket(&tool_fqn, minutes.get(), payment_coin)
        .await
    {
        Ok(response) => {
            progress.success();
            response
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    emit_result("buy_expiry_ticket", &tool_fqn, &response.tx_digest)
}

async fn enable_limited_invocations(
    tool_fqn: ToolFqn,
    explicit_cashier_admin: Option<sui::types::Address>,
    cost_per_invocation: u64,
    min_invocations: u64,
    max_invocations: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    if min_invocations > max_invocations {
        return Err(NexusCliError::Any(anyhow!(
            "Minimum invocations '{min_invocations}' cannot exceed maximum invocations '{max_invocations}'."
        )));
    }
    command_title!("Enabling invocation limited payment tickets for tool '{tool_fqn}'");
    let cashier_admin = cashier_admin(&tool_fqn, explicit_cashier_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .enable_limited_invocation_tickets(
            &tool_fqn,
            cashier_admin,
            cost_per_invocation,
            min_invocations,
            max_invocations,
        )
        .await
    {
        Ok(response) => {
            progress.success();
            response
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    emit_result(
        "enable_limited_invocation_tickets",
        &tool_fqn,
        &response.tx_digest,
    )
}

async fn disable_limited_invocations(
    tool_fqn: ToolFqn,
    explicit_cashier_admin: Option<sui::types::Address>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Disabling invocation limited payment tickets for tool '{tool_fqn}'");
    let cashier_admin = cashier_admin(&tool_fqn, explicit_cashier_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .disable_limited_invocation_tickets(&tool_fqn, cashier_admin)
        .await
    {
        Ok(response) => {
            progress.success();
            response
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    emit_result(
        "disable_limited_invocation_tickets",
        &tool_fqn,
        &response.tx_digest,
    )
}

async fn buy_limited_invocations(
    tool_fqn: ToolFqn,
    invocations: NonZeroU64,
    payment_coin: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Buying an invocation limited payment ticket for tool '{tool_fqn}'");
    validate_payment_coin(payment_coin, gas.sui_gas_coin)?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .buy_limited_invocation_ticket(&tool_fqn, invocations.get(), payment_coin)
        .await
    {
        Ok(response) => {
            progress.success();
            response
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    emit_result(
        "buy_limited_invocation_ticket",
        &tool_fqn,
        &response.tx_digest,
    )
}

pub(crate) async fn handle_cashier(command: CashierCommand) -> AnyResult<(), NexusCliError> {
    match command {
        CashierCommand::Expiry(command) => match command {
            ExpiryTicketCommand::Enable {
                tool_fqn,
                cashier_admin,
                cost_per_minute,
                gas,
            } => enable_expiry(tool_fqn, cashier_admin, cost_per_minute, gas).await,
            ExpiryTicketCommand::Disable {
                tool_fqn,
                cashier_admin,
                gas,
            } => disable_expiry(tool_fqn, cashier_admin, gas).await,
            ExpiryTicketCommand::BuyTicket {
                tool_fqn,
                minutes,
                payment_coin,
                gas,
            } => buy_expiry(tool_fqn, minutes, payment_coin, gas).await,
        },
        CashierCommand::LimitedInvocations(command) => match command {
            LimitedInvocationsTicketCommand::Enable {
                tool_fqn,
                cashier_admin,
                cost_per_invocation,
                min_invocations,
                max_invocations,
                gas,
            } => {
                enable_limited_invocations(
                    tool_fqn,
                    cashier_admin,
                    cost_per_invocation,
                    min_invocations,
                    max_invocations,
                    gas,
                )
                .await
            }
            LimitedInvocationsTicketCommand::Disable {
                tool_fqn,
                cashier_admin,
                gas,
            } => disable_limited_invocations(tool_fqn, cashier_admin, gas).await,
            LimitedInvocationsTicketCommand::BuyTicket {
                tool_fqn,
                invocations,
                payment_coin,
                gas,
            } => buy_limited_invocations(tool_fqn, invocations, payment_coin, gas).await,
        },
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn cashier_commands_use_the_tool_namespace() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "expiry",
            "enable",
            "--tool-fqn",
            "com.example.tool@1",
            "--cashier-admin",
            "0x1",
            "--cost-per-minute",
            "10",
        ])
        .expect("tool cashier command should parse");

        assert!(matches!(
            cli.command,
            crate::Command::Tool(super::super::ToolCommand::Cashier(CashierCommand::Expiry(
                ExpiryTicketCommand::Enable { .. }
            )))
        ));
    }

    #[test]
    fn ticket_quantity_must_be_positive() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "expiry",
            "buy-ticket",
            "--tool-fqn",
            "com.example.tool@1",
            "--minutes",
            "0",
            "--payment-coin",
            "0x1",
        ])
        .is_err());
    }
}
