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
        idents::tap::TapStandard,
        nexus::{
            error::NexusError,
            tap::{
                fetch_execution_payment_history,
                fetch_tap_agent_payment_vault_for_agent,
                AnnounceEndpointRevisionResult,
                CreateAgentResult,
                GetSkillRequirementsResult,
                PublishSkillResult,
                RegisterSkillResult,
                ScheduleSkillExecutionResult,
                TapPackagePublishOptions,
            },
            workflow::StandardTapExecuteOptions,
        },
        types::{
            Agent,
            Dag as JsonDag,
            SkillId,
            TapExecutionPaymentReceipt,
            TapPublishArtifact,
            TapSkillConfig,
            DEFAULT_ENTRY_GROUP,
        },
    },
    regex::Regex,
    tokio::{
        fs::{create_dir_all, File},
        io::AsyncWriteExt,
    },
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
        #[arg(
            long,
            help = "Override TAP package path from the config.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        tap_package: Option<PathBuf>,
    },
    #[command(about = "Publish a TAP package, DAG, endpoint object, and publish artifact.")]
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
            help = "Override TAP package path from the config.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        tap_package: Option<PathBuf>,
        #[arg(
            long,
            help = "Write the publish artifact JSON to this path.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        out: Option<PathBuf>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Create a standard TAP agent.")]
    CreateAgent {
        #[arg(long, help = "Agent operator address.", value_name = "ADDRESS")]
        operator: sui::types::Address,
        #[arg(long, help = "Metadata hash bytes as hex.", value_name = "HEX")]
        metadata_hash_hex: String,
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
        #[arg(
            long,
            help = "Endpoint object ID override. Defaults to artifact metadata when present.",
            value_name = "OBJECT_ID"
        )]
        endpoint_object_id: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Announce an endpoint revision from a publish artifact.")]
    Announce {
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
        #[arg(
            long,
            help = "Endpoint object ID override. Defaults to artifact metadata when present.",
            value_name = "OBJECT_ID"
        )]
        endpoint_object_id: Option<sui::types::Address>,
        #[arg(
            long,
            default_value_t = true,
            help = "Whether this revision becomes active for new executions."
        )]
        active_for_new_executions: bool,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(subcommand, about = "Manage locally saved TAP agent aliases.")]
    Agent(AgentCommand),
    #[command(subcommand, about = "Inspect TAP agent payment vaults.")]
    Vault(VaultCommand),
    #[command(subcommand, about = "Inspect standard TAP execution payment history.")]
    Payments(PaymentsCommand),
    #[command(about = "Fetch live skill requirements from the TAP registry.")]
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
            help = "Priority fee per gas unit for the DAG execution.",
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
        #[arg(
            long = "payment-refund-mode",
            help = "Standard TAP payment refund mode byte.",
            value_name = "MODE",
            default_value_t = 0u8
        )]
        payment_refund_mode: u8,
        #[arg(
            long = "authorization-plan-hash-hex",
            help = "Optional authorization-plan hash bytes as hex.",
            value_name = "HEX"
        )]
        authorization_plan_hash_hex: Option<String>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Schedule a standard TAP skill execution.")]
    Schedule {
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "Agent-local generated skill index.", value_name = "U64")]
        skill_id: u64,
        #[arg(long, help = "Long-term gas coin ID.", value_name = "OBJECT_ID")]
        long_term_gas_coin_id: sui::types::Address,
        #[arg(long, help = "Input commitment bytes as hex.", value_name = "HEX")]
        input_commitment_hex: String,
        #[arg(
            long,
            default_value = "",
            help = "Refill policy bytes as hex.",
            value_name = "HEX"
        )]
        refill_policy_hex: String,
        #[arg(
            long = "authorization-plan-hash-hex",
            help = "Optional authorization-plan hash bytes as hex.",
            value_name = "HEX"
        )]
        authorization_plan_hash_hex: Option<String>,
        #[arg(
            long,
            default_value = "",
            help = "Schedule entries hash bytes as hex.",
            value_name = "HEX"
        )]
        schedule_entries_hash_hex: String,
        #[arg(long, default_value = "once", help = "Schedule recurrence kind.")]
        recurrence_kind: String,
        #[arg(long, default_value_t = 0, help = "Minimum interval in milliseconds.")]
        min_interval_ms: u64,
        #[arg(long, default_value_t = 1, help = "Maximum occurrences.")]
        max_occurrences: u64,
        #[arg(long, default_value_t = false, help = "Allow recursive execution.")]
        allow_recursive: bool,
        #[arg(
            long,
            default_value_t = 0,
            help = "First scheduled time offset in milliseconds."
        )]
        first_after_ms: u64,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum AgentCommand {
    #[command(about = "Save a TAP Agent under a local alias.")]
    Save {
        #[arg(long, help = "Local alias.", value_name = "NAME")]
        name: String,
        #[arg(long, help = "TAP Agent object ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
    },
    #[command(about = "List locally saved TAP agent aliases.")]
    List,
    #[command(about = "Remove a locally saved TAP agent alias.")]
    Remove {
        #[arg(long, help = "Local alias.", value_name = "NAME")]
        name: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum VaultCommand {
    #[command(about = "Show a TAP agent payment-vault balance.")]
    Balance {
        #[arg(
            long,
            help = "Local agent alias.",
            value_name = "NAME",
            conflicts_with = "agent_id"
        )]
        alias: Option<String>,
        #[arg(long, help = "TAP Agent object ID.", value_name = "OBJECT_ID")]
        agent_id: Option<sui::types::Address>,
    },
}

#[derive(Subcommand)]
pub(crate) enum PaymentsCommand {
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
            help = "TAP Agent object ID for vault history.",
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
}

pub(crate) async fn handle(command: TapCommand) -> AnyResult<(), NexusCliError> {
    match command {
        TapCommand::Scaffold { name, target } => scaffold_tap_skill(name, target).await,
        TapCommand::ValidateSkill {
            config,
            tap_package,
        } => validate_skill(config, tap_package).await.map(|_| ()),
        TapCommand::PublishSkill {
            config,
            tap_package,
            out,
            gas,
        } => {
            publish_skill(
                config,
                out,
                tap_package,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::CreateAgent {
            operator,
            metadata_hash_hex,
            gas,
        } => {
            create_agent(
                operator,
                metadata_hash_hex,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::RegisterSkill {
            artifact,
            agent_id,
            endpoint_object_id,
            gas,
        } => {
            register_skill(
                artifact,
                agent_id,
                endpoint_object_id,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::Announce {
            artifact,
            agent_id,
            skill_id,
            endpoint_object_id,
            active_for_new_executions,
            gas,
        } => {
            announce_endpoint_revision(
                artifact,
                agent_id,
                skill_id,
                endpoint_object_id,
                active_for_new_executions,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::Agent(command) => handle_agent_command(command).await,
        TapCommand::Vault(command) => handle_vault_command(command).await,
        TapCommand::Payments(command) => handle_payments_command(command).await,
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
            payment_refund_mode,
            authorization_plan_hash_hex,
            gas,
        } => {
            execute_standard_tap_skill(
                agent_id,
                skill_id,
                entry_group,
                input_json,
                remote,
                priority_fee_per_gas_unit,
                payment_source_hex,
                payment_max_budget,
                payment_refund_mode,
                authorization_plan_hash_hex,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        TapCommand::Schedule {
            agent_id,
            skill_id,
            long_term_gas_coin_id,
            input_commitment_hex,
            refill_policy_hex,
            authorization_plan_hash_hex,
            schedule_entries_hash_hex,
            recurrence_kind,
            min_interval_ms,
            max_occurrences,
            allow_recursive,
            first_after_ms,
            gas,
        } => {
            schedule_skill_execution(
                agent_id,
                skill_id,
                long_term_gas_coin_id,
                input_commitment_hex,
                refill_policy_hex,
                authorization_plan_hash_hex,
                schedule_entries_hash_hex,
                recurrence_kind,
                min_interval_ms,
                max_occurrences,
                allow_recursive,
                first_after_ms,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}

pub(crate) async fn scaffold_tap_skill(
    name: String,
    target: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scaffolding TAP skill '{name}' in '{target}'",
        target = target.display()
    );

    let root = target.join(name.to_case(Case::Kebab));
    let package_name = name.to_case(Case::Snake);
    let module_name = package_name.clone();

    let handle = loading!("Writing TAP template files...");
    let files = scaffold_files(&name, &package_name, &module_name);

    create_dir_all(&root).await.map_err(NexusCliError::Io)?;

    for (path, contents) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await.map_err(NexusCliError::Io)?;
        }

        let mut file = File::create(path).await.map_err(NexusCliError::Io)?;
        file.write_all(contents.as_bytes())
            .await
            .map_err(NexusCliError::Io)?;
    }

    handle.success();
    notify_success!("Created TAP skill scaffold at {}", root.display());
    json_output(&json!({ "path": root }))?;

    Ok(())
}

fn scaffold_files(name: &str, package_name: &str, module_name: &str) -> Vec<(PathBuf, String)> {
    vec![
        (
            PathBuf::from("tap/Move.toml"),
            format!(
                r#"[package]
name = "{package_name}"
edition = "2024.beta"

[dependencies]
nexus_interface = {{ local = "../../nexus/sui/interface" }}
nexus_workflow = {{ local = "../../nexus/sui/workflow" }}
nexus_primitives = {{ local = "../../nexus/sui/primitives" }}

[addresses]
{package_name} = "0x0"
"#
            ),
        ),
        (
            PathBuf::from(format!("tap/sources/{module_name}.move")),
            format!(
                r#"module {package_name}::{module_name};

/// Minimal third-party TAP package scaffold.
/// Fill this package with business logic, endpoint state, and standard TAP exports.
public struct {witness} has drop {{}}

public fun init_for_test(): {witness} {{
    {witness} {{}}
}}
"#,
                witness = name.to_case(Case::Pascal)
            ),
        ),
        (
            PathBuf::from("dag.json"),
            r#"{
  "vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.weather_skill@1"
      },
      "name": "entry",
      "entry_ports": [
        {
          "name": "input"
        }
      ]
    }
  ],
  "edges": []
}
"#
            .to_string(),
        ),
        (
            PathBuf::from("skill.tap.json"),
            format!(
                r#"{{
  "name": "{name}",
  "tap_package_name": "{package_name}",
  "dag_path": "dag.json",
  "tap_package_path": "tap",
  "requirements": {{
    "input_schema_hash": [1],
    "workflow_hash": [1],
    "metadata_hash": [1],
    "payment_policy": {{
      "mode": "user_funded",
      "max_budget": 0,
      "token_type_hash": [],
      "refund_mode": 0
    }},
    "schedule_policy": {{
      "recurrence_kind": "once",
      "min_interval_ms": 0,
      "max_occurrences": 1,
      "allow_recursive": false
    }},
    "vertex_authorization_schema": {{
      "schema_hash": [],
      "fixed_tools": [],
      "requires_payment": false
    }}
  }},
  "shared_objects": [],
  "interface_revision": 1,
  "active_for_new_executions": true
}}
"#
            ),
        ),
    ]
}

async fn read_skill_config(path: &PathBuf) -> AnyResult<TapSkillConfig, NexusCliError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(NexusCliError::Io)?;
    serde_json::from_str(&text).map_err(|e| NexusCliError::Any(e.into()))
}

fn resolve_relative(base_file: &PathBuf, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    base_file
        .parent()
        .map(|parent| parent.join(&path))
        .unwrap_or(path)
}

fn validate_dag_file(dag_path: &std::path::Path) -> AnyResult<()> {
    let dag_text = std::fs::read_to_string(dag_path)
        .map_err(|error| anyhow!("failed to read DAG file '{}': {error}", dag_path.display()))?;
    let dag: JsonDag = serde_json::from_str(&dag_text)
        .map_err(|error| anyhow!("failed to parse DAG JSON '{}': {error}", dag_path.display()))?;

    dag_validator::validate(dag).map_err(|error| {
        anyhow!(
            "DAG validation failed for '{}': {error}",
            dag_path.display()
        )
    })
}

fn validate_tap_package_manifest(
    manifest_path: &std::path::Path,
    config: &TapSkillConfig,
) -> AnyResult<()> {
    let manifest_text = std::fs::read_to_string(manifest_path).map_err(|error| {
        anyhow!(
            "failed to read TAP package manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    let manifest: toml::Value = toml::from_str(&manifest_text).map_err(|error| {
        anyhow!(
            "failed to parse TAP package manifest '{}': {error}",
            manifest_path.display()
        )
    })?;

    let package_name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            anyhow!(
                "TAP package manifest '{}' is missing [package].name",
                manifest_path.display()
            )
        })?;
    if package_name != config.tap_package_name {
        bail!(
            "TAP package manifest '{}' has package.name='{}' but skill config expects '{}'",
            manifest_path.display(),
            package_name,
            config.tap_package_name
        );
    }

    let addresses = manifest
        .get("addresses")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            anyhow!(
                "TAP package manifest '{}' is missing [addresses]",
                manifest_path.display()
            )
        })?;
    if !addresses.contains_key(&config.tap_package_name) {
        bail!(
            "TAP package manifest '{}' must define [addresses].{}",
            manifest_path.display(),
            config.tap_package_name
        );
    }

    Ok(())
}

fn collect_move_source_files(root: &std::path::Path) -> AnyResult<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();

    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|error| {
            anyhow!(
                "failed to read source directory '{}': {error}",
                dir.display()
            )
        })? {
            let entry = entry.map_err(|error| {
                anyhow!(
                    "failed to inspect TAP package source entry in '{}': {error}",
                    dir.display()
                )
            })?;
            let file_type = entry.file_type().map_err(|error| {
                anyhow!(
                    "failed to read TAP package source entry type '{}': {error}",
                    entry.path().display()
                )
            })?;
            let path = entry.path();

            if file_type.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("move") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn validate_tap_package_sources(
    tap_package_path: &std::path::Path,
    config: &TapSkillConfig,
) -> AnyResult<()> {
    let sources_dir = tap_package_path.join("sources");
    if !sources_dir.exists() {
        bail!(
            "TAP package sources directory '{}' does not exist",
            sources_dir.display()
        );
    }

    let source_files = collect_move_source_files(&sources_dir)?;
    if source_files.is_empty() {
        bail!(
            "TAP package '{}' has no Move source files under '{}'",
            config.tap_package_name,
            sources_dir.display()
        );
    }

    let module_pattern = Regex::new(&format!(
        r"(?m)^\s*module\s+{}::[A-Za-z_][A-Za-z0-9_]*\s*;",
        regex::escape(&config.tap_package_name)
    ))?;

    for source_file in &source_files {
        let source_text = std::fs::read_to_string(source_file).map_err(|error| {
            anyhow!(
                "failed to read TAP source file '{}': {error}",
                source_file.display()
            )
        })?;
        if module_pattern.is_match(&source_text) {
            return Ok(());
        }
    }

    bail!(
        "TAP package '{}' has no source file declaring `module {}::...;` under '{}'",
        config.tap_package_name,
        config.tap_package_name,
        sources_dir.display()
    );
}

pub(crate) async fn validate_skill(
    config_path: PathBuf,
    tap_package_override: Option<PathBuf>,
) -> AnyResult<TapSkillConfig, NexusCliError> {
    command_title!("Validating TAP skill config '{}'", config_path.display());

    let handle = loading!("Validating TAP skill config...");
    let mut config = read_skill_config(&config_path).await?;
    if let Some(path) = tap_package_override {
        config.tap_package_path = path;
    }

    config
        .validate()
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    let dag_path = resolve_relative(&config_path, config.dag_path.clone());
    if !dag_path.exists() {
        handle.error();
        return Err(NexusCliError::Any(anyhow!(
            "DAG file '{}' does not exist",
            dag_path.display()
        )));
    }
    if let Err(error) = validate_dag_file(&dag_path) {
        handle.error();
        return Err(NexusCliError::Any(error));
    }

    let tap_package = resolve_relative(&config_path, config.tap_package_path.clone());
    let move_toml = tap_package.join("Move.toml");
    if !move_toml.exists() {
        handle.error();
        return Err(NexusCliError::Any(anyhow!(
            "TAP package Move.toml '{}' does not exist",
            move_toml.display()
        )));
    }
    if let Err(error) = validate_tap_package_manifest(&move_toml, &config) {
        handle.error();
        return Err(NexusCliError::Any(error));
    }
    if let Err(error) = validate_tap_package_sources(&tap_package, &config) {
        handle.error();
        return Err(NexusCliError::Any(error));
    }

    handle.success();
    json_output(&json!({
        "valid": true,
        "skill_name": config.name,
        "tap_package_name": config.tap_package_name,
        "interface_revision": config.interface_revision,
    }))?;

    Ok(config)
}

async fn publish_skill(
    config_path: PathBuf,
    out: Option<PathBuf>,
    tap_package_override: Option<PathBuf>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config_path.clone(), tap_package_override).await?;
    let dag_path = resolve_relative(&config_path, config.dag_path.clone());
    let tap_package_path = resolve_relative(&config_path, config.tap_package_path.clone());
    let dag_text = tokio::fs::read_to_string(&dag_path)
        .await
        .map_err(NexusCliError::Io)?;
    let dag: JsonDag = serde_json::from_str(&dag_text).map_err(|e| NexusCliError::Any(e.into()))?;

    command_title!("Publishing TAP skill");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let publish = nexus_client
        .tap()
        .publish_skill(
            &config,
            dag,
            TapPackagePublishOptions {
                package_path: tap_package_path,
                named_address_overrides: vec![(
                    config.tap_package_name.clone(),
                    sui::types::Address::ZERO,
                )],
            },
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    let artifact_json = serde_json::to_string_pretty(&publish.artifact)
        .map_err(|e| NexusCliError::Any(e.into()))?;
    if let Some(out) = out {
        if let Some(parent) = out.parent() {
            create_dir_all(parent).await.map_err(NexusCliError::Io)?;
        }
        let mut file = File::create(&out).await.map_err(NexusCliError::Io)?;
        file.write_all(artifact_json.as_bytes())
            .await
            .map_err(NexusCliError::Io)?;
        notify_success!("Wrote TAP publish artifact to {}", out.display());
    }

    json_output(&publish_skill_result_json(&publish))
}

async fn read_artifact(path: PathBuf) -> AnyResult<TapPublishArtifact, NexusCliError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(NexusCliError::Io)?;
    serde_json::from_str(&text).map_err(|e| NexusCliError::Any(e.into()))
}

fn decode_hex_arg(value: &str, name: &str) -> AnyResult<Vec<u8>, NexusCliError> {
    hex::decode(value.trim_start_matches("0x"))
        .map_err(|e| NexusCliError::Any(anyhow!("invalid {name} hex: {e}")))
}

fn decode_optional_hex_arg(
    value: Option<String>,
    name: &str,
) -> AnyResult<Option<Vec<u8>>, NexusCliError> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| decode_hex_arg(&value, name))
        .transpose()
}

fn standard_execute_options_from_cli(
    payment_source_hex: String,
    payment_max_budget: u64,
    payment_refund_mode: u8,
    authorization_plan_hash_hex: Option<String>,
) -> AnyResult<StandardTapExecuteOptions, NexusCliError> {
    Ok(StandardTapExecuteOptions {
        payment_source: decode_hex_arg(&payment_source_hex, "payment-source")?,
        payment_coin: None,
        payment_coin_balance: None,
        payment_max_budget,
        payment_refund_mode,
        authorization_plan_hash: decode_optional_hex_arg(
            authorization_plan_hash_hex,
            "authorization-plan-hash",
        )?,
        authorization_plan: Vec::new(),
    })
}

fn agent_id_from_alias_or_arg(
    conf: &CliConf,
    alias: Option<String>,
    agent_id: Option<sui::types::Address>,
) -> AnyResult<Agent, NexusCliError> {
    if let Some(agent_id) = agent_id {
        return Ok(Agent(agent_id));
    }
    if let Some(alias) = alias {
        let agent_id = conf.tap_agents.get(&alias).copied().ok_or_else(|| {
            NexusCliError::Any(anyhow!("No TAP agent alias '{alias}' found in CLI config"))
        })?;
        return Ok(Agent(agent_id));
    }
    Err(NexusCliError::Any(anyhow!(
        "provide either --agent-id or --alias"
    )))
}

async fn handle_agent_command(command: AgentCommand) -> AnyResult<(), NexusCliError> {
    match command {
        AgentCommand::Save { name, agent_id } => {
            let mut conf = CliConf::load().await.unwrap_or_default();
            conf.tap_agents.insert(name.clone(), agent_id);
            conf.save().await.map_err(NexusCliError::Any)?;
            notify_success!("Saved TAP agent alias {name}");
            json_output(&json!({ "name": name, "agent_id": agent_id }))
        }
        AgentCommand::List => {
            let conf = CliConf::load().await.unwrap_or_default();
            let mut agents = conf.tap_agents.into_iter().collect::<Vec<_>>();
            agents.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
            json_output(&json!({
                "agents": agents.into_iter().map(|(name, agent_id)| {
                    json!({ "name": name, "agent_id": agent_id })
                }).collect::<Vec<_>>()
            }))
        }
        AgentCommand::Remove { name } => {
            let mut conf = CliConf::load().await.unwrap_or_default();
            let removed = conf.tap_agents.remove(&name);
            conf.save().await.map_err(NexusCliError::Any)?;
            json_output(&json!({ "name": name, "removed": removed }))
        }
    }
}

async fn handle_vault_command(command: VaultCommand) -> AnyResult<(), NexusCliError> {
    match command {
        VaultCommand::Balance { alias, agent_id } => {
            let conf = CliConf::load().await.unwrap_or_default();
            let agent_id = agent_id_from_alias_or_arg(&conf, alias, agent_id)?;
            let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
            let vault = fetch_tap_agent_payment_vault_for_agent(nexus_client.crawler(), agent_id)
                .await
                .map_err(|e| NexusCliError::Any(e.into()))?;
            json_output(&json!({
                "agent_id": agent_id,
                "vault_id": vault.object_id,
                "available_balance": vault.data.available_balance,
                "locked_amount": vault.data.locked_amount,
                "unlocked_balance": vault.data.available_balance.saturating_sub(vault.data.locked_amount)
            }))
        }
    }
}

async fn handle_payments_command(command: PaymentsCommand) -> AnyResult<(), NexusCliError> {
    match command {
        PaymentsCommand::List {
            alias,
            agent_id,
            completed,
            pending,
            all: _,
        } => {
            let conf = CliConf::load().await.unwrap_or_default();
            let agent_id = if alias.is_some() || agent_id.is_some() {
                Some(agent_id_from_alias_or_arg(&conf, alias, agent_id)?)
            } else {
                None
            };
            let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
            let owner = nexus_client.signer().get_active_address();
            let history = fetch_execution_payment_history(
                nexus_client.crawler(),
                &nexus_client.get_nexus_objects(),
                owner,
                agent_id,
            )
            .await
            .map_err(|e| NexusCliError::Any(e.into()))?;
            let include = |receipt: &&TapExecutionPaymentReceipt| {
                (!completed && !pending)
                    || (completed && receipt.resolved)
                    || (pending && !receipt.resolved)
            };
            let wallet_receipts = history
                .wallet_receipts
                .iter()
                .filter(include)
                .cloned()
                .collect::<Vec<_>>();
            let vault_receipts = history
                .vault_receipts
                .iter()
                .filter(include)
                .cloned()
                .collect::<Vec<_>>();
            json_output(&json!({
                "owner": owner,
                "agent_id": agent_id,
                "wallet_receipts": wallet_receipts,
                "vault_receipts": vault_receipts,
                "unresolved_execution_ids": history.unresolved_execution_ids,
                "resolved_execution_ids": history.resolved_execution_ids
            }))
        }
    }
}

async fn create_agent(
    operator: sui::types::Address,
    metadata_hash_hex: String,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let metadata_hash = decode_hex_arg(&metadata_hash_hex, "metadata-hash")?;
    command_title!("Creating TAP agent");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .create_agent(operator, metadata_hash)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Created TAP agent {agent_id}",
        agent_id = result.agent_id.to_string().truecolor(100, 100, 100)
    );
    json_output(&create_agent_result_json(operator, &result))
}

async fn register_skill(
    artifact: PathBuf,
    agent_id: sui::types::Address,
    endpoint_object_id: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    let resolved_endpoint_object_id = artifact
        .endpoint_object_id_or(endpoint_object_id)
        .map_err(NexusCliError::Any)?;
    command_title!("Registering TAP skill for agent '{}'", agent_id);

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .register_skill(Agent(agent_id), &artifact, endpoint_object_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Registered TAP skill {skill_id}",
        skill_id = result.skill_id.to_string().truecolor(100, 100, 100)
    );
    json_output(&register_skill_result_json(
        &artifact,
        resolved_endpoint_object_id,
        &result,
    ))
}

async fn announce_endpoint_revision(
    artifact: PathBuf,
    agent_id: sui::types::Address,
    skill_id: u64,
    endpoint_object_id: Option<sui::types::Address>,
    active_for_new_executions: bool,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    command_title!("Announcing TAP endpoint revision for '{agent_id}:{skill_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .announce_endpoint_revision(
            Agent(agent_id),
            skill_id,
            &artifact,
            endpoint_object_id,
            active_for_new_executions,
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&announce_result_json(&artifact, &result).map_err(NexusCliError::Any)?)
}

async fn fetch_requirements(
    agent_id: sui::types::Address,
    skill_id: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Fetching TAP skill requirements for '{agent_id}:{skill_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let result = nexus_client
        .tap()
        .get_skill_requirements(Agent(agent_id), skill_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&requirements_result_json(&result))
}

async fn dry_run_skill(config: PathBuf) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config, None).await?;
    let package_id = sui::types::Address::ZERO;
    let digest = config
        .digest_input(package_id)
        .digest_hex()
        .map_err(NexusCliError::Any)?;
    json_output(&json!({
        "dry_run": true,
        "valid": true,
        "skill_name": config.name,
        "interface_revision": config.interface_revision,
        "config_digest_hex_with_zero_package": digest,
        "next_step": "publish TAP plus DAG, then create-agent and register-skill"
    }))
}

#[allow(clippy::too_many_arguments)]
async fn execute_standard_tap_skill(
    agent_id: sui::types::Address,
    skill_id: u64,
    entry_group: String,
    input_json: serde_json::Value,
    remote: Vec<String>,
    priority_fee_per_gas_unit: u64,
    payment_source_hex: String,
    payment_max_budget: u64,
    payment_refund_mode: u8,
    authorization_plan_hash_hex: Option<String>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing standard TAP skill '{agent_id}:{skill_id}'");

    let options = standard_execute_options_from_cli(
        payment_source_hex,
        payment_max_budget,
        payment_refund_mode,
        authorization_plan_hash_hex,
    )?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf = conf.data_storage.clone().into();
    let input_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;

    let tx_handle = loading!("Crafting and executing standard TAP transaction...");
    let result = match nexus_client
        .workflow()
        .execute_standard_tap(
            Agent(agent_id),
            skill_id,
            input_data,
            priority_fee_per_gas_unit,
            Some(&entry_group),
            &storage_conf,
            options,
        )
        .await
    {
        Ok(result) => result,
        Err(NexusError::Storage(e)) => {
            tx_handle.error();
            return Err(NexusCliError::Any(anyhow!(
                "{e}.\nEnsure remote storage is configured."
            )));
        }
        Err(error) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(error));
        }
    };

    tx_handle.success();

    notify_success!(
        "DAGExecution object ID: {id}",
        id = result
            .execution_object_id
            .to_string()
            .truecolor(100, 100, 100)
    );

    json_output(&standard_execute_result_json(
        Agent(agent_id),
        skill_id,
        &result,
    ))
}

fn standard_execute_result_json(
    agent_id: Agent,
    skill_id: SkillId,
    result: &nexus_sdk::nexus::workflow::ExecuteResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "agent_id": agent_id,
        "skill_id": skill_id,
        "execution_id": result.execution_object_id,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "submit": result.standard_tap.as_ref().map(|submit| json!({
            "agent_id": submit.agent_id,
            "skill_id": submit.skill_id,
            "dag_id": submit.dag_id,
            "endpoint_key": submit.endpoint_key,
            "endpoint_object_id": submit.endpoint_object.object_id(),
            "endpoint_object_version": submit.endpoint_object.version(),
            "payment_max_budget": submit.payment_max_budget,
            "payment_refund_mode": submit.payment_refund_mode,
            "authorization_plan_hash": submit.authorization_plan_hash,
        }))
    })
}

#[allow(clippy::too_many_arguments)]
async fn schedule_skill_execution(
    agent_id: sui::types::Address,
    skill_id: u64,
    long_term_gas_coin_id: sui::types::Address,
    input_commitment_hex: String,
    refill_policy_hex: String,
    authorization_plan_hash_hex: Option<String>,
    schedule_entries_hash_hex: String,
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
    first_after_ms: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let input_commitment = decode_hex_arg(&input_commitment_hex, "input-commitment")?;
    let refill_policy = decode_hex_arg(&refill_policy_hex, "refill-policy")?;
    let authorization_plan_hash =
        decode_optional_hex_arg(authorization_plan_hash_hex, "authorization-plan-hash")?;
    let schedule_entries_hash =
        decode_hex_arg(&schedule_entries_hash_hex, "schedule-entries-hash")?;
    let schedule_policy = nexus_sdk::types::TapSchedulePolicy {
        recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    };

    command_title!("Scheduling TAP skill execution for '{agent_id}:{skill_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .schedule_skill_execution(
            Agent(agent_id),
            skill_id,
            long_term_gas_coin_id,
            input_commitment,
            refill_policy,
            authorization_plan_hash,
            schedule_policy,
            schedule_entries_hash,
            first_after_ms,
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&schedule_result_json(long_term_gas_coin_id, &result))
}

fn create_agent_result_json(
    operator: sui::types::Address,
    result: &CreateAgentResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::CREATE_AGENT.name.to_string(),
        "agent_id": result.agent_id,
        "operator": operator,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
    })
}

fn publish_skill_result_json(result: &PublishSkillResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": "publish_skill",
        "tap_package_id": result.tap_package.package_id,
        "tap_package_digest": result.tap_package.tx_digest,
        "tap_package_checkpoint": result.tap_package.tx_checkpoint,
        "dag_id": result.dag.dag_object_id,
        "dag_digest": result.dag.tx_digest,
        "dag_checkpoint": result.dag.tx_checkpoint,
        "endpoint_object_id": result.endpoint.endpoint_object.object_id(),
        "endpoint_object_version": result.endpoint.endpoint_object.version(),
        "endpoint_object_digest": result.endpoint.endpoint_object.digest(),
        "endpoint_digest": result.endpoint.tx_digest,
        "endpoint_checkpoint": result.endpoint.tx_checkpoint,
        "artifact": result.artifact,
    })
}

fn register_skill_result_json(
    artifact: &TapPublishArtifact,
    endpoint_object_id: sui::types::Address,
    result: &RegisterSkillResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::REGISTER_SKILL.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "dag_id": artifact.dag_id,
        "tap_package_id": artifact.tap_package_id,
        "endpoint_object_id": endpoint_object_id,
    })
}

fn announce_result_json(
    artifact: &TapPublishArtifact,
    result: &AnnounceEndpointRevisionResult,
) -> anyhow::Result<serde_json::Value> {
    Ok(json!({
        "standard_tap": true,
        "function": TapStandard::ANNOUNCE_ENDPOINT_REVISION.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "endpoint_key": result.endpoint_key,
        "endpoint_object_id": result.endpoint_object.object_id(),
        "endpoint_object_version": result.endpoint_object.version(),
        "tap_package_id": artifact.tap_package_id,
        "config_digest_hex": hex::encode(&result.config_digest),
        "config_digest_input": result.config_digest_input,
    }))
}

fn requirements_result_json(result: &GetSkillRequirementsResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::GET_SKILL_REQUIREMENTS.name.to_string(),
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "active_endpoint_key": result.active_endpoint_key,
        "requirements": result.requirements,
    })
}

fn schedule_result_json(
    long_term_gas_coin_id: sui::types::Address,
    result: &ScheduleSkillExecutionResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::SCHEDULE_SKILL_EXECUTION.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "long_term_gas_coin_id": long_term_gas_coin_id,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        assert_matches::assert_matches,
        nexus_sdk::{
            nexus::workflow::{ExecuteResult, StandardTapSubmitMetadata},
            types::{
                InterfaceRevision,
                TapPaymentPolicy,
                TapSchedulePolicy,
                TapSkillRequirements,
                TapVertexAuthorizationSchema,
            },
        },
    };

    async fn publish_skill_artifact(
        config_path: PathBuf,
        dag_id: sui::types::Address,
        tap_package_id: sui::types::Address,
        endpoint_object_id: Option<sui::types::Address>,
        endpoint_object_version: Option<u64>,
        endpoint_object_digest_hex: Option<String>,
        out: Option<PathBuf>,
    ) -> AnyResult<(), NexusCliError> {
        let config = validate_skill(config_path, None).await?;
        command_title!("Creating TAP publish artifact");

        let mut artifact = TapPublishArtifact::from_config(&config, dag_id, tap_package_id)
            .map_err(NexusCliError::Any)?;
        let has_endpoint_metadata = endpoint_object_id.is_some()
            || endpoint_object_version.is_some()
            || endpoint_object_digest_hex.is_some();
        if let (Some(id), Some(version), Some(digest_hex)) = (
            endpoint_object_id,
            endpoint_object_version,
            endpoint_object_digest_hex,
        ) {
            let digest_bytes = decode_hex_arg(&digest_hex, "endpoint-object-digest")?;
            let digest = sui::types::Digest::from_bytes(digest_bytes.as_slice())
                .map_err(|e| NexusCliError::Any(e.into()))?;
            artifact = artifact
                .with_endpoint_object(sui::types::ObjectReference::new(id, version, digest))
                .map_err(NexusCliError::Any)?;
        } else if has_endpoint_metadata {
            return Err(NexusCliError::Any(anyhow!(
            "endpoint-object-id, endpoint-object-version, and endpoint-object-digest-hex must be provided together"
        )));
        }
        let artifact_json =
            serde_json::to_string_pretty(&artifact).map_err(|e| NexusCliError::Any(e.into()))?;

        if let Some(out) = out {
            if let Some(parent) = out.parent() {
                create_dir_all(parent).await.map_err(NexusCliError::Io)?;
            }
            let mut file = File::create(&out).await.map_err(NexusCliError::Io)?;
            file.write_all(artifact_json.as_bytes())
                .await
                .map_err(NexusCliError::Io)?;
            notify_success!("Wrote TAP publish artifact to {}", out.display());
        }

        json_output(&artifact)?;
        Ok(())
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
        let config = validate_skill(config_path.clone(), None)
            .await
            .expect("generated config validates");
        assert_eq!(config.name, "weather skill");
        assert_eq!(config.interface_revision, InterfaceRevision(1));
    }

    #[tokio::test]
    async fn publish_artifact_flow_can_embed_endpoint_metadata() {
        let tempdir = tempfile::tempdir().unwrap().keep();

        scaffold_tap_skill("weather skill".to_string(), tempdir.clone())
            .await
            .expect("scaffold succeeds");

        let root = tempdir.join("weather-skill");
        let config_path = root.join("skill.tap.json");
        let artifact_path = root.join("artifact-with-endpoint.json");
        publish_skill_artifact(
            config_path,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
            Some(sui::types::Address::from_static("0xf")),
            Some(7),
            Some(hex::encode([8_u8; 32])),
            Some(artifact_path.clone()),
        )
        .await
        .expect("artifact generation succeeds");

        let artifact_text = tokio::fs::read_to_string(artifact_path).await.unwrap();
        let artifact: TapPublishArtifact = serde_json::from_str(&artifact_text).unwrap();
        assert_eq!(
            artifact.endpoint_object_id,
            Some(sui::types::Address::from_static("0xf"))
        );
        assert_eq!(artifact.endpoint_object_version, Some(7));
        assert_eq!(artifact.endpoint_object_digest, Some(vec![8; 32]));
        assert!(artifact.endpoint_config_digest.is_some());
        assert_eq!(
            artifact.endpoint_config_digest_hex.as_ref().unwrap().len(),
            64
        );
    }

    #[tokio::test]
    async fn publish_artifact_rejects_partial_endpoint_metadata() {
        let tempdir = tempfile::tempdir().unwrap().keep();

        scaffold_tap_skill("weather skill".to_string(), tempdir.clone())
            .await
            .expect("scaffold succeeds");

        let error = publish_skill_artifact(
            tempdir.join("weather-skill/skill.tap.json"),
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
            Some(sui::types::Address::from_static("0xf")),
            None,
            None,
            None,
        )
        .await
        .expect_err("partial endpoint metadata should fail");

        assert!(error
            .to_string()
            .contains("endpoint-object-id, endpoint-object-version"));
    }

    #[test]
    fn announce_result_digest_fields_are_endpoint_bound() {
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: TapSkillRequirements {
                input_schema_hash: vec![1],
                workflow_hash: vec![2],
                metadata_hash: vec![3],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
            },
            shared_objects: Vec::new(),
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };
        let artifact = TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
        )
        .expect("valid artifact");
        let endpoint_object_id = sui::types::Address::from_static("0xf");
        let digest_input = artifact.endpoint_config_digest_input(endpoint_object_id);
        let digest = digest_input.digest().expect("endpoint digest");
        let output = announce_result_json(
            &artifact,
            &AnnounceEndpointRevisionResult {
                tx_digest: sui::types::Digest::from([7; 32]),
                tx_checkpoint: 42,
                endpoint_key: nexus_sdk::types::TapEndpointKey {
                    agent_id: Agent(sui::types::Address::from_static("0xa")),
                    skill_id: 11,
                    interface_revision: InterfaceRevision(1),
                },
                endpoint_object: sui::types::ObjectReference::new(
                    endpoint_object_id,
                    7,
                    sui::types::Digest::from([8; 32]),
                ),
                config_digest: digest,
                config_digest_input: digest_input,
            },
        )
        .expect("announce result json");

        assert_eq!(
            output["endpoint_object_id"],
            serde_json::Value::String(endpoint_object_id.to_string())
        );
        assert_eq!(
            output["config_digest_input"]["endpoint_object_id"],
            serde_json::Value::String(endpoint_object_id.to_string())
        );
        assert_eq!(output["config_digest_hex"].as_str().unwrap().len(), 64);
        assert_ne!(output["config_digest_hex"], artifact.config_digest_hex);
    }

    #[tokio::test]
    async fn validate_skill_rejects_missing_tap_package() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let config = TapSkillConfig {
            name: "bad".to_string(),
            tap_package_name: "bad_tap".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("missing-tap"),
            requirements: TapSkillRequirements {
                input_schema_hash: vec![1],
                workflow_hash: vec![1],
                metadata_hash: vec![1],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
            },
            shared_objects: Vec::new(),
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };
        let config_path = tempdir.join("skill.tap.json");
        tokio::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .await
            .unwrap();
        tokio::fs::write(tempdir.join("dag.json"), "{}")
            .await
            .unwrap();

        assert_matches!(validate_skill(config_path, None).await, Err(_));
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

        let error = validate_skill(root.join("skill.tap.json"), None)
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
edition = "2024.beta"

[dependencies]
nexus_interface = { local = "../../nexus/sui/interface" }
nexus_workflow = { local = "../../nexus/sui/workflow" }
nexus_primitives = { local = "../../nexus/sui/primitives" }

[addresses]
weather_skill = "0x0"
"#,
        )
        .await
        .unwrap();

        let error = validate_skill(root.join("skill.tap.json"), None)
            .await
            .expect_err("package name mismatch must fail validation");
        assert!(
            error.to_string().contains("package.name='other_tap'"),
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
        tokio::fs::write(
            root.join("tap/Move.toml"),
            r#"[package]
name = "weather_skill"
edition = "2024.beta"

[dependencies]
nexus_interface = { local = "../../nexus/sui/interface" }
nexus_workflow = { local = "../../nexus/sui/workflow" }
nexus_primitives = { local = "../../nexus/sui/primitives" }

[addresses]
other_alias = "0x0"
"#,
        )
        .await
        .unwrap();

        let error = validate_skill(root.join("skill.tap.json"), None)
            .await
            .expect_err("missing address alias must fail validation");
        assert!(
            error
                .to_string()
                .contains("must define [addresses].weather_skill"),
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

        let error = validate_skill(root.join("skill.tap.json"), None)
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
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: TapSkillRequirements::default(),
            shared_objects: Vec::new(),
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };

        let missing = tempdir.join("missing/Move.toml");
        let error = validate_tap_package_manifest(&missing, &config).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to read TAP package manifest"),
            "unexpected error: {error}"
        );

        let invalid = tempdir.join("invalid.toml");
        std::fs::write(&invalid, "[package\n").unwrap();
        let error = validate_tap_package_manifest(&invalid, &config).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to parse TAP package manifest"),
            "unexpected error: {error}"
        );

        let no_package_name = tempdir.join("no-name.toml");
        std::fs::write(
            &no_package_name,
            "[package]\n[addresses]\nweather_skill = \"0x0\"\n",
        )
        .unwrap();
        let error = validate_tap_package_manifest(&no_package_name, &config).unwrap_err();
        assert!(
            error.to_string().contains("missing [package].name"),
            "unexpected error: {error}"
        );

        let no_addresses = tempdir.join("no-addresses.toml");
        std::fs::write(&no_addresses, "[package]\nname = \"weather_skill\"\n").unwrap();
        let error = validate_tap_package_manifest(&no_addresses, &config).unwrap_err();
        assert!(
            error.to_string().contains("missing [addresses]"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn collect_and_validate_tap_package_sources_cover_directory_edges() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: TapSkillRequirements::default(),
            shared_objects: Vec::new(),
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };

        let missing_root = tempdir.join("missing");
        let error = collect_move_source_files(&missing_root).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to read source directory"),
            "unexpected error: {error}"
        );

        let tap_package = tempdir.join("tap");
        let error = validate_tap_package_sources(&tap_package, &config).unwrap_err();
        assert!(
            error.to_string().contains("does not exist"),
            "unexpected error: {error}"
        );

        let sources = tap_package.join("sources");
        std::fs::create_dir_all(sources.join("nested")).unwrap();
        std::fs::write(sources.join("README.md"), "not move").unwrap();
        let error = validate_tap_package_sources(&tap_package, &config).unwrap_err();
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
        validate_tap_package_sources(&tap_package, &config).unwrap();
    }

    #[tokio::test]
    async fn read_artifact_and_agent_alias_helpers_cover_success_and_errors() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let artifact_path = tempdir.join("artifact.json");
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: TapSkillRequirements {
                input_schema_hash: vec![1],
                workflow_hash: vec![2],
                metadata_hash: vec![3],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
            },
            shared_objects: Vec::new(),
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };
        let artifact = TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
        )
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
        conf.tap_agents.insert(
            "primary".to_string(),
            sui::types::Address::from_static("0xa"),
        );
        assert_eq!(
            agent_id_from_alias_or_arg(&conf, None, Some(sui::types::Address::from_static("0xb")))
                .unwrap(),
            Agent(sui::types::Address::from_static("0xb"))
        );
        assert_eq!(
            agent_id_from_alias_or_arg(&conf, Some("primary".to_string()), None).unwrap(),
            Agent(sui::types::Address::from_static("0xa"))
        );
        assert!(
            agent_id_from_alias_or_arg(&conf, Some("missing".to_string()), None)
                .unwrap_err()
                .to_string()
                .contains("No TAP agent alias")
        );
        assert!(agent_id_from_alias_or_arg(&conf, None, None)
            .unwrap_err()
            .to_string()
            .contains("provide either"));
    }

    #[test]
    fn standard_execute_options_decode_payment_and_authorization_args() {
        let options = standard_execute_options_from_cli(
            "0x0102".to_string(),
            99,
            7,
            Some("0x0908".to_string()),
        )
        .expect("valid options");

        assert_eq!(options.payment_source, vec![1, 2]);
        assert_eq!(options.payment_max_budget, 99);
        assert_eq!(options.payment_refund_mode, 7);
        assert_eq!(options.authorization_plan_hash, Some(vec![9, 8]));
    }

    #[test]
    fn standard_execute_options_accept_empty_optional_authorization_hash() {
        let options = standard_execute_options_from_cli(String::new(), 0, 0, Some(String::new()))
            .expect("valid defaults");

        assert_eq!(options.payment_source, Vec::<u8>::new());
        assert_eq!(options.authorization_plan_hash, None);
    }

    #[test]
    fn standard_execute_result_json_includes_submit_metadata() {
        let result = ExecuteResult {
            tx_digest: sui::types::Digest::from([7; 32]),
            execution_object_id: sui::types::Address::from_static("0xc"),
            tx_checkpoint: 42,
            standard_tap: Some(StandardTapSubmitMetadata {
                agent_id: Agent(sui::types::Address::from_static("0xa")),
                skill_id: 11,
                dag_id: sui::types::Address::from_static("0xd"),
                endpoint_key: nexus_sdk::types::TapEndpointKey {
                    agent_id: Agent(sui::types::Address::from_static("0xa")),
                    skill_id: 11,
                    interface_revision: InterfaceRevision(3),
                },
                endpoint_object: sui::types::ObjectReference::new(
                    sui::types::Address::from_static("0xe"),
                    9,
                    sui::types::Digest::from([8; 32]),
                ),
                payment_max_budget: 99,
                payment_refund_mode: 7,
                authorization_plan_hash: Some(vec![1, 2, 3]),
                authorization_plan: nexus_sdk::types::TapVertexAuthorizationPlan::default(),
            }),
        };

        let output = standard_execute_result_json(
            Agent(sui::types::Address::from_static("0xa")),
            11,
            &result,
        );

        assert_eq!(
            output["execution_id"],
            serde_json::json!(sui::types::Address::from_static("0xc").to_string())
        );
        assert_eq!(
            output["submit"]["dag_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert_eq!(
            output["submit"]["endpoint_key"]["interface_revision"],
            serde_json::json!(3)
        );
        assert_eq!(
            output["submit"]["payment_max_budget"],
            serde_json::json!(99)
        );
        assert_eq!(
            output["submit"]["authorization_plan_hash"],
            serde_json::json!([1, 2, 3])
        );
    }

    #[test]
    fn tap_submission_result_json_helpers_expose_created_ids() {
        let artifact = TapPublishArtifact::from_config(
            &TapSkillConfig {
                name: "weather skill".to_string(),
                tap_package_name: "weather_tap".to_string(),
                dag_path: PathBuf::from("dag.json"),
                tap_package_path: PathBuf::from("tap"),
                requirements: TapSkillRequirements {
                    input_schema_hash: vec![1],
                    workflow_hash: vec![2],
                    metadata_hash: vec![3],
                    payment_policy: TapPaymentPolicy::default(),
                    schedule_policy: TapSchedulePolicy::default(),
                    vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
                },
                shared_objects: Vec::new(),
                interface_revision: InterfaceRevision(1),
                active_for_new_executions: true,
            },
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
        )
        .expect("valid artifact");

        let create_output = create_agent_result_json(
            sui::types::Address::from_static("0x2"),
            &CreateAgentResult {
                tx_digest: sui::types::Digest::from([7; 32]),
                tx_checkpoint: 11,
                agent_id: Agent(sui::types::Address::from_static("0xa")),
                agent_object: sui::types::ObjectReference::new(
                    sui::types::Address::from_static("0xa"),
                    7,
                    sui::types::Digest::from([8; 32]),
                ),
            },
        );
        assert_eq!(
            create_output["agent_id"],
            serde_json::json!(sui::types::Address::from_static("0xa").to_string())
        );
        assert_eq!(create_output["tx_checkpoint"], serde_json::json!(11));

        let register_output = register_skill_result_json(
            &artifact,
            sui::types::Address::from_static("0xf"),
            &RegisterSkillResult {
                tx_digest: sui::types::Digest::from([8; 32]),
                tx_checkpoint: 12,
                agent_id: Agent(sui::types::Address::from_static("0xa")),
                skill_id: 11,
            },
        );
        assert_eq!(register_output["skill_id"], serde_json::json!(11));
        assert_eq!(
            register_output["endpoint_object_id"],
            serde_json::json!(sui::types::Address::from_static("0xf").to_string())
        );
    }

    #[test]
    fn publish_skill_result_json_exposes_complete_artifact_handoff() {
        let artifact = TapPublishArtifact::from_config(
            &TapSkillConfig {
                name: "weather skill".to_string(),
                tap_package_name: "weather_tap".to_string(),
                dag_path: PathBuf::from("dag.json"),
                tap_package_path: PathBuf::from("tap"),
                requirements: TapSkillRequirements {
                    input_schema_hash: vec![1],
                    workflow_hash: vec![2],
                    metadata_hash: vec![3],
                    payment_policy: TapPaymentPolicy::default(),
                    schedule_policy: TapSchedulePolicy::default(),
                    vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
                },
                shared_objects: Vec::new(),
                interface_revision: InterfaceRevision(1),
                active_for_new_executions: true,
            },
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
        )
        .expect("valid artifact")
        .with_endpoint_object(sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xf"),
            7,
            sui::types::Digest::from([8; 32]),
        ))
        .expect("endpoint artifact");

        let output = publish_skill_result_json(&PublishSkillResult {
            tap_package: nexus_sdk::nexus::tap::TapPackagePublishResult {
                tx_digest: sui::types::Digest::from([1; 32]),
                tx_checkpoint: 10,
                package_id: sui::types::Address::from_static("0xe"),
            },
            dag: nexus_sdk::nexus::workflow::PublishResult {
                tx_digest: sui::types::Digest::from([2; 32]),
                tx_checkpoint: 11,
                dag_object_id: sui::types::Address::from_static("0xd"),
            },
            endpoint: nexus_sdk::nexus::tap::CreateStandardEndpointResult {
                tx_digest: sui::types::Digest::from([3; 32]),
                tx_checkpoint: 12,
                endpoint_object: sui::types::ObjectReference::new(
                    sui::types::Address::from_static("0xf"),
                    7,
                    sui::types::Digest::from([8; 32]),
                ),
            },
            artifact,
        });

        assert_eq!(
            output["tap_package_id"],
            serde_json::json!(sui::types::Address::from_static("0xe").to_string())
        );
        assert_eq!(
            output["dag_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert_eq!(
            output["endpoint_object_id"],
            serde_json::json!(sui::types::Address::from_static("0xf").to_string())
        );
        assert_eq!(
            output["artifact"]["endpoint_object_id"],
            serde_json::json!(sui::types::Address::from_static("0xf").to_string())
        );
        assert_eq!(
            output["artifact"]["endpoint_config_digest_hex"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
    }

    #[test]
    fn tap_requirements_and_schedule_json_helpers_expose_live_state() {
        let requirements = TapSkillRequirements {
            input_schema_hash: vec![1],
            workflow_hash: vec![2],
            metadata_hash: vec![3],
            payment_policy: TapPaymentPolicy::default(),
            schedule_policy: TapSchedulePolicy::default(),
            vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
        };

        let requirements_output = requirements_result_json(&GetSkillRequirementsResult {
            agent_id: Agent(sui::types::Address::from_static("0xa")),
            skill_id: 11,
            active_endpoint_key: nexus_sdk::types::TapEndpointKey {
                agent_id: Agent(sui::types::Address::from_static("0xa")),
                skill_id: 11,
                interface_revision: InterfaceRevision(3),
            },
            requirements,
        });
        assert_eq!(
            requirements_output["active_endpoint_key"]["interface_revision"],
            serde_json::json!(3)
        );
        assert_eq!(
            requirements_output["requirements"]["workflow_hash"],
            serde_json::json!([2])
        );

        let schedule_output = schedule_result_json(
            sui::types::Address::from_static("0xc"),
            &ScheduleSkillExecutionResult {
                tx_digest: sui::types::Digest::from([9; 32]),
                tx_checkpoint: 13,
                scheduled_task_id: sui::types::Address::from_static("0xd"),
                agent_id: Agent(sui::types::Address::from_static("0xa")),
                skill_id: 11,
            },
        );
        assert_eq!(
            schedule_output["scheduled_task_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert_eq!(schedule_output["tx_checkpoint"], serde_json::json!(13));
    }
}
