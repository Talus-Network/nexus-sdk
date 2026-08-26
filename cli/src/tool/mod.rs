mod cashier;
mod tool_auth;
mod tool_claim_collateral;
mod tool_configure_verifier;
mod tool_inspect;
mod tool_list;
mod tool_new;
mod tool_output;
mod tool_register_offchain;
mod tool_register_onchain;
mod tool_set_invocation_cost;
mod tool_unregister;
mod tool_update;
mod tool_update_timeout;
mod tool_validate;

use {
    crate::prelude::*,
    cashier::{handle_cashier, CashierCommand},
    tool_auth::handle_tool_auth,
    tool_claim_collateral::*,
    tool_configure_verifier::{configure_verifier, VerifierCommand},
    tool_inspect::inspect_tool,
    tool_list::*,
    tool_new::*,
    tool_register_offchain::register_off_chain_tool,
    tool_register_onchain::register_onchain_tool,
    tool_set_invocation_cost::set_invocation_cost,
    tool_unregister::unregister_tool,
    tool_update::{update_metadata, update_on_chain_package, update_url},
    tool_update_timeout::update_timeout,
    tool_validate::{
        output_on_chain_validation,
        output_validation,
        validate_off_chain_tool,
        validate_on_chain_tool,
    },
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
            help = "The US collateral coin object ID. First owned Coin<US> object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        collateral_coin: Option<sui::types::Address>,

        #[arg(
            long = "invocation-cost",
            short = 'i',
            help = "The price of one tool invocation in MIST.",
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
            long = "tool-witness-id",
            short = 'w',
            help = "The tool witness object ID used as the on-chain execution stamp locator.",
            value_name = "OBJECT_ID"
        )]
        tool_witness_id: sui::types::Address,

        #[arg(
            long = "collateral-coin",
            short = 'c',
            help = "The US collateral coin object ID. First owned Coin<US> object is chosen if not present.",
            value_name = "OBJECT_ID"
        )]
        collateral_coin: Option<sui::types::Address>,

        #[arg(
            long = "invocation-cost",
            help = "Cost of one tool invocation in MIST",
            default_value_t = 0,
            value_name = "MIST"
        )]
        invocation_cost: u64,

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
            help = "The FQN of the registered onchain tool to validate",
            value_name = "FQN"
        )]
        ident: ToolFqn,
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
        /// A concise description of the Tool's observable behavior.
        #[arg(
            long = "description",
            help = "The user facing behavior description stored with the Tool"
        )]
        description: String,
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

    #[command(subcommand, about = "Manage Tool economic policies and entitlements")]
    Cashier(CashierCommand),

    #[command(about = "Unregister a Tool from live protocol lookup")]
    Unregister {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the Tool to unregister",
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
        #[arg(long = "yes", short = 'y', help = "Skip the confirmation prompt")]
        skip_confirmation: bool,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Update the HTTP endpoint of an offchain Tool")]
    UpdateUrl {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(long = "url", short = 'u', value_name = "URL")]
        url: reqwest::Url,
        #[arg(long = "owner-cap", short = 'o', value_name = "OBJECT_ID")]
        owner_cap: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Update the description of a Tool")]
    UpdateMetadata {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(long = "description", short = 'd', value_name = "TEXT")]
        description: String,
        #[arg(long = "owner-cap", short = 'o', value_name = "OBJECT_ID")]
        owner_cap: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Update the package of an onchain Tool")]
    MigratePackage {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(long = "package", short = 'p', value_name = "PACKAGE_ID")]
        package: sui::types::Address,
        #[arg(long = "owner-cap", short = 'o', value_name = "OBJECT_ID")]
        owner_cap: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Update the execution timeout of a Tool")]
    UpdateTimeout {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(long = "timeout", value_name = "DURATION", value_parser = ValueParser::from(humantime::parse_duration))]
        timeout: std::time::Duration,
        #[arg(long = "owner-cap", short = 'o', value_name = "OBJECT_ID")]
        owner_cap: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Set the invocation price of a Tool")]
    SetInvocationCost {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(long = "cost", value_name = "MIST")]
        cost: u64,
        #[arg(long = "cashier-admin", value_name = "OBJECT_ID")]
        cashier_admin: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(subcommand, about = "Configure Tool response verification")]
    ConfigureVerifier(VerifierCommand),

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

    #[command(about = "List all registered tools.")]
    List {
        //
    },

    #[command(
        about = "Inspect a registered tool by FQN. Returns the derived Tool and ToolCashier IDs and the complete semantic tool record when it exists."
    )]
    Inspect {
        #[arg(
            long = "tool-fqn",
            short = 't',
            help = "The FQN of the tool to inspect.",
            value_name = "FQN"
        )]
        tool_fqn: ToolFqn,
    },

    #[command(about = "Manage tool auth for signed HTTP.")]
    Auth {
        #[command(subcommand)]
        cmd: ToolAuthCommand,
    },
}

/// Handle the provided tool command. The [`ToolCommand`] instance is passed from
/// [`crate::main`].
pub(crate) async fn handle(command: ToolCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus tool new` ==
        ToolCommand::New {
            name,
            description,
            template,
            target,
        } => create_new_tool(name, description, template, target).await,

        // == `$ nexus tool validate` ==
        ToolCommand::Validate { tool_type } => match tool_type {
            ValidateCommand::Offchain { url } => {
                let meta = validate_off_chain_tool(url).await?;
                output_validation(&meta)
            }
            ValidateCommand::Onchain { ident } => {
                let tool = validate_on_chain_tool(ident).await?;
                output_on_chain_validation(&tool)
            }
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
                tool_witness_id,
                collateral_coin,
                invocation_cost,
                no_save,
                gas,
            } => {
                register_onchain_tool(
                    package,
                    module,
                    tool_fqn,
                    description,
                    timeout,
                    tool_witness_id,
                    collateral_coin,
                    invocation_cost,
                    no_save,
                    gas.sui_gas_coin,
                    gas.sui_gas_budget,
                )
                .await
            }
        },

        // == `$ nexus tool cashier` ==
        ToolCommand::Cashier(command) => handle_cashier(command).await,

        ToolCommand::Unregister {
            tool_fqn,
            owner_cap,
            skip_confirmation,
            gas,
        } => {
            unregister_tool(
                tool_fqn,
                owner_cap,
                skip_confirmation,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        ToolCommand::UpdateUrl {
            tool_fqn,
            url,
            owner_cap,
            gas,
        } => {
            update_url(
                tool_fqn,
                url,
                owner_cap,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        ToolCommand::UpdateMetadata {
            tool_fqn,
            description,
            owner_cap,
            gas,
        } => {
            update_metadata(
                tool_fqn,
                description,
                owner_cap,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        ToolCommand::MigratePackage {
            tool_fqn,
            package,
            owner_cap,
            gas,
        } => {
            update_on_chain_package(
                tool_fqn,
                package,
                owner_cap,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        ToolCommand::UpdateTimeout {
            tool_fqn,
            timeout,
            owner_cap,
            gas,
        } => {
            update_timeout(
                tool_fqn,
                timeout,
                owner_cap,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        ToolCommand::SetInvocationCost {
            tool_fqn,
            cost,
            cashier_admin,
            gas,
        } => {
            set_invocation_cost(
                tool_fqn,
                cost,
                cashier_admin,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        ToolCommand::ConfigureVerifier(command) => configure_verifier(command).await,

        // == `$ nexus tool claim-collateral` ==
        ToolCommand::ClaimCollateral {
            tool_fqn,
            owner_cap,
            gas,
        } => claim_collateral(tool_fqn, owner_cap, gas.sui_gas_coin, gas.sui_gas_budget).await,

        // == `$ nexus tool list` ==
        ToolCommand::List { .. } => list_tools().await,

        // == `$ nexus tool inspect` ==
        ToolCommand::Inspect { tool_fqn } => inspect_tool(tool_fqn).await,

        // == `$ nexus tool auth` ==
        ToolCommand::Auth { cmd } => handle_tool_auth(cmd).await,
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory as _, Parser};

    #[test]
    fn inspect_help_describes_the_semantic_record() {
        let command = crate::Cli::command();
        let tool = command
            .find_subcommand("tool")
            .expect("tool command should exist");
        let inspect = tool
            .find_subcommand("inspect")
            .expect("inspect command should exist");
        let about = inspect
            .get_about()
            .expect("inspect command should have help text")
            .to_string();

        assert!(about.contains("complete semantic tool record"));
        assert!(!about.contains("full onchain"));
    }

    #[test]
    fn onchain_registration_rejects_manual_authorization_mode() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "register",
            "onchain",
            "--package",
            "0x1",
            "--module",
            "tool",
            "--tool-fqn",
            "com.example.tool@1",
            "--description",
            "example",
            "--tool-witness-id",
            "0x2",
            "--workflow-authorization-cap-first",
        ])
        .is_err());
    }

    #[test]
    fn onchain_validation_rejects_invalid_tool_fqn() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "validate",
            "onchain",
            "--ident",
            "not-a-tool-fqn",
        ])
        .is_err());
    }
}
