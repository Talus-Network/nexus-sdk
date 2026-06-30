mod tap_agent;
mod tap_bind;
mod tap_common;
mod tap_create_agent;
mod tap_create_skill_artifact;
mod tap_default_agent;
mod tap_dry_run;
mod tap_execute;
mod tap_output;
mod tap_payments;
mod tap_publish_skill;
mod tap_register_skill;
mod tap_registry;
mod tap_requirements;
mod tap_scaffold;
mod tap_schedule_task;
mod tap_scheduled_task;
mod tap_settle;
mod tap_update_skill;
mod tap_validate_skill;
mod tap_vault;
mod tap_vault_deposit;

#[cfg(test)]
use tap_validate_skill::{
    collect_move_source_files,
    validate_tap_package_manifest,
    validate_tap_package_sources,
};
use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
        workflow,
    },
    convert_case::{Case, Casing},
    nexus_sdk::{
        dag::validator as dag_validator,
        nexus::{
            error::NexusError,
            tap::{
                fetch_agent_payment_vault_for_agent,
                fetch_execution_payment_history,
                AgentTaskStateAction,
                CreateAgentResult,
                GetSkillRequirementsResult,
                PublishSkillResult,
                RegisterSkillResult,
                TapPackagePublishOptions,
                UpdateSkillResult,
            },
            workflow::AgentDagExecuteOptions,
        },
        types::{
            Dag as JsonDag,
            ExecutionPaymentReceipt,
            SkillConfig,
            SkillId,
            TapPublishArtifact,
            DEFAULT_ENTRY_GROUP,
        },
    },
    regex::Regex,
    tap_agent::handle_agent_command,
    tap_bind::bind_agent_skill,
    tap_common::{
        agent_execute_options_from_cli,
        agent_id_from_alias_or_arg,
        ensure_cli_agent_owner,
        ensure_cli_mutable_agent,
        read_artifact,
        schedule_policy_from_cli,
    },
    tap_create_agent::create_agent,
    tap_create_skill_artifact::{create_skill_artifact, ArtifactPaymentMode},
    tap_default_agent::show_default_agent,
    tap_dry_run::dry_run_skill,
    tap_execute::execute_agent_dag_skill,
    tap_output::{
        agent_execute_result_json,
        agent_list_result_json,
        agent_remove_result_json,
        agent_save_result_json,
        bind_result_json,
        create_agent_result_json,
        create_skill_artifact_result_json,
        default_agent_result_json,
        dry_run_result_json,
        payment_resolve_result_json,
        payment_show_result_json,
        payment_wait_result_json,
        payments_list_result_json,
        publish_skill_result_json,
        register_skill_result_json,
        registry_show_result_json,
        requirements_result_json,
        scaffold_result_json,
        schedule_task_result_json,
        update_skill_result_json,
        validate_skill_result_json,
        vault_balance_result_json,
        vault_deposit_result_json,
    },
    tap_payments::handle_payments_command,
    tap_publish_skill::publish_skill,
    tap_register_skill::register_skill,
    tap_registry::show_registry,
    tap_requirements::fetch_requirements,
    tap_scaffold::scaffold_tap_skill,
    tap_schedule_task::{schedule_tap_task, TapTaskPaymentSourceArg},
    tap_scheduled_task::set_agent_task_state,
    tap_settle::handle_execution_command,
    tap_update_skill::update_skill_from_artifact,
    tap_validate_skill::{resolve_relative, validate_skill},
    tap_vault::handle_vault_command,
    tap_vault_deposit::deposit_agent_vault,
    tokio::fs::create_dir_all,
};

#[derive(Subcommand)]
pub(crate) enum TapCommand {
    #[command(about = "Scaffold a TAP package and DAG-backed skill config.")]
    Scaffold {
        #[arg(long, short, help = "Skill/package name.", value_name = "NAME")]
        name: String,
        #[arg(
            long,
            help = "Directory where the TAP package folder will be created.",
            value_parser = ValueParser::from(expand_tilde),
            default_value = "."
        )]
        target: PathBuf,
    },
    #[command(about = "Validate a TAP skill config and local TAP package metadata.")]
    ValidateSkill {
        #[arg(
            long,
            short,
            help = "Path to the TAP skill config JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        config: PathBuf,
    },
    #[command(about = "Publish a TAP package, DAG, and publish artifact.")]
    PublishSkill {
        #[arg(
            long,
            short,
            help = "Path to the TAP skill config JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        config: PathBuf,
        #[arg(
            long,
            help = "Write the publish artifact JSON to this path.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        out: Option<PathBuf>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(
        about = "Create a TAP skill publish artifact from explicit inputs and fetched DAG data."
    )]
    CreateSkillArtifact {
        #[arg(
            long,
            help = "Skill name recorded in the artifact.",
            value_name = "NAME"
        )]
        skill_name: String,
        #[arg(long, help = "Published DAG object ID.", value_name = "OBJECT_ID")]
        dag_id: sui::types::Address,
        #[arg(long, help = "Skill interface revision.", value_name = "U64")]
        interface_revision: u64,
        #[arg(
            long = "payment-mode",
            value_enum,
            help = "Skill payment policy mode.",
            value_name = "MODE"
        )]
        payment_mode: ArtifactPaymentMode,
        #[arg(
            long = "agent-funded-max-budget",
            help = "Maximum budget for agent-funded skills.",
            value_name = "AMOUNT"
        )]
        agent_funded_max_budget: Option<u64>,
        #[arg(long, default_value = "once", help = "Schedule recurrence kind.")]
        recurrence_kind: String,
        #[arg(long, default_value_t = 0, help = "Minimum interval in milliseconds.")]
        min_interval_ms: u64,
        #[arg(long, default_value_t = 1, help = "Maximum occurrences.")]
        max_occurrences: u64,
        #[arg(long, default_value_t = false, help = "Allow recursive execution.")]
        allow_recursive: bool,
        #[arg(
            long = "fixed-tool",
            help = "Fixed tool requirement as <TOOL_REGISTRY_ID>=<TOOL_FQN>. Repeatable.",
            value_name = "REGISTRY=FQN"
        )]
        fixed_tool: Vec<String>,
        #[arg(
            long,
            help = "Write the publish artifact JSON to this path.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        out: PathBuf,
    },
    #[command(about = "Create a standard Talus agent.")]
    CreateAgent {
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Register a TAP skill from a publish artifact.")]
    RegisterSkill {
        #[arg(
            long,
            help = "Path to the publish artifact JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        artifact: PathBuf,
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Update an existing TAP skill from a publish artifact.")]
    UpdateSkill {
        #[arg(
            long,
            help = "Path to the publish artifact JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        artifact: PathBuf,
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "Agent-local generated skill index.", value_name = "U64")]
        skill_id: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(subcommand, about = "Manage locally saved Talus agent aliases.")]
    Agent(AgentCommand),
    #[command(subcommand, about = "Inspect Talus agent payment vaults.")]
    Vault(VaultCommand),
    #[command(
        subcommand,
        about = "Inspect standard TAP execution payments and history."
    )]
    Payments(PaymentsCommand),
    #[command(subcommand, about = "Settle or abort active TAP executions.")]
    Execution(SettleCommand),
    #[command(subcommand, about = "Inspect the agent registry.")]
    Registry(RegistryCommand),
    #[command(subcommand, about = "Inspect the standard TAP default agent metadata.")]
    DefaultAgent(DefaultAgentCommand),
    #[command(about = "Create a Talus agent and register its first skill atomically.")]
    Bind {
        #[arg(
            long,
            help = "Path to the publish artifact JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        artifact: PathBuf,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Fetch live skill requirements from the agent registry.")]
    Requirements {
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "Agent-local generated skill index.", value_name = "U64")]
        skill_id: u64,
    },
    #[command(about = "Dry-run user inputs against a TAP skill config.")]
    DryRun {
        #[arg(
            long,
            short,
            help = "Path to the TAP skill config JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        config: PathBuf,
    },
    #[command(about = "Execute a standard TAP skill through its active DAG endpoint.")]
    Execute {
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "Agent-local generated skill index.", value_name = "U64")]
        skill_id: u64,
        #[arg(
            long = "entry-group",
            short = 'e',
            help = "DAG entry group to invoke.",
            value_name = "NAME",
            default_value = DEFAULT_ENTRY_GROUP,
        )]
        entry_group: String,
        #[arg(
            long = "input-json",
            short = 'i',
            help = "Initial DAG input data as a JSON object.",
            value_parser = ValueParser::from(parse_json_string),
            value_name = "DATA"
        )]
        input_json: serde_json::Value,
        #[arg(
            long = "remote",
            short = 'r',
            help = "Comma-separated {vertex}.{port} inputs to store remotely.",
            value_delimiter = ',',
            value_name = "VERTEX.PORT"
        )]
        remote: Vec<String>,
        #[arg(
            long = "priority-fee-per-gas-unit",
            help = "Priority fee per gas unit for the DAG execution. Defaults to 0 when omitted.",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        priority_fee_per_gas_unit: u64,
        #[arg(
            long = "payment-source-hex",
            help = "Payment source bytes as hex.",
            value_name = "HEX",
            default_value = ""
        )]
        payment_source_hex: String,
        #[arg(
            long = "payment-max-budget",
            help = "Maximum standard TAP payment budget.",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        payment_max_budget: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Create a scheduled task for a TAP skill and attach TAP payment.")]
    ScheduleTask {
        #[arg(long, help = "Talus agent object ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "Agent-local generated skill index.", value_name = "U64")]
        skill_id: u64,
        #[arg(
            long = "entry-group",
            short = 'e',
            help = "DAG entry group to invoke.",
            value_name = "NAME",
            default_value = DEFAULT_ENTRY_GROUP,
        )]
        entry_group: String,
        #[arg(
            long = "dag-id",
            help = "Optional DAG selection for runtime-selected skills. Pinned skills validate this against their pinned DAG when provided.",
            value_name = "OBJECT_ID"
        )]
        dag_id: Option<sui::types::Address>,
        #[arg(
            long = "input-json",
            short = 'i',
            help = "Initial DAG input data as a JSON object.",
            value_parser = ValueParser::from(parse_json_string),
            value_name = "DATA"
        )]
        input_json: Option<serde_json::Value>,
        #[arg(
            long = "remote",
            short = 'r',
            help = "Comma-separated {vertex}.{port} inputs to store remotely.",
            value_delimiter = ',',
            value_name = "VERTEX.PORT"
        )]
        remote: Vec<String>,
        #[arg(long = "metadata", short = 'm', value_name = "KEY=VALUE")]
        metadata: Vec<String>,
        #[arg(
            long = "execution-priority-fee-per-gas-unit",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        execution_priority_fee_per_gas_unit: u64,
        #[command(flatten)]
        schedule_start: crate::scheduler::task::ScheduleStartOptions,
        #[command(flatten)]
        schedule_deadline: crate::scheduler::task::ScheduleDeadlineOptions,
        #[arg(
            long = "schedule-priority-fee-per-gas-unit",
            value_name = "AMOUNT",
            default_value_t = 0u64
        )]
        schedule_priority_fee_per_gas_unit: u64,
        #[arg(
            long = "generator",
            value_enum,
            default_value_t = crate::scheduler::task::TaskGeneratorArg::Queue,
            value_name = "KIND"
        )]
        generator: crate::scheduler::task::TaskGeneratorArg,
        #[arg(long = "payment-source", value_enum, value_name = "SOURCE")]
        payment_source: TapTaskPaymentSourceArg,
        #[arg(long = "prepay-amount", value_name = "AMOUNT")]
        prepay_amount: u64,
        #[arg(long = "refund-recipient", value_name = "ADDRESS")]
        refund_recipient: Option<sui::types::Address>,
        #[arg(long = "occurrence-budget", value_name = "AMOUNT")]
        occurrence_budget: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(
        subcommand,
        about = "Operate an existing explicit-agent TAP scheduled task."
    )]
    ScheduledTask(ScheduledTaskCommand),
}

#[derive(Subcommand)]
pub(crate) enum ScheduledTaskCommand {
    #[command(about = "Pause a TAP scheduled task.")]
    Pause {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, help = "Talus agent object ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Resume a TAP scheduled task.")]
    Resume {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, help = "Talus agent object ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Cancel a TAP scheduled task.")]
    Cancel {
        #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
        task_id: sui::types::Address,
        #[arg(long, help = "Talus agent object ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum AgentCommand {
    #[command(about = "Save a Talus agent under a local alias.")]
    Save {
        #[arg(long, help = "Local alias.", value_name = "NAME")]
        name: String,
        #[arg(long, help = "Talus agent object ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
    },
    #[command(about = "List locally saved Talus agent aliases.")]
    List,
    #[command(about = "Remove a locally saved Talus agent alias.")]
    Remove {
        #[arg(long, help = "Local alias.", value_name = "NAME")]
        name: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum VaultCommand {
    #[command(about = "Show a Talus agent payment-vault balance.")]
    Balance {
        #[arg(
            long,
            help = "Local agent alias.",
            value_name = "NAME",
            conflicts_with = "agent_id"
        )]
        alias: Option<String>,
        #[arg(long, help = "Talus agent object ID.", value_name = "OBJECT_ID")]
        agent_id: Option<sui::types::Address>,
    },
    #[command(about = "Deposit MIST into a Talus agent payment vault.")]
    Deposit {
        #[arg(
            long,
            help = "Local agent alias.",
            value_name = "NAME",
            conflicts_with = "agent_id"
        )]
        alias: Option<String>,
        #[arg(long, help = "Talus agent object ID.", value_name = "OBJECT_ID")]
        agent_id: Option<sui::types::Address>,
        #[arg(long, help = "Amount in MIST to deposit.", value_name = "AMOUNT")]
        amount: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum RegistryCommand {
    #[command(about = "Print the agent registry contents as JSON.")]
    Show,
}

#[derive(Subcommand)]
pub(crate) enum DefaultAgentCommand {
    #[command(about = "Print the configured standard TAP default agent as JSON.")]
    Show,
}

#[derive(Subcommand)]
pub(crate) enum PaymentsCommand {
    #[command(about = "Show a standard TAP execution payment by ID.")]
    Show {
        #[arg(
            long = "payment-id",
            help = "Execution payment object ID.",
            value_name = "OBJECT_ID"
        )]
        payment_id: sui::types::Address,
    },
    #[command(about = "Wait for a standard TAP execution payment to settle.")]
    Wait {
        #[arg(
            long = "payment-id",
            help = "Execution payment object ID.",
            value_name = "OBJECT_ID"
        )]
        payment_id: sui::types::Address,
        #[arg(
            long = "timeout-secs",
            default_value_t = 120u64,
            help = "Maximum seconds to wait."
        )]
        timeout_secs: u64,
        #[arg(
            long = "poll-secs",
            default_value_t = 2u64,
            help = "Seconds between polls."
        )]
        poll_secs: u64,
    },
    #[command(about = "List wallet-owned and optional agent-vault execution payment receipts.")]
    List {
        #[arg(
            long,
            help = "Local agent alias.",
            value_name = "NAME",
            conflicts_with = "agent_id"
        )]
        alias: Option<String>,
        #[arg(
            long,
            help = "Talus agent object ID for vault history.",
            value_name = "OBJECT_ID"
        )]
        agent_id: Option<sui::types::Address>,
        #[arg(
            long,
            help = "Show only completed/resolved receipts.",
            conflicts_with = "pending"
        )]
        completed: bool,
        #[arg(
            long,
            help = "Show only pending/unresolved receipts.",
            conflicts_with = "completed"
        )]
        pending: bool,
        #[arg(long, help = "Show all receipts. This is the default.")]
        all: bool,
    },
    #[command(
        about = "Resolve a standard TAP payment by returning funds to the invoker given the execution is finished. Routes through `accomplish_tap_execution_payment_from_agent_vault` when `--alias`/`--agent-id` is supplied, otherwise calls `accomplish_tap_execution_payment`."
    )]
    Resolve {
        #[arg(
            long = "execution-id",
            help = "Shared `DAGExecution` object ID whose TAP payment should be accomplished.",
            value_name = "OBJECT_ID"
        )]
        execution_id: sui::types::Address,
        #[arg(
            long,
            help = "Local agent alias whose vault funds the settlement.",
            value_name = "NAME",
            conflicts_with = "agent_id"
        )]
        alias: Option<String>,
        #[arg(
            long,
            help = "Talus agent object ID whose vault funds the settlement.",
            value_name = "OBJECT_ID"
        )]
        agent_id: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(
        about = "Refill a live TAP execution payment. Uses a coin top-up by default, or the agent vault when `--alias`/`--agent-id` is supplied."
    )]
    Refill {
        #[arg(
            long = "execution-id",
            help = "Shared `DAGExecution` object ID whose TAP payment should be refilled.",
            value_name = "OBJECT_ID"
        )]
        execution_id: sui::types::Address,
        #[arg(long, help = "MIST amount to add to the execution payment.")]
        amount: u64,
        #[arg(
            long,
            help = "Local agent alias whose vault funds the refill.",
            value_name = "NAME",
            conflicts_with = "agent_id"
        )]
        alias: Option<String>,
        #[arg(
            long,
            help = "Talus agent object ID whose vault funds the refill.",
            value_name = "OBJECT_ID"
        )]
        agent_id: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum SettleCommand {
    #[command(about = "Permissionlessly settle one committed result walk after it is eligible.")]
    Settle {
        #[arg(
            long = "execution-id",
            help = "Shared `DAGExecution` object ID to settle.",
            value_name = "OBJECT_ID"
        )]
        execution_id: sui::types::Address,
        #[arg(long = "walk-index", help = "Committed-result walk index to settle.")]
        walk_index: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Permissionlessly abort an expired TAP DAG execution.")]
    Abort {
        #[arg(
            long = "execution-id",
            help = "Shared `DAGExecution` object ID to abort.",
            value_name = "OBJECT_ID"
        )]
        execution_id: sui::types::Address,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: TapCommand) -> AnyResult<(), NexusCliError> {
    match command {
        TapCommand::Scaffold { name, target } => scaffold_tap_skill(name, target).await,
        TapCommand::ValidateSkill { config } => validate_skill(config).await.map(|_| ()),
        TapCommand::PublishSkill { config, out, gas } => {
            publish_skill(config, out, gas.sui_gas_coin, gas.sui_gas_budget).await
        }
        TapCommand::CreateSkillArtifact {
            skill_name,
            dag_id,
            interface_revision,
            payment_mode,
            agent_funded_max_budget,
            recurrence_kind,
            min_interval_ms,
            max_occurrences,
            allow_recursive,
            fixed_tool,
            out,
        } => {
            create_skill_artifact(
                skill_name,
                dag_id,
                interface_revision,
                payment_mode,
                agent_funded_max_budget,
                recurrence_kind,
                min_interval_ms,
                max_occurrences,
                allow_recursive,
                fixed_tool,
                out,
            )
            .await
        }
        TapCommand::CreateAgent { gas } => create_agent(gas.sui_gas_coin, gas.sui_gas_budget).await,
        TapCommand::RegisterSkill {
            artifact,
            agent_id,
            gas,
        } => register_skill(artifact, agent_id, gas.sui_gas_coin, gas.sui_gas_budget).await,
        TapCommand::UpdateSkill {
            artifact,
            agent_id,
            skill_id,
            gas,
        } => {
            update_skill_from_artifact(
                artifact,
                agent_id,
                skill_id,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::Agent(command) => handle_agent_command(command).await,
        TapCommand::Vault(command) => handle_vault_command(command).await,
        TapCommand::Payments(command) => handle_payments_command(command).await,
        TapCommand::Execution(command) => handle_execution_command(command).await,
        TapCommand::Registry(RegistryCommand::Show) => show_registry().await,
        TapCommand::DefaultAgent(DefaultAgentCommand::Show) => show_default_agent().await,
        TapCommand::Bind { artifact, gas } => {
            bind_agent_skill(artifact, gas.sui_gas_coin, gas.sui_gas_budget).await
        }
        TapCommand::Requirements { agent_id, skill_id } => {
            fetch_requirements(agent_id, skill_id).await
        }
        TapCommand::DryRun { config } => dry_run_skill(config).await,
        TapCommand::Execute {
            agent_id,
            skill_id,
            entry_group,
            input_json,
            remote,
            priority_fee_per_gas_unit,
            payment_source_hex,
            payment_max_budget,
            gas,
        } => {
            execute_agent_dag_skill(
                agent_id,
                skill_id,
                entry_group,
                input_json,
                remote,
                priority_fee_per_gas_unit,
                payment_source_hex,
                payment_max_budget,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::ScheduleTask {
            agent_id,
            skill_id,
            entry_group,
            dag_id,
            input_json,
            remote,
            metadata,
            execution_priority_fee_per_gas_unit,
            schedule_start,
            schedule_deadline,
            schedule_priority_fee_per_gas_unit,
            generator,
            payment_source,
            prepay_amount,
            refund_recipient,
            occurrence_budget,
            gas,
        } => {
            let crate::scheduler::task::ScheduleStartOptions {
                schedule_start_ms,
                schedule_start_offset_ms,
            } = schedule_start;
            let crate::scheduler::task::ScheduleDeadlineOptions {
                schedule_deadline_offset_ms,
            } = schedule_deadline;
            schedule_tap_task(
                agent_id,
                skill_id,
                entry_group,
                dag_id,
                input_json,
                remote,
                metadata,
                execution_priority_fee_per_gas_unit,
                schedule_start_ms,
                schedule_start_offset_ms,
                schedule_deadline_offset_ms,
                schedule_priority_fee_per_gas_unit,
                generator.into(),
                payment_source,
                prepay_amount,
                refund_recipient,
                occurrence_budget,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::ScheduledTask(command) => match command {
            ScheduledTaskCommand::Pause {
                task_id,
                agent_id,
                gas,
            } => set_agent_task_state(task_id, agent_id, gas, AgentTaskStateAction::Pause).await,
            ScheduledTaskCommand::Resume {
                task_id,
                agent_id,
                gas,
            } => set_agent_task_state(task_id, agent_id, gas, AgentTaskStateAction::Resume).await,
            ScheduledTaskCommand::Cancel {
                task_id,
                agent_id,
                gas,
            } => set_agent_task_state(task_id, agent_id, gas, AgentTaskStateAction::Cancel).await,
        },
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
mod tests {
    use {
        super::*,
        assert_matches::assert_matches,
        nexus_sdk::types::{
            InterfaceVersion,
            SkillPaymentPolicy,
            SkillRequirements,
            SkillSchedulePolicy,
        },
        std::ffi::OsString,
    };

    struct EnvGuard {
        home: Option<OsString>,
        rpc: Option<OsString>,
        pk: Option<OsString>,
    }

    impl EnvGuard {
        fn without_sui_credentials(path: &std::path::Path) -> Self {
            let guard = Self {
                home: std::env::var_os("HOME"),
                rpc: std::env::var_os("SUI_RPC_URL"),
                pk: std::env::var_os("SUI_PK"),
            };
            std::env::set_var("HOME", path);
            std::env::remove_var("SUI_RPC_URL");
            std::env::remove_var("SUI_PK");
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.home.take() {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            match self.rpc.take() {
                Some(value) => std::env::set_var("SUI_RPC_URL", value),
                None => std::env::remove_var("SUI_RPC_URL"),
            }
            match self.pk.take() {
                Some(value) => std::env::set_var("SUI_PK", value),
                None => std::env::remove_var("SUI_PK"),
            }
        }
    }

    fn gas_args() -> GasArgs {
        GasArgs {
            sui_gas_coin: None,
            sui_gas_budget: DEFAULT_GAS_BUDGET,
        }
    }

    fn test_artifact() -> TapPublishArtifact {
        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            requirements: SkillRequirements {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceVersion::new(1),
        };

        TapPublishArtifact::from_config(&config, sui::types::Address::from_static("0xd"))
            .expect("valid artifact")
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn handle_dispatches_all_tap_command_variants_to_local_boundaries() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::without_sui_credentials(temp_home.path());
        let tempdir = tempfile::tempdir().expect("tempdir");

        handle(TapCommand::Scaffold {
            name: "weather skill".to_string(),
            target: tempdir.path().to_path_buf(),
        })
        .await
        .expect("scaffold dispatch succeeds");
        let root = tempdir.path().join("weather-skill");
        let config = root.join("skill.tap.json");

        handle(TapCommand::ValidateSkill {
            config: config.clone(),
        })
        .await
        .expect("validate dispatch succeeds");

        let publish_error = handle(TapCommand::PublishSkill {
            config: config.clone(),
            out: None,
            gas: gas_args(),
        })
        .await
        .expect_err("publish dispatch reaches missing RPC");
        assert!(publish_error
            .to_string()
            .contains("Sui RPC URL is not configured"));

        let dispatch_artifact_path = root.join("dispatch-skill-artifact.json");
        let create_artifact_error = handle(TapCommand::CreateSkillArtifact {
            skill_name: "weather skill".to_string(),
            dag_id: sui::types::Address::from_static("0xd"),
            interface_revision: 1,
            payment_mode: ArtifactPaymentMode::UserFunded,
            agent_funded_max_budget: None,
            recurrence_kind: "once".to_string(),
            min_interval_ms: 0,
            max_occurrences: 1,
            allow_recursive: false,
            fixed_tool: Vec::new(),
            out: dispatch_artifact_path,
        })
        .await
        .expect_err("create-skill-artifact dispatch reaches missing RPC");
        assert!(create_artifact_error
            .to_string()
            .contains("Sui RPC URL is not configured"));

        let missing_rpc = handle(TapCommand::CreateAgent { gas: gas_args() })
            .await
            .expect_err("create-agent dispatch reaches missing RPC");
        assert!(missing_rpc
            .to_string()
            .contains("Sui RPC URL is not configured"));

        let artifact_path = root.join("artifact.json");
        tokio::fs::write(
            &artifact_path,
            serde_json::to_string(&test_artifact()).expect("serialize artifact"),
        )
        .await
        .expect("write artifact");
        let register_error = handle(TapCommand::RegisterSkill {
            artifact: artifact_path.clone(),
            agent_id: sui::types::Address::from_static("0xa"),
            gas: gas_args(),
        })
        .await
        .expect_err("register dispatch reaches missing RPC");
        assert!(register_error
            .to_string()
            .contains("Sui RPC URL is not configured"));

        let update_error = handle(TapCommand::UpdateSkill {
            artifact: root.join("missing-artifact.json"),
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            gas: gas_args(),
        })
        .await
        .expect_err("update skill dispatch reads artifact first");
        assert!(
            update_error.to_string().contains("No such file")
                || update_error.to_string().contains("not found")
        );

        handle(TapCommand::Agent(AgentCommand::Save {
            name: "primary".to_string(),
            agent_id: sui::types::Address::from_static("0xa"),
        }))
        .await
        .expect("agent save dispatch succeeds");
        handle(TapCommand::Agent(AgentCommand::List))
            .await
            .expect("agent list dispatch succeeds");

        let vault_error = handle(TapCommand::Vault(VaultCommand::Balance {
            alias: Some("missing".to_string()),
            agent_id: None,
        }))
        .await
        .expect_err("vault dispatch resolves alias before RPC");
        assert!(vault_error.to_string().contains("No Talus agent alias"));

        let payments_error = handle(TapCommand::Payments(PaymentsCommand::List {
            alias: Some("missing".to_string()),
            agent_id: None,
            completed: false,
            pending: true,
            all: false,
        }))
        .await
        .expect_err("payments dispatch resolves alias before RPC");
        assert!(payments_error.to_string().contains("No Talus agent alias"));

        let resolve_error = handle(TapCommand::Payments(PaymentsCommand::Resolve {
            execution_id: sui::types::Address::from_static("0xee"),
            alias: None,
            agent_id: None,
            gas: gas_args(),
        }))
        .await
        .expect_err("payments resolve dispatch reaches missing RPC");
        assert!(
            resolve_error
                .to_string()
                .contains("Sui RPC URL is not configured"),
            "unexpected error: {resolve_error}"
        );

        let resolve_vault_error = handle(TapCommand::Payments(PaymentsCommand::Resolve {
            execution_id: sui::types::Address::from_static("0xee"),
            alias: Some("missing".to_string()),
            agent_id: None,
            gas: gas_args(),
        }))
        .await
        .expect_err("payments resolve --alias resolves before RPC");
        assert!(
            resolve_vault_error
                .to_string()
                .contains("No Talus agent alias"),
            "unexpected error: {resolve_vault_error}"
        );

        let refill_error = handle(TapCommand::Payments(PaymentsCommand::Refill {
            execution_id: sui::types::Address::from_static("0xee"),
            amount: 100,
            alias: None,
            agent_id: None,
            gas: gas_args(),
        }))
        .await
        .expect_err("payments refill dispatch reaches missing RPC");
        assert!(
            refill_error
                .to_string()
                .contains("Sui RPC URL is not configured"),
            "unexpected error: {refill_error}"
        );

        let refill_vault_error = handle(TapCommand::Payments(PaymentsCommand::Refill {
            execution_id: sui::types::Address::from_static("0xee"),
            amount: 100,
            alias: Some("missing".to_string()),
            agent_id: None,
            gas: gas_args(),
        }))
        .await
        .expect_err("payments refill --alias resolves before RPC");
        assert!(
            refill_vault_error
                .to_string()
                .contains("No Talus agent alias"),
            "unexpected error: {refill_vault_error}"
        );

        let execution_settle_error = handle(TapCommand::Execution(SettleCommand::Settle {
            execution_id: sui::types::Address::from_static("0xee"),
            walk_index: 0,
            gas: gas_args(),
        }))
        .await
        .expect_err("execution settle dispatch reaches missing RPC");
        assert!(
            execution_settle_error
                .to_string()
                .contains("Sui RPC URL is not configured"),
            "unexpected error: {execution_settle_error}"
        );

        let execution_abort_error = handle(TapCommand::Execution(SettleCommand::Abort {
            execution_id: sui::types::Address::from_static("0xee"),
            gas: gas_args(),
        }))
        .await
        .expect_err("execution abort dispatch reaches missing RPC");
        assert!(
            execution_abort_error
                .to_string()
                .contains("Sui RPC URL is not configured"),
            "unexpected error: {execution_abort_error}"
        );

        let default_agent_error = handle(TapCommand::DefaultAgent(DefaultAgentCommand::Show))
            .await
            .expect_err("default-agent dispatch reaches missing RPC");
        assert!(default_agent_error
            .to_string()
            .contains("Sui RPC URL is not configured"));

        let requirements_error = handle(TapCommand::Requirements {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
        })
        .await
        .expect_err("requirements dispatch reaches missing RPC");
        assert!(requirements_error
            .to_string()
            .contains("Sui RPC URL is not configured"));

        handle(TapCommand::DryRun {
            config: config.clone(),
        })
        .await
        .expect("dry-run dispatch succeeds");

        let execute_error = handle(TapCommand::Execute {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            entry_group: DEFAULT_ENTRY_GROUP.to_string(),
            input_json: serde_json::json!({}),
            remote: Vec::new(),
            priority_fee_per_gas_unit: 0,
            payment_source_hex: "0xinvalid".to_string(),
            payment_max_budget: 0,
            gas: gas_args(),
        })
        .await
        .expect_err("execute dispatch decodes payment source first");
        assert!(execute_error
            .to_string()
            .contains("invalid payment-source hex"));

        let vault_deposit_error = handle(TapCommand::Vault(VaultCommand::Deposit {
            alias: None,
            agent_id: Some(sui::types::Address::from_static("0xa")),
            amount: 1000,
            gas: gas_args(),
        }))
        .await
        .expect_err("vault deposit dispatch reaches missing RPC");
        assert!(vault_deposit_error
            .to_string()
            .contains("Sui RPC URL is not configured"));
    }

    #[tokio::test]
    async fn scaffold_and_validate_skill_flow() {
        let tempdir = tempfile::tempdir().unwrap().keep();

        scaffold_tap_skill("weather skill".to_string(), tempdir.clone())
            .await
            .expect("scaffold succeeds");

        let root = tempdir.join("weather-skill");
        assert!(root.join("tap/Move.toml").exists());
        assert!(root.join("dag.json").exists());
        assert!(root.join("skill.tap.json").exists());

        let config_path = root.join("skill.tap.json");
        let config = validate_skill(config_path.clone())
            .await
            .expect("generated config validates");
        assert_eq!(config.name, "weather skill");
        assert_eq!(config.interface_revision, InterfaceVersion::new(1));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn create_skill_artifact_aborts_before_write_when_dag_cannot_be_fetched() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::without_sui_credentials(temp_home.path());
        let tempdir = tempfile::tempdir().unwrap().keep();
        let artifact_path = tempdir.join("artifact.json");

        let error = create_skill_artifact(
            "weather skill".to_string(),
            sui::types::Address::from_static("0xd"),
            1,
            ArtifactPaymentMode::UserFunded,
            None,
            "once".to_string(),
            0,
            1,
            false,
            Vec::new(),
            artifact_path.clone(),
        )
        .await
        .expect_err("missing RPC aborts before writing");

        assert!(
            error.to_string().contains("Sui RPC URL is not configured"),
            "unexpected error: {error}"
        );
        assert!(!artifact_path.exists());
    }

    #[test]
    fn update_skill_result_exposes_skill_update_revision() {
        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            requirements: SkillRequirements {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceVersion::new(1),
        };
        let artifact =
            TapPublishArtifact::from_config(&config, sui::types::Address::from_static("0xd"))
                .expect("valid artifact");
        let output = update_skill_result_json(
            &artifact,
            &UpdateSkillResult {
                tx_digest: sui::types::Digest::from([7; 32]),
                tx_checkpoint: 42,
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                current_interface_revision: InterfaceVersion::new(3),
                dag_binding: nexus_sdk::types::SkillDagBinding::pinned(artifact.dag_id),
                requirements: artifact.requirements.clone(),
            },
        );

        assert_eq!(output["function"], "update_skill");
        assert_eq!(output["current_interface_revision"], serde_json::json!(3));
        assert!(output.get("config_digest_hex").is_none());
    }

    #[tokio::test]
    async fn validate_skill_rejects_missing_tap_package() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let config = SkillConfig {
            name: "bad".to_string(),
            dag_path: PathBuf::from("dag.json"),
            requirements: SkillRequirements {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceVersion::new(1),
        };
        let config_path = tempdir.join("skill.tap.json");
        tokio::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .await
            .unwrap();
        tokio::fs::write(tempdir.join("dag.json"), "{}")
            .await
            .unwrap();

        assert_matches!(validate_skill(config_path).await, Err(_));
    }

    #[tokio::test]
    async fn validate_skill_rejects_invalid_dag_graph() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        scaffold_tap_skill("weather skill".to_string(), tempdir.clone())
            .await
            .expect("scaffold succeeds");

        let root = tempdir.join("weather-skill");
        tokio::fs::write(
            root.join("dag.json"),
            "{\n  \"vertices\": [],\n  \"edges\": []\n}",
        )
        .await
        .unwrap();

        let error = validate_skill(root.join("skill.tap.json"))
            .await
            .expect_err("invalid DAG must fail validation");
        assert!(
            error
                .to_string()
                .contains("The DAG has no entry vertices or ports."),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn validate_skill_rejects_mismatched_move_package_name() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        scaffold_tap_skill("weather skill".to_string(), tempdir.clone())
            .await
            .expect("scaffold succeeds");

        let root = tempdir.join("weather-skill");
        tokio::fs::write(
            root.join("tap/Move.toml"),
            r#"[package]
name = "other_tap"
version = "1.0.0"
edition = "2024"

[dependencies]
nexus_interface = { local = "../../nexus/sui/interface" }
nexus_workflow = { local = "../../nexus/sui/workflow" }
nexus_primitives = { local = "../../nexus/sui/primitives" }

[environments]
testnet = "00000000"
"#,
        )
        .await
        .unwrap();

        let error = validate_skill(root.join("skill.tap.json"))
            .await
            .expect_err("package name mismatch must fail validation");
        assert!(
            error.to_string().contains(
                "TAP package 'other_tap' has no source file declaring `module other_tap::...;`"
            ),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn validate_skill_rejects_missing_named_address_alias() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        scaffold_tap_skill("weather skill".to_string(), tempdir.clone())
            .await
            .expect("scaffold succeeds");

        let root = tempdir.join("weather-skill");
        // `validate-skill` now enforces the new-style 2024 layout, so a
        // manifest carrying [addresses] is rejected up front (the section is
        // an old-style marker that breaks dep resolution against new-style
        // published packages, regardless of whether the alias matches).
        tokio::fs::write(
            root.join("tap/Move.toml"),
            r#"[package]
name = "weather_skill"
version = "1.0.0"
edition = "2024"

[dependencies]
nexus_interface = { local = "deps/interface" }
nexus_workflow = { local = "deps/workflow" }
nexus_primitives = { local = "deps/primitives" }

[addresses]
other_alias = "0x0"

[environments]
localnet = "6c457631"
"#,
        )
        .await
        .unwrap();

        let error = validate_skill(root.join("skill.tap.json"))
            .await
            .expect_err("[addresses] section must fail validation");
        assert!(
            error.to_string().contains("[addresses]"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn validate_skill_rejects_missing_package_module_declaration() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        scaffold_tap_skill("weather skill".to_string(), tempdir.clone())
            .await
            .expect("scaffold succeeds");

        let root = tempdir.join("weather-skill");
        tokio::fs::write(
            root.join("tap/sources/weather_skill.move"),
            r#"module other_alias::weather_skill;

public struct WeatherSkill has drop {}
"#,
        )
        .await
        .unwrap();

        let error = validate_skill(root.join("skill.tap.json"))
            .await
            .expect_err("missing package module declaration must fail validation");
        assert!(
            error.to_string().contains("module weather_skill::...;"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn resolve_relative_returns_absolute_paths_unchanged() {
        let absolute = std::env::current_dir().unwrap().join("tap");
        let resolved = resolve_relative(&PathBuf::from("/tmp/skill.tap.json"), absolute.clone());

        assert_eq!(resolved, absolute);
    }

    #[test]
    fn validate_tap_package_manifest_rejects_unreadable_and_invalid_inputs() {
        let tempdir = tempfile::tempdir().unwrap().keep();

        let missing = tempdir.join("missing/Move.toml");
        let error = validate_tap_package_manifest(&missing).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to read TAP package manifest"),
            "unexpected error: {error}"
        );

        let invalid = tempdir.join("invalid.toml");
        std::fs::write(&invalid, "[package\n").unwrap();
        let error = validate_tap_package_manifest(&invalid).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to parse TAP package manifest"),
            "unexpected error: {error}"
        );

        let no_package_name = tempdir.join("no-name.toml");
        std::fs::write(&no_package_name, "[package]\nedition = \"2024\"\n").unwrap();
        let error = validate_tap_package_manifest(&no_package_name).unwrap_err();
        assert!(
            error.to_string().contains("missing [package].name"),
            "unexpected error: {error}"
        );

        // `validate-skill` now enforces the new-style 2024 layout up front. A
        // manifest without [package].version fails fast so the author catches
        // the missing field locally rather than at `tap publish-skill` time.
        let no_version = tempdir.join("no-version.toml");
        std::fs::write(
            &no_version,
            "[package]\nname = \"weather_skill\"\nedition = \"2024\"\n[environments]\nlocalnet = \"6c457631\"\n",
        )
        .unwrap();
        let error = validate_tap_package_manifest(&no_version).unwrap_err();
        assert!(
            error.to_string().contains("[package].version"),
            "unexpected error: {error}"
        );

        // The old beta edition won't resolve against new-style published deps,
        // so reject it. `validate-skill` accepts only `edition = "2024"`.
        let beta_edition = tempdir.join("beta-edition.toml");
        std::fs::write(
            &beta_edition,
            "[package]\nname = \"weather_skill\"\nversion = \"1.0.0\"\nedition = \"2024.beta\"\n[environments]\nlocalnet = \"6c457631\"\n",
        )
        .unwrap();
        let error = validate_tap_package_manifest(&beta_edition).unwrap_err();
        assert!(
            error.to_string().contains("edition = \"2024.beta\""),
            "unexpected error: {error}"
        );

        // [addresses] is the old-style marker — reject manifests that carry
        // it, even if their other fields look new-style.
        let with_addresses = tempdir.join("with-addresses.toml");
        std::fs::write(
            &with_addresses,
            "[package]\nname = \"weather_skill\"\nversion = \"1.0.0\"\nedition = \"2024\"\n[addresses]\nweather_skill = \"0x0\"\n[environments]\nlocalnet = \"6c457631\"\n",
        )
        .unwrap();
        let error = validate_tap_package_manifest(&with_addresses).unwrap_err();
        assert!(
            error.to_string().contains("[addresses]"),
            "unexpected error: {error}"
        );

        // [environments] is required so Sui can resolve per-network
        // `Published.toml` for each dependency. Missing → reject.
        let no_environments = tempdir.join("no-environments.toml");
        std::fs::write(
            &no_environments,
            "[package]\nname = \"weather_skill\"\nversion = \"1.0.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        let error = validate_tap_package_manifest(&no_environments).unwrap_err();
        assert!(
            error.to_string().contains("[environments]"),
            "unexpected error: {error}"
        );

        // Happy path: full new-style manifest with one environment entry.
        let valid = tempdir.join("valid-new-style.toml");
        std::fs::write(
            &valid,
            "[package]\nname = \"weather_skill\"\nversion = \"1.0.0\"\nedition = \"2024\"\n[environments]\nlocalnet = \"6c457631\"\n",
        )
        .unwrap();
        let package_name = validate_tap_package_manifest(&valid)
            .expect("complete new-style manifest must validate");
        assert_eq!(package_name, "weather_skill");
    }

    #[test]
    fn collect_and_validate_tap_package_sources_cover_directory_edges() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let package_name = "weather_skill";

        let missing_root = tempdir.join("missing");
        let error = collect_move_source_files(&missing_root).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to read source directory"),
            "unexpected error: {error}"
        );

        let tap_package = tempdir.join("tap");
        let error = validate_tap_package_sources(&tap_package, package_name).unwrap_err();
        assert!(
            error.to_string().contains("does not exist"),
            "unexpected error: {error}"
        );

        let sources = tap_package.join("sources");
        std::fs::create_dir_all(sources.join("nested")).unwrap();
        std::fs::write(sources.join("README.md"), "not move").unwrap();
        let error = validate_tap_package_sources(&tap_package, package_name).unwrap_err();
        assert!(
            error.to_string().contains("has no Move source files"),
            "unexpected error: {error}"
        );

        std::fs::write(
            sources.join("nested").join("weather.move"),
            "module weather_skill::weather;\n",
        )
        .unwrap();
        let files = collect_move_source_files(&sources).unwrap();
        assert!(files
            .iter()
            .any(|path| path.ends_with("nested/weather.move")));
        validate_tap_package_sources(&tap_package, package_name).unwrap();
    }

    #[tokio::test]
    async fn read_artifact_and_agent_alias_helpers_cover_success_and_errors() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let artifact_path = tempdir.join("artifact.json");
        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            requirements: SkillRequirements {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceVersion::new(1),
        };
        let artifact =
            TapPublishArtifact::from_config(&config, sui::types::Address::from_static("0xd"))
                .unwrap();
        tokio::fs::write(&artifact_path, serde_json::to_string(&artifact).unwrap())
            .await
            .unwrap();

        let parsed = read_artifact(artifact_path.clone()).await.unwrap();
        assert_eq!(parsed.skill_name, "weather skill");

        tokio::fs::write(&artifact_path, "{").await.unwrap();
        let error = read_artifact(artifact_path).await.unwrap_err();
        assert!(
            error.to_string().contains("EOF while parsing"),
            "unexpected error: {error}"
        );

        let missing = read_artifact(tempdir.join("missing.json"))
            .await
            .unwrap_err();
        assert!(
            missing.to_string().contains("No such file")
                || missing.to_string().contains("not found"),
            "unexpected error: {missing}"
        );

        let mut conf = CliConf::default();
        conf.agents.insert(
            "primary".to_string(),
            sui::types::Address::from_static("0xa"),
        );
        assert_eq!(
            agent_id_from_alias_or_arg(&conf, None, Some(sui::types::Address::from_static("0xb")))
                .unwrap(),
            sui::types::Address::from_static("0xb")
        );
        assert_eq!(
            agent_id_from_alias_or_arg(&conf, Some("primary".to_string()), None).unwrap(),
            sui::types::Address::from_static("0xa")
        );
        assert!(
            agent_id_from_alias_or_arg(&conf, Some("missing".to_string()), None)
                .unwrap_err()
                .to_string()
                .contains("No Talus agent alias")
        );
        assert!(agent_id_from_alias_or_arg(&conf, None, None)
            .unwrap_err()
            .to_string()
            .contains("provide either"));
    }

    #[test]
    fn agent_execute_options_decode_payment_args() {
        let options =
            agent_execute_options_from_cli("0x0102".to_string(), 99).expect("valid options");

        assert_eq!(options.payment_source, vec![1, 2]);
        assert_eq!(options.payment_max_budget, 99);
    }

    #[test]
    fn agent_execute_options_accept_empty_payment_source() {
        let options = agent_execute_options_from_cli(String::new(), 0).expect("valid defaults");

        assert_eq!(options.payment_source, Vec::<u8>::new());
    }
}
