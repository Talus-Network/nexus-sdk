mod tool_claim_collateral;
mod tool_list;
mod tool_new;
mod tool_register;
mod tool_set_invocation_cost;
mod tool_unregister;
mod tool_validate;

use {
    crate::prelude::*,
    tool_claim_collateral::*,
    tool_list::*,
    tool_new::*,
    tool_register::*,
    tool_set_invocation_cost::*,
    tool_unregister::*,
    tool_validate::*,
};

#[derive(Subcommand)]
pub(crate) enum ToolCommand {
    #[command(about = "Create a new tool scaffolding with the specified name and template.")]
    New {
        /// The name of the tool to create. This will be the name of the
        /// directory that contains the newly created tool.
        #[arg(long = "name", short = 'n', help = "The name of the tool to create")]
        name: String,
        /// The template to use for generating this tool.
        #[arg(
            long = "template",
            short = 't',
            value_enum,
            help = "The Nexus Tool template to use"
        )]
        template: ToolTemplate,
        /// The target directory to create the tool in. Defaults to the current
        /// directory.
        #[arg(
            long = "target",
            short = 'd',
            help = "The target directory to create the tool in",
            default_value = "./",
            value_parser = ValueParser::from(expand_tilde)
        )]
        target: PathBuf,
    },

    #[command(about = "Validate a tool based on its identifier.")]
    Validate {
        /// The ident of the Tool to validate.
        #[command(flatten)]
        ident: ToolIdent,
    },

    #[command(about = "Register a tool based on its identifier.")]
    Register {
        /// The collateral coin object ID. Second coin object is chosen if not
        /// present.
        #[arg(
            long = "collateral-coin",
            short = 'c',
            help = "The collateral coin object ID. Second coin object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        collateral_coin: Option<sui::ObjectID>,
        #[arg(
            long = "invocation-cost",
            short = 'i',
            help = "What is the cost of invoking this tool in MIST.",
            default_value = "0",
            value_name = "MIST"
        )]
        invocation_cost: u64,
        #[arg(
            long = "batch",
            help = "Should all tools on a webserver be registered at once?"
        )]
        batch: bool,
        /// The ident of the Tool to register.
        #[command(flatten)]
        ident: ToolIdent,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Unregister a tool identified by its FQN.")]
    Unregister {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool to unregister.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "The OwnerCap<OverTool> object ID that must be owned by the sender.",
            value_name = "OBJECT_ID"
        )]
        owner_cap: sui::ObjectID,
        /// Whether to skip the confirmation prompt.
        #[arg(long = "yes", short = 'y', help = "Skip the confirmation prompt")]
        skip_confirmation: bool,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Claim collateral for a tool identified by its FQN.")]
    ClaimCollateral {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool to claim the collateral for.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "The OwnerCap<OverTool> object ID that must be owned by the sender.",
            value_name = "OBJECT_ID"
        )]
        owner_cap: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Set a single invocation cost of a tool in MIST")]
    SetInvocationCost {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool to set the invocation cost for.",
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
            long = "invocation-cost",
            short = 'i',
            help = "What is the cost of invoking this tool in MIST.",
            default_value = "0",
            value_name = "MIST"
        )]
        invocation_cost: u64,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "List all registered tools.")]
    List {
        //
    },
}

/// Struct holding an either on-chain or off-chain Tool identifier. Off-chain
/// tools are identified by their URL, while on-chain tools are identified by
/// a Move ident.
#[derive(Args, Clone, Debug)]
#[group(required = true, multiple = false)]
pub(crate) struct ToolIdent {
    #[arg(
        long = "off-chain",
        short = 'f',
        help = "The URL of the off-chain Tool to validate",
        value_name = "URL"
    )]
    pub(crate) off_chain: Option<reqwest::Url>,
    #[arg(
        long = "on-chain",
        short = 'n',
        help = "The ident of on-chain Tool to validate",
        value_name = "IDENT"
    )]
    pub(crate) on_chain: Option<String>,
}

/// Handle the provided tool command. The [ToolCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: ToolCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus tool new` ==
        ToolCommand::New {
            name,
            template,
            target,
        } => create_new_tool(name, template, target).await,

        // == `$ nexus tool validate` ==
        ToolCommand::Validate { ident } => validate_tool(ident).await.map(|_| ()),

        // == `$ nexus tool register` ==
        ToolCommand::Register {
            ident,
            collateral_coin,
            invocation_cost,
            batch,
            gas,
        } => {
            register_tool(
                ident,
                collateral_coin,
                invocation_cost,
                batch,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        // == `$ nexus tool unregister` ==
        ToolCommand::Unregister {
            tool_fqn,
            owner_cap,
            gas,
            skip_confirmation,
        } => {
            unregister_tool(
                tool_fqn,
                owner_cap,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
                skip_confirmation,
            )
            .await
        }

        // == `$ nexus tool claim-collateral` ==
        ToolCommand::ClaimCollateral {
            tool_fqn,
            owner_cap,
            gas,
        } => claim_collateral(tool_fqn, owner_cap, gas.sui_gas_coin, gas.sui_gas_budget).await,

        // == `$ nexus tool set-invocation-cost` ==
        ToolCommand::SetInvocationCost {
            tool_fqn,
            owner_cap,
            invocation_cost,
            gas,
        } => {
            set_tool_invocation_cost(
                tool_fqn,
                owner_cap,
                invocation_cost,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        // == `$ nexus tool list` ==
        ToolCommand::List { .. } => list_tools().await,
    }
}
