use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{move_bindings::interface::payment::PaymentSourceKind, nexus::tool::ToolEconomy},
    std::num::NonZeroU64,
};

/// Commands for one Tool's economic policies and entitlement products.
#[derive(Subcommand)]
pub(crate) enum CashierCommand {
    #[command(about = "Show accepted policies and canonical offers")]
    Inspect {
        #[command(flatten)]
        tool: ToolArgs,
    },

    #[command(about = "Show the beneficiary access accounts and refundable Invocations")]
    Access {
        #[command(flatten)]
        tool: ToolArgs,
        #[command(flatten)]
        beneficiary: BeneficiaryArgs,
    },

    #[command(about = "List finalized Invocations and prepaid deposits")]
    Inbox {
        #[command(flatten)]
        tool: ToolArgs,
    },

    #[command(about = "Collect one same policy batch of finalized Invocations")]
    CollectInvocations {
        #[command(flatten)]
        admin: AdminArgs,
        #[arg(
            long = "invocation-id",
            value_name = "OBJECT_ID",
            required = true,
            action = clap::ArgAction::Append,
            help = "Finalized Invocation object ID"
        )]
        invocation_ids: Vec<sui::types::Address>,
        #[arg(
            long,
            value_name = "ADDRESS",
            help = "Payout address. Uses the active signer when omitted."
        )]
        recipient: Option<sui::types::Address>,
    },

    #[command(about = "Collect one batch of prepaid sale deposits")]
    CollectDeposits {
        #[command(flatten)]
        admin: AdminArgs,
        #[arg(
            long = "deposit-id",
            value_name = "OBJECT_ID",
            required = true,
            action = clap::ArgAction::Append,
            help = "Cashier deposit object ID"
        )]
        deposit_ids: Vec<sui::types::Address>,
        #[arg(
            long,
            value_name = "ADDRESS",
            help = "Payout address. Uses the active signer when omitted."
        )]
        recipient: Option<sui::types::Address>,
    },

    #[command(subcommand, about = "Manage fixed price admission")]
    FixedPrice(PolicyToggleCommand),

    #[command(subcommand, about = "Manage sponsored free admission")]
    Free(PolicyToggleCommand),

    #[command(subcommand, about = "Manage finite invocation credits")]
    FiniteCredits(FiniteCreditCommand),

    #[command(subcommand, about = "Manage time based access passes")]
    TimePass(TimePassCommand),
}

#[derive(Args)]
pub(crate) struct ToolArgs {
    #[arg(
        long = "tool-fqn",
        short = 't',
        help = "Fully qualified Tool name",
        value_name = "FQN"
    )]
    tool_fqn: ToolFqn,
}

#[derive(Args)]
pub(crate) struct AdminArgs {
    #[command(flatten)]
    tool: ToolArgs,
    #[arg(
        long = "cashier-admin",
        short = 'a',
        help = "Cashier admin capability. Uses the saved capability when omitted.",
        value_name = "OBJECT_ID"
    )]
    cashier_admin: Option<sui::types::Address>,
    #[command(flatten)]
    gas: GasArgs,
}

#[derive(Args)]
#[group(id = "beneficiary", multiple = false)]
pub(crate) struct BeneficiaryArgs {
    #[arg(
        long,
        value_name = "ADDRESS",
        group = "beneficiary",
        help = "User funded execution source allowed to use this entitlement"
    )]
    beneficiary_user: Option<sui::types::Address>,
    #[arg(
        long,
        value_name = "AGENT_ID",
        group = "beneficiary",
        help = "Agent funded execution source allowed to use this entitlement"
    )]
    beneficiary_agent: Option<sui::types::Address>,
}

impl BeneficiaryArgs {
    fn resolve(self, default_user: sui::types::Address) -> PaymentSourceKind {
        match (self.beneficiary_user, self.beneficiary_agent) {
            (Some(user), None) => PaymentSourceKind::user_funded(user),
            (None, Some(agent)) => PaymentSourceKind::agent_funded(agent),
            (None, None) => PaymentSourceKind::user_funded(default_user),
            (Some(_), Some(_)) => unreachable!("clap rejects multiple beneficiary selectors"),
        }
    }
}

#[derive(Subcommand)]
pub(crate) enum PolicyToggleCommand {
    #[command(about = "Accept this policy for new Invocations")]
    Enable {
        #[command(flatten)]
        admin: AdminArgs,
    },
    #[command(about = "Stop accepting this policy for new Invocations")]
    Disable {
        #[command(flatten)]
        admin: AdminArgs,
    },
}

#[derive(Args)]
pub(crate) struct FiniteCreditTerms {
    #[command(flatten)]
    admin: AdminArgs,
    #[arg(long, value_name = "MIST", help = "Price of one credit in MIST")]
    price_per_credit: NonZeroU64,
    #[arg(long, value_name = "COUNT", help = "Minimum credits in one purchase")]
    minimum_credits: NonZeroU64,
    #[arg(long, value_name = "COUNT", help = "Maximum credits in one purchase")]
    maximum_credits: NonZeroU64,
}

#[derive(Subcommand)]
pub(crate) enum FiniteCreditCommand {
    #[command(about = "Enable finite credit admission and sales")]
    Enable {
        #[command(flatten)]
        terms: FiniteCreditTerms,
    },
    #[command(about = "Close issuance without invalidating existing credits")]
    CloseIssuance {
        #[command(flatten)]
        admin: AdminArgs,
    },
    #[command(about = "Open issuance using the current terms")]
    OpenIssuance {
        #[command(flatten)]
        admin: AdminArgs,
    },
    #[command(about = "Change terms for future purchases")]
    UpdateTerms {
        #[command(flatten)]
        terms: FiniteCreditTerms,
    },
    #[command(about = "Buy units for the beneficiary finite credit account")]
    Buy {
        #[command(flatten)]
        tool: ToolArgs,
        #[arg(long, value_name = "COUNT", help = "Number of credits to buy")]
        credits: NonZeroU64,
        #[command(flatten)]
        beneficiary: BeneficiaryArgs,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Add units under Tool owner authority")]
    Issue {
        #[command(flatten)]
        admin: AdminArgs,
        #[command(flatten)]
        beneficiary: BeneficiaryArgs,
        #[arg(long, value_name = "COUNT", help = "Number of credits to issue")]
        credits: NonZeroU64,
    },
    #[command(about = "Restore one refunded Invocation to its credit account")]
    RestoreRefund {
        #[command(flatten)]
        tool: ToolArgs,
        #[arg(
            long,
            value_name = "OBJECT_ID",
            help = "Refunded Invocation held by the credit account"
        )]
        invocation_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Args)]
pub(crate) struct TimePassTerms {
    #[command(flatten)]
    admin: AdminArgs,
    #[arg(long, value_name = "MIST", help = "Price per millisecond in MIST")]
    price_per_ms: NonZeroU64,
    #[arg(
        long,
        value_name = "MILLISECONDS",
        help = "Minimum duration of one purchase"
    )]
    minimum_duration_ms: NonZeroU64,
    #[arg(
        long,
        value_name = "MILLISECONDS",
        help = "Maximum duration of one purchase"
    )]
    maximum_duration_ms: NonZeroU64,
}

#[derive(Subcommand)]
pub(crate) enum TimePassCommand {
    #[command(about = "Enable time pass admission and sales")]
    Enable {
        #[command(flatten)]
        terms: TimePassTerms,
    },
    #[command(about = "Close issuance without invalidating existing passes")]
    CloseIssuance {
        #[command(flatten)]
        admin: AdminArgs,
    },
    #[command(about = "Open issuance using the current terms")]
    OpenIssuance {
        #[command(flatten)]
        admin: AdminArgs,
    },
    #[command(about = "Change terms for future purchases")]
    UpdateTerms {
        #[command(flatten)]
        terms: TimePassTerms,
    },
    #[command(about = "Buy duration for the beneficiary time pass account")]
    Buy {
        #[command(flatten)]
        tool: ToolArgs,
        #[arg(
            long,
            value_name = "MILLISECONDS",
            help = "Duration of the access window"
        )]
        duration_ms: NonZeroU64,
        #[command(flatten)]
        beneficiary: BeneficiaryArgs,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Set a time pass window under Tool owner authority")]
    Issue {
        #[command(flatten)]
        admin: AdminArgs,
        #[command(flatten)]
        beneficiary: BeneficiaryArgs,
        #[arg(long, value_name = "MILLISECONDS", help = "Inclusive window start")]
        valid_from_ms: u64,
        #[arg(long, value_name = "MILLISECONDS", help = "Exclusive window end")]
        valid_until_ms: u64,
    },
}

#[derive(Clone, Copy)]
enum AdminAction {
    EnableFixedPrice,
    DisableFixedPrice,
    EnableFree,
    DisableFree,
    CloseFiniteCreditIssuance,
    OpenFiniteCreditIssuance,
    CloseTimePassIssuance,
    OpenTimePassIssuance,
}

async fn resolve_cashier_admin(
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
                "No cashier admin capability is available for Tool '{tool_fqn}'. Pass --cashier-admin or register the Tool with this CLI first."
            ))
        })
}

fn validate_range(
    minimum: NonZeroU64,
    maximum: NonZeroU64,
    unit: &str,
) -> AnyResult<(), NexusCliError> {
    if minimum > maximum {
        return Err(NexusCliError::Any(anyhow!(
            "Minimum {unit} '{minimum}' cannot exceed maximum {unit} '{maximum}'"
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
    }))
}

fn emit_purchase(
    action: &str,
    tool_fqn: &ToolFqn,
    result: &nexus_sdk::nexus::tool::EntitlementPurchaseResult,
) -> AnyResult<(), NexusCliError> {
    notify_success!(
        "Entitlement object: {id}",
        id = result.entitlement_id.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({
        "action": action,
        "tool_fqn": tool_fqn,
        "digest": result.tx_digest,
        "entitlement_id": result.entitlement_id,
        "deposit_id": result.deposit_id,
    }))
}

fn emit_issuance(
    action: &str,
    tool_fqn: &ToolFqn,
    result: &nexus_sdk::nexus::tool::EntitlementIssueResult,
) -> AnyResult<(), NexusCliError> {
    notify_success!(
        "Entitlement object: {id}",
        id = result.entitlement_id.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({
        "action": action,
        "tool_fqn": tool_fqn,
        "digest": result.tx_digest,
        "entitlement_id": result.entitlement_id,
    }))
}

async fn inspect(tool_fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Tool economy '{tool_fqn}'");
    let client = get_read_only_nexus_client().await?;
    let economy = client
        .tool()
        .inspect_economy(&tool_fqn)
        .await
        .map_err(NexusCliError::Nexus)?;
    print_economy(&tool_fqn, &economy);
    json_output(&json!({
        "tool_fqn": tool_fqn,
        "economy": economy,
    }))
}

async fn inspect_access(
    tool_fqn: ToolFqn,
    beneficiary: BeneficiaryArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Tool access '{tool_fqn}'");
    let client = get_owner_nexus_client().await?;
    let owner = client.owner().map_err(NexusCliError::Nexus)?;
    let access = client
        .tool()
        .inspect_access(&tool_fqn, beneficiary.resolve(owner))
        .await
        .map_err(NexusCliError::Nexus)?;

    match &access.finite_credits {
        Some(credits) => {
            notify_success!(
                "Finite credits: {} remaining in {}",
                credits.remaining,
                credits.account_id,
            );
            for invocation in &credits.refunded_invocations {
                notify_success!("Refund ready to restore: {invocation}");
            }
        }
        None => notify_success!("Finite credits: none"),
    }
    match &access.time_pass {
        Some(pass) => notify_success!(
            "Time pass: {} from {} through {}",
            if pass.active { "active" } else { "inactive" },
            pass.valid_from_ms,
            pass.valid_until_ms,
        ),
        None => notify_success!("Time pass: none"),
    }
    json_output(&json!({
        "tool_fqn": tool_fqn,
        "access": access,
    }))
}

async fn inspect_inbox(tool_fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Tool cashier inbox '{tool_fqn}'");
    let client = get_read_only_nexus_client().await?;
    let inbox = client
        .tool()
        .inspect_cashier_inbox(&tool_fqn)
        .await
        .map_err(NexusCliError::Nexus)?;
    notify_success!(
        "Collectable objects: {} Invocations, {} deposits",
        inbox.invocations.len(),
        inbox.deposits.len()
    );
    json_output(&json!({
        "tool_fqn": tool_fqn,
        "inbox": inbox,
    }))
}

fn print_economy(tool_fqn: &ToolFqn, economy: &ToolEconomy) {
    notify_success!("Tool economy: {tool_fqn}");
    for policy in &economy.policies {
        notify_success!("Policy: {policy}");
    }
    if let Some(price) = economy.fixed_price_mist {
        notify_success!("Fixed price: {price} MIST");
    }
    if economy.free_invocations {
        notify_success!("Sponsored free admission: enabled");
    }
    if let Some(offer) = &economy.finite_credits {
        notify_success!(
            "Finite credits: {} MIST each, {} to {}, issuance {}",
            offer.price_per_credit,
            offer.minimum_credits,
            offer.maximum_credits,
            if offer.issuance_enabled {
                "open"
            } else {
                "closed"
            }
        );
    }
    if let Some(offer) = &economy.time_pass {
        notify_success!(
            "Time pass: {} MIST per ms, {} to {} ms, issuance {}",
            offer.price_per_ms,
            offer.minimum_duration_ms,
            offer.maximum_duration_ms,
            if offer.issuance_enabled {
                "open"
            } else {
                "closed"
            }
        );
    }
}

async fn run_admin(admin: AdminArgs, action: AdminAction) -> AnyResult<(), NexusCliError> {
    let ToolArgs { tool_fqn } = admin.tool;
    let cashier_admin = resolve_cashier_admin(&tool_fqn, admin.cashier_admin).await?;
    command_title!("Updating Tool economy '{tool_fqn}'");
    let client = get_nexus_client(admin.gas.sui_gas_coin, admin.gas.sui_gas_budget).await?;
    let progress = loading!("Submitting policy transaction...");
    let result = match action {
        AdminAction::EnableFixedPrice => {
            client
                .tool()
                .enable_fixed_price(&tool_fqn, cashier_admin)
                .await
        }
        AdminAction::DisableFixedPrice => {
            client
                .tool()
                .disable_fixed_price(&tool_fqn, cashier_admin)
                .await
        }
        AdminAction::EnableFree => {
            client
                .tool()
                .enable_free_invocations(&tool_fqn, cashier_admin)
                .await
        }
        AdminAction::DisableFree => {
            client
                .tool()
                .disable_free_invocations(&tool_fqn, cashier_admin)
                .await
        }
        AdminAction::CloseFiniteCreditIssuance => {
            client
                .tool()
                .close_finite_credit_issuance(&tool_fqn, cashier_admin)
                .await
        }
        AdminAction::OpenFiniteCreditIssuance => {
            client
                .tool()
                .open_finite_credit_issuance(&tool_fqn, cashier_admin)
                .await
        }
        AdminAction::CloseTimePassIssuance => {
            client
                .tool()
                .close_time_pass_issuance(&tool_fqn, cashier_admin)
                .await
        }
        AdminAction::OpenTimePassIssuance => {
            client
                .tool()
                .open_time_pass_issuance(&tool_fqn, cashier_admin)
                .await
        }
    };
    let result = match result {
        Ok(result) => {
            progress.success();
            result
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    let action = match action {
        AdminAction::EnableFixedPrice => "enable_fixed_price",
        AdminAction::DisableFixedPrice => "disable_fixed_price",
        AdminAction::EnableFree => "enable_free_invocations",
        AdminAction::DisableFree => "disable_free_invocations",
        AdminAction::CloseFiniteCreditIssuance => "close_finite_credit_issuance",
        AdminAction::OpenFiniteCreditIssuance => "open_finite_credit_issuance",
        AdminAction::CloseTimePassIssuance => "close_time_pass_issuance",
        AdminAction::OpenTimePassIssuance => "open_time_pass_issuance",
    };
    emit_result(action, &tool_fqn, &result.tx_digest)
}

async fn set_finite_credit_terms(
    terms: FiniteCreditTerms,
    enable: bool,
) -> AnyResult<(), NexusCliError> {
    validate_range(terms.minimum_credits, terms.maximum_credits, "credits")?;
    let ToolArgs { tool_fqn } = terms.admin.tool;
    let cashier_admin = resolve_cashier_admin(&tool_fqn, terms.admin.cashier_admin).await?;
    let client =
        get_nexus_client(terms.admin.gas.sui_gas_coin, terms.admin.gas.sui_gas_budget).await?;
    let result = if enable {
        client
            .tool()
            .enable_finite_credits(
                &tool_fqn,
                cashier_admin,
                terms.price_per_credit.get(),
                terms.minimum_credits.get(),
                terms.maximum_credits.get(),
            )
            .await
    } else {
        client
            .tool()
            .update_finite_credit_terms(
                &tool_fqn,
                cashier_admin,
                terms.price_per_credit.get(),
                terms.minimum_credits.get(),
                terms.maximum_credits.get(),
            )
            .await
    }
    .map_err(NexusCliError::Nexus)?;
    emit_result(
        if enable {
            "enable_finite_credits"
        } else {
            "update_finite_credit_terms"
        },
        &tool_fqn,
        &result.tx_digest,
    )
}

async fn set_time_pass_terms(terms: TimePassTerms, enable: bool) -> AnyResult<(), NexusCliError> {
    validate_range(
        terms.minimum_duration_ms,
        terms.maximum_duration_ms,
        "duration",
    )?;
    let ToolArgs { tool_fqn } = terms.admin.tool;
    let cashier_admin = resolve_cashier_admin(&tool_fqn, terms.admin.cashier_admin).await?;
    let client =
        get_nexus_client(terms.admin.gas.sui_gas_coin, terms.admin.gas.sui_gas_budget).await?;
    let result = if enable {
        client
            .tool()
            .enable_time_passes(
                &tool_fqn,
                cashier_admin,
                terms.price_per_ms.get(),
                terms.minimum_duration_ms.get(),
                terms.maximum_duration_ms.get(),
            )
            .await
    } else {
        client
            .tool()
            .update_time_pass_terms(
                &tool_fqn,
                cashier_admin,
                terms.price_per_ms.get(),
                terms.minimum_duration_ms.get(),
                terms.maximum_duration_ms.get(),
            )
            .await
    }
    .map_err(NexusCliError::Nexus)?;
    emit_result(
        if enable {
            "enable_time_passes"
        } else {
            "update_time_pass_terms"
        },
        &tool_fqn,
        &result.tx_digest,
    )
}

async fn buy_finite_credits(
    tool_fqn: ToolFqn,
    credits: NonZeroU64,
    beneficiary: BeneficiaryArgs,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let owner = client.owner().map_err(NexusCliError::Nexus)?;
    let result = client
        .tool()
        .buy_finite_credits_for(&tool_fqn, credits.get(), beneficiary.resolve(owner))
        .await
        .map_err(NexusCliError::Nexus)?;
    emit_purchase("buy_finite_credits", &tool_fqn, &result)
}

async fn buy_time_pass(
    tool_fqn: ToolFqn,
    duration_ms: NonZeroU64,
    beneficiary: BeneficiaryArgs,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let owner = client.owner().map_err(NexusCliError::Nexus)?;
    let result = client
        .tool()
        .buy_time_pass_for(&tool_fqn, duration_ms.get(), beneficiary.resolve(owner))
        .await
        .map_err(NexusCliError::Nexus)?;
    emit_purchase("buy_time_pass", &tool_fqn, &result)
}

async fn issue_finite_credits(
    admin: AdminArgs,
    beneficiary: BeneficiaryArgs,
    credits: NonZeroU64,
) -> AnyResult<(), NexusCliError> {
    let ToolArgs { tool_fqn } = admin.tool;
    let cashier_admin = resolve_cashier_admin(&tool_fqn, admin.cashier_admin).await?;
    let client = get_nexus_client(admin.gas.sui_gas_coin, admin.gas.sui_gas_budget).await?;
    let owner = client.owner().map_err(NexusCliError::Nexus)?;
    let result = client
        .tool()
        .issue_finite_credits(
            &tool_fqn,
            cashier_admin,
            beneficiary.resolve(owner),
            credits.get(),
        )
        .await
        .map_err(NexusCliError::Nexus)?;
    emit_issuance("issue_finite_credits", &tool_fqn, &result)
}

async fn issue_time_pass(
    admin: AdminArgs,
    beneficiary: BeneficiaryArgs,
    valid_from_ms: u64,
    valid_until_ms: u64,
) -> AnyResult<(), NexusCliError> {
    if valid_from_ms >= valid_until_ms {
        return Err(NexusCliError::Any(anyhow!(
            "Time pass end must be after its start"
        )));
    }
    let ToolArgs { tool_fqn } = admin.tool;
    let cashier_admin = resolve_cashier_admin(&tool_fqn, admin.cashier_admin).await?;
    let client = get_nexus_client(admin.gas.sui_gas_coin, admin.gas.sui_gas_budget).await?;
    let owner = client.owner().map_err(NexusCliError::Nexus)?;
    let result = client
        .tool()
        .issue_time_pass(
            &tool_fqn,
            cashier_admin,
            beneficiary.resolve(owner),
            valid_from_ms,
            valid_until_ms,
        )
        .await
        .map_err(NexusCliError::Nexus)?;
    emit_issuance("issue_time_pass", &tool_fqn, &result)
}

async fn restore_finite_credit_refund(
    tool_fqn: ToolFqn,
    invocation_id: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let result = client
        .tool()
        .restore_finite_credit_refund(&tool_fqn, invocation_id)
        .await
        .map_err(NexusCliError::Nexus)?;
    emit_issuance("restore_finite_credit_refund", &tool_fqn, &result)
}

async fn collect_invocations(
    admin: AdminArgs,
    invocation_ids: Vec<sui::types::Address>,
    recipient: Option<sui::types::Address>,
) -> AnyResult<(), NexusCliError> {
    let ToolArgs { tool_fqn } = admin.tool;
    let cashier_admin = resolve_cashier_admin(&tool_fqn, admin.cashier_admin).await?;
    let client = get_nexus_client(admin.gas.sui_gas_coin, admin.gas.sui_gas_budget).await?;
    let recipient = recipient.unwrap_or(client.owner().map_err(NexusCliError::Nexus)?);
    let result = client
        .tool()
        .collect_invocations(&tool_fqn, cashier_admin, &invocation_ids, recipient)
        .await
        .map_err(NexusCliError::Nexus)?;
    emit_result("collect_invocations", &tool_fqn, &result.tx_digest)
}

async fn collect_deposits(
    admin: AdminArgs,
    deposit_ids: Vec<sui::types::Address>,
    recipient: Option<sui::types::Address>,
) -> AnyResult<(), NexusCliError> {
    let ToolArgs { tool_fqn } = admin.tool;
    let cashier_admin = resolve_cashier_admin(&tool_fqn, admin.cashier_admin).await?;
    let client = get_nexus_client(admin.gas.sui_gas_coin, admin.gas.sui_gas_budget).await?;
    let recipient = recipient.unwrap_or(client.owner().map_err(NexusCliError::Nexus)?);
    let result = client
        .tool()
        .collect_deposits(&tool_fqn, cashier_admin, &deposit_ids, recipient)
        .await
        .map_err(NexusCliError::Nexus)?;
    emit_result("collect_deposits", &tool_fqn, &result.tx_digest)
}

pub(crate) async fn handle_cashier(command: CashierCommand) -> AnyResult<(), NexusCliError> {
    match command {
        CashierCommand::Inspect { tool } => inspect(tool.tool_fqn).await,
        CashierCommand::Access { tool, beneficiary } => {
            inspect_access(tool.tool_fqn, beneficiary).await
        }
        CashierCommand::Inbox { tool } => inspect_inbox(tool.tool_fqn).await,
        CashierCommand::CollectInvocations {
            admin,
            invocation_ids,
            recipient,
        } => collect_invocations(admin, invocation_ids, recipient).await,
        CashierCommand::CollectDeposits {
            admin,
            deposit_ids,
            recipient,
        } => collect_deposits(admin, deposit_ids, recipient).await,
        CashierCommand::FixedPrice(command) => match command {
            PolicyToggleCommand::Enable { admin } => {
                run_admin(admin, AdminAction::EnableFixedPrice).await
            }
            PolicyToggleCommand::Disable { admin } => {
                run_admin(admin, AdminAction::DisableFixedPrice).await
            }
        },
        CashierCommand::Free(command) => match command {
            PolicyToggleCommand::Enable { admin } => {
                run_admin(admin, AdminAction::EnableFree).await
            }
            PolicyToggleCommand::Disable { admin } => {
                run_admin(admin, AdminAction::DisableFree).await
            }
        },
        CashierCommand::FiniteCredits(command) => match command {
            FiniteCreditCommand::Enable { terms } => set_finite_credit_terms(terms, true).await,
            FiniteCreditCommand::CloseIssuance { admin } => {
                run_admin(admin, AdminAction::CloseFiniteCreditIssuance).await
            }
            FiniteCreditCommand::OpenIssuance { admin } => {
                run_admin(admin, AdminAction::OpenFiniteCreditIssuance).await
            }
            FiniteCreditCommand::UpdateTerms { terms } => {
                set_finite_credit_terms(terms, false).await
            }
            FiniteCreditCommand::Buy {
                tool,
                credits,
                beneficiary,
                gas,
            } => buy_finite_credits(tool.tool_fqn, credits, beneficiary, gas).await,
            FiniteCreditCommand::Issue {
                admin,
                beneficiary,
                credits,
            } => issue_finite_credits(admin, beneficiary, credits).await,
            FiniteCreditCommand::RestoreRefund {
                tool,
                invocation_id,
                gas,
            } => restore_finite_credit_refund(tool.tool_fqn, invocation_id, gas).await,
        },
        CashierCommand::TimePass(command) => match command {
            TimePassCommand::Enable { terms } => set_time_pass_terms(terms, true).await,
            TimePassCommand::CloseIssuance { admin } => {
                run_admin(admin, AdminAction::CloseTimePassIssuance).await
            }
            TimePassCommand::OpenIssuance { admin } => {
                run_admin(admin, AdminAction::OpenTimePassIssuance).await
            }
            TimePassCommand::UpdateTerms { terms } => set_time_pass_terms(terms, false).await,
            TimePassCommand::Buy {
                tool,
                duration_ms,
                beneficiary,
                gas,
            } => buy_time_pass(tool.tool_fqn, duration_ms, beneficiary, gas).await,
            TimePassCommand::Issue {
                admin,
                beneficiary,
                valid_from_ms,
                valid_until_ms,
            } => issue_time_pass(admin, beneficiary, valid_from_ms, valid_until_ms).await,
        },
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn finite_credit_purchase_uses_product_vocabulary() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "finite-credits",
            "buy",
            "--tool-fqn",
            "com.example.tool@1",
            "--credits",
            "5",
        ])
        .expect("finite credit purchase should parse");

        assert!(matches!(
            cli.command,
            crate::Command::Tool(super::super::ToolCommand::Cashier(
                CashierCommand::FiniteCredits(FiniteCreditCommand::Buy { .. })
            ))
        ));
    }

    #[test]
    fn purchase_does_not_expose_coin_object_selection() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "finite-credits",
            "buy",
            "--tool-fqn",
            "com.example.tool@1",
            "--credits",
            "5",
            "--payment-coin",
            "0x1",
        ])
        .is_err());
    }

    #[test]
    fn access_inspection_defaults_to_the_active_user() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "access",
            "--tool-fqn",
            "com.example.tool@1",
        ])
        .expect("Tool access should be discoverable without object IDs");

        assert!(matches!(
            cli.command,
            crate::Command::Tool(super::super::ToolCommand::Cashier(
                CashierCommand::Access { .. }
            ))
        ));
    }

    #[test]
    fn zero_entitlement_quantity_is_rejected() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "time-pass",
            "buy",
            "--tool-fqn",
            "com.example.tool@1",
            "--duration-ms",
            "0",
        ])
        .is_err());
    }

    #[test]
    fn owner_can_issue_an_agent_entitlement() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "finite-credits",
            "issue",
            "--tool-fqn",
            "com.example.tool@1",
            "--cashier-admin",
            "0x8",
            "--beneficiary-agent",
            "0xa",
            "--credits",
            "5",
        ])
        .expect("Agent finite credit issuance should parse");

        assert!(matches!(
            cli.command,
            crate::Command::Tool(super::super::ToolCommand::Cashier(
                CashierCommand::FiniteCredits(FiniteCreditCommand::Issue { .. })
            ))
        ));
    }

    #[test]
    fn entitlement_rejects_two_beneficiary_sources() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "time-pass",
            "buy",
            "--tool-fqn",
            "com.example.tool@1",
            "--duration-ms",
            "10",
            "--beneficiary-user",
            "0x2",
            "--beneficiary-agent",
            "0x3",
        ])
        .is_err());
    }

    #[test]
    fn finite_credit_refund_and_collection_need_no_policy_witness() {
        let restore = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "finite-credits",
            "restore-refund",
            "--tool-fqn",
            "com.example.tool@1",
            "--invocation-id",
            "0x7",
        ])
        .expect("finite credit refund restore should parse");
        assert!(matches!(
            restore.command,
            crate::Command::Tool(super::super::ToolCommand::Cashier(
                CashierCommand::FiniteCredits(FiniteCreditCommand::RestoreRefund { .. })
            ))
        ));

        let collect = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "cashier",
            "collect-invocations",
            "--tool-fqn",
            "com.example.tool@1",
            "--cashier-admin",
            "0x8",
            "--invocation-id",
            "0x9",
        ])
        .expect("Invocation collection should infer its policy");
        assert!(matches!(
            collect.command,
            crate::Command::Tool(super::super::ToolCommand::Cashier(
                CashierCommand::CollectInvocations { .. }
            ))
        ));
    }
}
