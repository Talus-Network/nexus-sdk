mod tool_new;
mod tool_register;
mod tool_unregister;
mod tool_validate;

use {crate::prelude::*, tool_new::*, tool_register::*, tool_unregister::*, tool_validate::*};

#[derive(Subcommand)]
pub(crate) enum ToolCommand {
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
            default_value = "./"
        )]
        target: String,
    },

    Validate {
        /// The ident of the Tool to validate.
        #[command(flatten)]
        ident: ToolIdent,
    },

    Register {
        /// The ident of the Tool to register.
        #[command(flatten)]
        ident: ToolIdent,
        /// The gas coin object ID. First coin object is chosen if not present.
        #[arg(
            long = "sui-gas-coin",
            short = 'g',
            help = "The gas coin object ID. First coin object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        sui_gas_coin: Option<sui::ObjectID>,
        /// The collateral coin object ID. Second coin object is chosen if not
        /// present.
        #[arg(
            long = "sui-collateral-coin",
            short = 'c',
            help = "The collateral coin object ID. Second coin object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        sui_collateral_coin: Option<sui::ObjectID>,
        /// The gas budget for registering a Tool.
        #[arg(
            long = "sui-gas-budget",
            short = 'b',
            help = "The gas budget for registering a Tool",
            value_name = "AMOUNT",
            default_value_t = sui::MIST_PER_SUI / 10
        )]
        sui_gas_budget: u64,
    },

    Unregister {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool to unregister.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        /// The gas coin object ID. First coin object is chosen if not present.
        #[arg(
            long = "sui-gas-coin",
            short = 'g',
            help = "The gas coin object ID. First coin object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        sui_gas_coin: Option<sui::ObjectID>,
        /// The gas budget for unregistering a Tool.
        #[arg(
            long = "sui-gas-budget",
            short = 'b',
            help = "The gas budget for unregistering a Tool",
            value_name = "AMOUNT",
            default_value_t = sui::MIST_PER_SUI / 10
        )]
        sui_gas_budget: u64,
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

/// Useful struct holding Tool metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ToolMeta {
    pub(crate) fqn: ToolFqn,
    pub(crate) url: String,
    pub(crate) input_schema: serde_json::Value,
    pub(crate) output_schema: serde_json::Value,
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
            sui_gas_coin,
            sui_collateral_coin,
            sui_gas_budget,
        } => register_tool(ident, sui_gas_coin, sui_collateral_coin, sui_gas_budget).await,

        // == `$ nexus tool unregister` ==
        ToolCommand::Unregister {
            tool_fqn,
            sui_gas_coin,
            sui_gas_budget,
        } => unregister_tool(tool_fqn, sui_gas_coin, sui_gas_budget).await,
    }
}
