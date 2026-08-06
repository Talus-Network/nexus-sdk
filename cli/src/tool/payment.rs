use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    std::num::NonZeroU64,
};

/// Commands for expiry and invocation limited tool payment tickets.
#[derive(Subcommand)]
pub(crate) enum PaymentCommand {
    #[command(subcommand, about = "Manage expiry payment tickets")]
    Expiry(ExpiryPaymentCommand),

    #[command(subcommand, about = "Manage invocation limited payment tickets")]
    LimitedInvocations(LimitedInvocationsPaymentCommand),
}

/// Commands for time limited tool payment tickets.
#[derive(Subcommand)]
pub(crate) enum ExpiryPaymentCommand {
    #[command(about = "Enable expiry payment tickets for a tool")]
    Enable {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(
            long = "payment-admin",
            short = 'a',
            help = "Payment admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        payment_admin: Option<sui::types::Address>,
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
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(
            long = "payment-admin",
            short = 'a',
            help = "Payment admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        payment_admin: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Buy an expiry payment ticket for a tool")]
    BuyTicket {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
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
pub(crate) enum LimitedInvocationsPaymentCommand {
    #[command(about = "Enable invocation limited payment tickets for a tool")]
    Enable {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(
            long = "payment-admin",
            short = 'a',
            help = "Payment admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        payment_admin: Option<sui::types::Address>,
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

    #[command(about = "Disable invocation limited payment tickets for a tool")]
    Disable {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(
            long = "payment-admin",
            short = 'a',
            help = "Payment admin capability object ID. Defaults to the capability saved for this tool.",
            value_name = "OBJECT_ID"
        )]
        payment_admin: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Buy an invocation limited payment ticket for a tool")]
    BuyTicket {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
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

async fn payment_admin(
    tool_fqn: &ToolFqn,
    explicit: Option<sui::types::Address>,
) -> AnyResult<sui::types::Address, NexusCliError> {
    if let Some(payment_admin) = explicit {
        return Ok(payment_admin);
    }

    let conf = CliConf::load().await.unwrap_or_default();
    conf.tools
        .get(tool_fqn)
        .and_then(|tool| tool.payment_admin)
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No payment admin capability was provided for tool '{tool_fqn}'. Pass --payment-admin or register the Tool with this CLI first."
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
    explicit_payment_admin: Option<sui::types::Address>,
    cost_per_minute: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Enabling expiry payment tickets for tool '{tool_fqn}'");
    let payment_admin = payment_admin(&tool_fqn, explicit_payment_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .enable_expiry_tickets(&tool_fqn, payment_admin, cost_per_minute)
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
    explicit_payment_admin: Option<sui::types::Address>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Disabling expiry payment tickets for tool '{tool_fqn}'");
    let payment_admin = payment_admin(&tool_fqn, explicit_payment_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .disable_expiry_tickets(&tool_fqn, payment_admin)
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
    explicit_payment_admin: Option<sui::types::Address>,
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
    let payment_admin = payment_admin(&tool_fqn, explicit_payment_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .enable_limited_invocation_tickets(
            &tool_fqn,
            payment_admin,
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
    explicit_payment_admin: Option<sui::types::Address>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Disabling invocation limited payment tickets for tool '{tool_fqn}'");
    let payment_admin = payment_admin(&tool_fqn, explicit_payment_admin).await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Crafting and executing transaction...");
    let response = match client
        .tool()
        .disable_limited_invocation_tickets(&tool_fqn, payment_admin)
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

pub(crate) async fn handle_payment(command: PaymentCommand) -> AnyResult<(), NexusCliError> {
    match command {
        PaymentCommand::Expiry(command) => match command {
            ExpiryPaymentCommand::Enable {
                tool_fqn,
                payment_admin,
                cost_per_minute,
                gas,
            } => enable_expiry(tool_fqn, payment_admin, cost_per_minute, gas).await,
            ExpiryPaymentCommand::Disable {
                tool_fqn,
                payment_admin,
                gas,
            } => disable_expiry(tool_fqn, payment_admin, gas).await,
            ExpiryPaymentCommand::BuyTicket {
                tool_fqn,
                minutes,
                payment_coin,
                gas,
            } => buy_expiry(tool_fqn, minutes, payment_coin, gas).await,
        },
        PaymentCommand::LimitedInvocations(command) => match command {
            LimitedInvocationsPaymentCommand::Enable {
                tool_fqn,
                payment_admin,
                cost_per_invocation,
                min_invocations,
                max_invocations,
                gas,
            } => {
                enable_limited_invocations(
                    tool_fqn,
                    payment_admin,
                    cost_per_invocation,
                    min_invocations,
                    max_invocations,
                    gas,
                )
                .await
            }
            LimitedInvocationsPaymentCommand::Disable {
                tool_fqn,
                payment_admin,
                gas,
            } => disable_limited_invocations(tool_fqn, payment_admin, gas).await,
            LimitedInvocationsPaymentCommand::BuyTicket {
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
    fn payment_commands_use_the_tool_namespace() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "payment",
            "expiry",
            "enable",
            "--tool-fqn",
            "com.example.tool@1",
            "--payment-admin",
            "0x1",
            "--cost-per-minute",
            "10",
        ])
        .expect("tool payment command should parse");

        assert!(matches!(
            cli.command,
            crate::Command::Tool(super::super::ToolCommand::Payment(PaymentCommand::Expiry(
                ExpiryPaymentCommand::Enable { .. }
            )))
        ));
    }

    #[test]
    fn ticket_quantity_must_be_positive() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "payment",
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
