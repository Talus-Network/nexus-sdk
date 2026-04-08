mod tool_auth;
mod tool_claim_collateral;
mod tool_list;
mod tool_new;
mod tool_register_offchain;
mod tool_register_onchain;
mod tool_set_invocation_cost;
mod tool_unregister;
mod tool_update_timeout;
mod tool_validate;

use {
    crate::{prelude::*, tool::tool_update_timeout::update_tool_timeout},
    tool_auth::handle_tool_auth,
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
pub(crate) enum ToolAuthCommand {
    #[command(about = "Generate a new Ed25519 message-signing key for a tool.")]
    Keygen {
        #[arg(
            long = "out",
            help = "Write the generated keypair JSON to this path.",
            long_help = "Write the generated keypair JSON to this path. The output contains both `private_key_hex` and `public_key_hex`.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        out: Option<PathBuf>,
    },

    #[command(about = "Register (or rotate) a tool message-signing key on-chain.")]
    RegisterKey {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,

        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "OwnerCap<OverTool> object ID (defaults to saved CLI config for this tool).",
            value_name = "OBJECT_ID"
        )]
        owner_cap: Option<sui::types::Address>,

        #[arg(
            long = "signing-key",
            short = 'k',
            help = "Tool Ed25519 private key (hex/base64/base64url) OR a path to a file containing it.",
            value_name = "KEY_OR_PATH"
        )]
        signing_key: String,

        #[arg(
            long = "description",
            help = "Optional description bytes stored on the key binding.",
            value_name = "TEXT"
        )]
        description: Option<String>,

        #[arg(
            long = "skip-if-active",
            help = "Skip registration if the same public key is already the active key (idempotent). Useful in CI to avoid re-registering an unchanged key."
        )]
        skip_if_active: bool,

        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "List all registered message-signing keys for a tool.")]
    ListKeys {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The fully qualified name (FQN) of the tool.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
    },

    #[command(
        about = "Export a leader allowlist file for tool-side verification (no RPC at runtime)."
    )]
    ExportAllowedLeaders {
        #[arg(
            long = "all",
            help = "Export allowlist entries for all leaders registered in network_auth (recommended).",
            conflicts_with = "leaders"
        )]
        all: bool,

        /// One or more leader capability IDs (`leader_cap::OverNetwork` object IDs) to include.
        #[arg(
            long = "leader",
            value_name = "LEADER_CAP_ID",
            required_unless_present = "all"
        )]
        leaders: Vec<sui::types::Address>,

        #[arg(
            long = "out",
            help = "Output path for the allowlist JSON file.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        out: PathBuf,
    },

    #[command(
        about = "Sync an allowed leaders allowlist file from on-chain network_auth (polling)."
    )]
    SyncAllowedLeaders {
        #[arg(
            long = "out",
            help = "Output path for the allowlist JSON file.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        out: PathBuf,

        #[arg(
            long = "interval",
            default_value = "30s",
            help = "Polling interval (e.g. 500ms, 5s, 2m, 1h).",
            value_name = "DURATION",
            value_parser = ValueParser::from(humantime::parse_duration)
        )]
        interval: std::time::Duration,

        #[arg(long = "once", help = "Sync once and exit.")]
        once: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum RegisterCommand {
    #[command(about = "Register an offchain tool")]
    Offchain {
        #[arg(
            long = "url",
            short = 'u',
            help = "The URL of the offchain tool. Required unless --from-meta is provided.",
            required_unless_present = "from_meta"
        )]
        url: Option<reqwest::Url>,

        #[arg(
            long = "from-meta",
            help = "Path to a JSON file containing tool metadata (as produced by the tool binary's --meta flag), or '-' to read from stdin. Skips the live HTTP validation step.",
            value_name = "FILE|-",
            conflicts_with = "batch"
        )]
        from_meta: Option<String>,

        #[arg(
            long = "collateral-coin",
            short = 'c',
            help = "The collateral coin object ID. Second coin object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        collateral_coin: Option<sui::types::Address>,

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
            help = "Should all tools on a webserver be registered at once? Incompatible with --from-meta."
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
            long = "package",
            short = 'p',
            help = "The onchain tool package address",
            value_name = "ADDRESS"
        )]
        package: sui::types::Address,

        #[arg(long = "module", short = 'm', help = "The onchain tool module name")]
        module: sui::types::Identifier,

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
            long = "timeout",
            short = 'i',
            help = "The timeout duration for the tool execution. Defaults to 5 seconds. Value must be between 1 second and 2 minutes.",
            value_name = "DURATION",
            value_parser = ValueParser::from(humantime::parse_duration),
            default_value = "5s"
        )]
        timeout: std::time::Duration,

        #[arg(
            long = "witness-id",
            short = 'w',
            help = "The witness object ID that proves the tool's identity.",
            value_name = "OBJECT_ID"
        )]
        witness_id: sui::types::Address,

        #[arg(
            long = "collateral-coin",
            short = 'c',
            help = "The collateral coin object ID. Second coin object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        collateral_coin: Option<sui::types::Address>,

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
        owner_cap: Option<sui::types::Address>,
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
        owner_cap: Option<sui::types::Address>,
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
        owner_cap: Option<sui::types::Address>,
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

    #[command(about = "Manage tool auth for signed HTTP.")]
    Auth {
        #[command(subcommand)]
        cmd: ToolAuthCommand,
    },

    #[command(about = "Update a tool's timeout duration.")]
    UpdateTimeout {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool to update the timeout for.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
        #[arg(
            long = "owner-cap",
            short = 'o',
            help = "The OwnerCap<OverTool> object ID that must be owned by the sender.",
            value_name = "OBJECT_ID"
        )]
        owner_cap: Option<sui::types::Address>,
        #[arg(
            long = "timeout",
            short = 'i',
            help = "The new timeout duration for the tool execution. Value must be between 1 second and 2 minutes.",
            value_name = "DURATION",
            value_parser = ValueParser::from(humantime::parse_duration),
        )]
        timeout: std::time::Duration,
        #[command(flatten)]
        gas: GasArgs,
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
                from_meta,
                collateral_coin,
                invocation_cost,
                batch,
                no_save,
                gas,
            } => {
                register_off_chain_tool(
                    url,
                    from_meta,
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
                package,
                module,
                tool_fqn,
                description,
                timeout,
                witness_id,
                collateral_coin,
                no_save,
                gas,
            } => {
                register_onchain_tool(
                    package,
                    module,
                    tool_fqn,
                    description,
                    timeout,
                    witness_id,
                    collateral_coin,
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

        // == `$ nexus tool auth` ==
        ToolCommand::Auth { cmd } => handle_tool_auth(cmd).await,

        // == `$ nexus tool update-timeout` ==
        ToolCommand::UpdateTimeout {
            tool_fqn,
            owner_cap,
            timeout,
            gas,
        } => {
            update_tool_timeout(
                tool_fqn,
                owner_cap,
                timeout,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}
