mod tool_claim_collateral;
mod tool_list;
mod tool_new;
mod tool_register_offchain;
mod tool_register_onchain;
mod tool_set_invocation_cost;
mod tool_unregister;
mod tool_validate;

use {
    crate::prelude::*,
    tool_claim_collateral::*,
    tool_list::*,
    tool_new::*,
    tool_register_offchain::register_off_chain_tool,
    tool_register_onchain::register_onchain_tool,
    tool_set_invocation_cost::*,
    tool_unregister::*,
    tool_validate::{validate_off_chain_tool, validate_on_chain_tool},
};

#[derive(Subcommand)]
pub(crate) enum RegisterCommand {
    #[command(about = "Register an offchain tool")]
    Offchain {
        #[arg(long = "url", short = 'u', help = "The URL of the offchain tool")]
        url: reqwest::Url,

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

        #[arg(
            long = "no-save",
            help = "If this flag is set, the tool owner caps will not be saved to the local config file."
        )]
        no_save: bool,

        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Register an onchain tool")]
    Onchain {
        #[arg(
            long = "package-address",
            short = 'p',
            help = "The address of the published package containing the tool.",
            value_name = "ADDRESS"
        )]
        package_address: sui::ObjectID,

        #[arg(
            long = "module-name",
            short = 'm',
            help = "The module name containing the tool's execute function.",
            value_name = "MODULE"
        )]
        module_name: String,

        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) for this tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,

        #[arg(
            long = "description",
            short = 'd',
            help = "Description of what the tool does.",
            value_name = "DESCRIPTION"
        )]
        description: String,

        #[arg(
            long = "witness-id",
            short = 'w',
            help = "The witness object ID that proves the tool's identity.",
            value_name = "OBJECT_ID"
        )]
        witness_id: sui::ObjectID,

        #[arg(
            long = "no-save",
            help = "If this flag is set, the tool owner caps will not be saved to the local config file."
        )]
        no_save: bool,

        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum ValidateCommand {
    #[command(about = "Validate an offchain tool")]
    Offchain {
        #[arg(
            long = "url",
            short = 'u',
            help = "The URL of the offchain tool to validate"
        )]
        url: reqwest::Url,
    },

    #[command(about = "Validate an onchain tool")]
    Onchain {
        #[arg(
            long = "ident",
            short = 'i',
            help = "The identifier of the onchain tool to validate"
        )]
        ident: String,
    },
}

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

    #[command(about = "Validate a tool based on its type.")]
    Validate {
        #[command(subcommand)]
        tool_type: ValidateCommand,
    },

    #[command(about = "Register a tool based on its type.")]
    Register {
        #[command(subcommand)]
        tool_type: RegisterCommand,
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
        owner_cap: Option<sui::ObjectID>,
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
        owner_cap: Option<sui::ObjectID>,
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
        owner_cap: Option<sui::ObjectID>,
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
        ToolCommand::Validate { tool_type } => match tool_type {
            ValidateCommand::Offchain { url } => validate_off_chain_tool(url).await.map(|_| ()),
            ValidateCommand::Onchain { ident } => validate_on_chain_tool(ident).await.map(|_| ()),
        },

        // == `$ nexus tool register` ==
        ToolCommand::Register { tool_type } => match tool_type {
            RegisterCommand::Offchain {
                url,
                collateral_coin,
                invocation_cost,
                batch,
                no_save,
                gas,
            } => {
                register_off_chain_tool(
                    url,
                    collateral_coin,
                    invocation_cost,
                    batch,
                    no_save,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }
            RegisterCommand::Onchain {
                package_address,
                module_name,
                tool_fqn,
                description,
                witness_id,
                no_save,
                gas,
            } => {
                register_onchain_tool(
                    package_address,
                    module_name,
                    tool_fqn,
                    description,
                    witness_id,
                    no_save,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }
        },

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
