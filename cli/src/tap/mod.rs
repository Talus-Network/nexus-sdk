use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*},
    convert_case::{Case, Casing},
    nexus_sdk::{
        idents::tap::TapStandard,
        types::{AgentId, SkillId, TapPublishArtifact, TapSkillConfig},
    },
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
    #[command(
        about = "Create the post-publish TAP skill artifact from a validated config and published IDs."
    )]
    PublishSkill {
        #[arg(
            long,
            short,
            help = "Path to the TAP skill config JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        config: PathBuf,
        #[arg(long, help = "Published DAG object ID.", value_name = "OBJECT_ID")]
        dag_id: sui::types::Address,
        #[arg(long, help = "Published TAP package ID.", value_name = "OBJECT_ID")]
        tap_package_id: sui::types::Address,
        #[arg(
            long,
            help = "Write the publish artifact JSON to this path.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        out: Option<PathBuf>,
    },
    #[command(about = "Print a create-agent PTB plan.")]
    CreateAgent {
        #[arg(long, help = "Agent operator address.", value_name = "ADDRESS")]
        operator: sui::types::Address,
        #[arg(long, help = "Metadata hash bytes as hex.", value_name = "HEX")]
        metadata_hash_hex: String,
        #[arg(long, default_value_t = 0, help = "Payment/auth mode byte.")]
        auth_mode: u8,
    },
    #[command(about = "Print a register-skill PTB plan from a publish artifact.")]
    RegisterSkill {
        #[arg(
            long,
            help = "Path to the publish artifact JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        artifact: PathBuf,
    },
    #[command(about = "Print an endpoint announcement PTB plan from a publish artifact.")]
    Announce {
        #[arg(
            long,
            help = "Path to the publish artifact JSON.",
            value_parser = ValueParser::from(expand_tilde)
        )]
        artifact: PathBuf,
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "On-chain generated skill ID.", value_name = "OBJECT_ID")]
        skill_id: sui::types::Address,
        #[arg(long, help = "Endpoint object ID.", value_name = "OBJECT_ID")]
        endpoint_object_id: sui::types::Address,
        #[arg(long, default_value_t = 1, help = "Endpoint object version.")]
        endpoint_object_version: u64,
    },
    #[command(about = "Print a skill requirements fetch plan.")]
    Requirements {
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "On-chain generated skill ID.", value_name = "OBJECT_ID")]
        skill_id: sui::types::Address,
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
    #[command(about = "Print an execute-agent-skill PTB plan.")]
    Execute {
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "On-chain generated skill ID.", value_name = "OBJECT_ID")]
        skill_id: sui::types::Address,
        #[arg(long, help = "Input commitment bytes as hex.", value_name = "HEX")]
        input_commitment_hex: String,
    },
    #[command(about = "Print a schedule-skill-execution PTB plan.")]
    Schedule {
        #[arg(long, help = "On-chain generated agent ID.", value_name = "OBJECT_ID")]
        agent_id: sui::types::Address,
        #[arg(long, help = "On-chain generated skill ID.", value_name = "OBJECT_ID")]
        skill_id: sui::types::Address,
        #[arg(long, help = "Long-term gas coin ID.", value_name = "OBJECT_ID")]
        long_term_gas_coin_id: sui::types::Address,
        #[arg(long, help = "Input commitment bytes as hex.", value_name = "HEX")]
        input_commitment_hex: String,
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
            dag_id,
            tap_package_id,
            out,
        } => publish_skill_artifact(config, dag_id, tap_package_id, out).await,
        TapCommand::CreateAgent {
            operator,
            metadata_hash_hex,
            auth_mode,
        } => print_create_agent_plan(operator, metadata_hash_hex, auth_mode),
        TapCommand::RegisterSkill { artifact } => print_register_skill_plan(artifact).await,
        TapCommand::Announce {
            artifact,
            agent_id,
            skill_id,
            endpoint_object_id,
            endpoint_object_version,
        } => {
            print_announce_plan(
                artifact,
                agent_id,
                skill_id,
                endpoint_object_id,
                endpoint_object_version,
            )
            .await
        }
        TapCommand::Requirements { agent_id, skill_id } => {
            print_requirements_plan(agent_id, skill_id)
        }
        TapCommand::DryRun { config } => dry_run_skill(config).await,
        TapCommand::Execute {
            agent_id,
            skill_id,
            input_commitment_hex,
        } => print_execute_plan(agent_id, skill_id, input_commitment_hex),
        TapCommand::Schedule {
            agent_id,
            skill_id,
            long_term_gas_coin_id,
            input_commitment_hex,
        } => print_schedule_plan(
            agent_id,
            skill_id,
            long_term_gas_coin_id,
            input_commitment_hex,
        ),
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
  "vertices": [],
  "edges": [],
  "outputs": []
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
      "auth_mode": 0,
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

    let tap_package = resolve_relative(&config_path, config.tap_package_path.clone());
    let move_toml = tap_package.join("Move.toml");
    if !move_toml.exists() {
        handle.error();
        return Err(NexusCliError::Any(anyhow!(
            "TAP package Move.toml '{}' does not exist",
            move_toml.display()
        )));
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

pub(crate) async fn publish_skill_artifact(
    config_path: PathBuf,
    dag_id: sui::types::Address,
    tap_package_id: sui::types::Address,
    out: Option<PathBuf>,
) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config_path, None).await?;
    command_title!("Creating TAP publish artifact");

    let artifact = TapPublishArtifact::from_config(&config, dag_id, tap_package_id)
        .map_err(NexusCliError::Any)?;
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

fn print_create_agent_plan(
    operator: sui::types::Address,
    metadata_hash_hex: String,
    auth_mode: u8,
) -> AnyResult<(), NexusCliError> {
    let metadata_hash = decode_hex_arg(&metadata_hash_hex, "metadata-hash")?;
    json_output(&json!({
        "dry_run": true,
        "builder": "transactions::tap::create_agent",
        "function": TapStandard::CREATE_AGENT.name.to_string(),
        "operator": operator,
        "metadata_hash": metadata_hash,
        "auth_mode": auth_mode
    }))
}

async fn print_register_skill_plan(artifact: PathBuf) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    json_output(&json!({
        "dry_run": true,
        "builder": "transactions::tap::register_skill",
        "function": TapStandard::REGISTER_SKILL.name.to_string(),
        "dag_id": artifact.dag_id,
        "tap_package_id": artifact.tap_package_id,
        "skill_id": "generated_on_chain_by_register_skill"
    }))
}

async fn print_announce_plan(
    artifact: PathBuf,
    agent_id: sui::types::Address,
    skill_id: sui::types::Address,
    endpoint_object_id: sui::types::Address,
    endpoint_object_version: u64,
) -> AnyResult<(), NexusCliError> {
    let artifact = read_artifact(artifact).await?;
    json_output(&json!({
        "dry_run": true,
        "builder": "transactions::tap::announce_endpoint_revision",
        "function": TapStandard::ANNOUNCE_ENDPOINT_REVISION.name.to_string(),
        "agent_id": AgentId(agent_id),
        "skill_id": SkillId(skill_id),
        "tap_package_id": artifact.tap_package_id,
        "endpoint_object_id": endpoint_object_id,
        "endpoint_object_version": endpoint_object_version,
        "interface_revision": artifact.interface_revision,
        "config_digest_hex": artifact.config_digest_hex
    }))
}

fn print_requirements_plan(
    agent_id: sui::types::Address,
    skill_id: sui::types::Address,
) -> AnyResult<(), NexusCliError> {
    json_output(&json!({
        "dry_run": true,
        "builder": "transactions::tap::get_skill_requirements",
        "function": TapStandard::GET_SKILL_REQUIREMENTS.name.to_string(),
        "agent_id": AgentId(agent_id),
        "skill_id": SkillId(skill_id)
    }))
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

fn print_execute_plan(
    agent_id: sui::types::Address,
    skill_id: sui::types::Address,
    input_commitment_hex: String,
) -> AnyResult<(), NexusCliError> {
    let input_commitment = decode_hex_arg(&input_commitment_hex, "input-commitment")?;
    json_output(&json!({
        "dry_run": true,
        "builder": "transactions::tap::execute_agent_skill",
        "function": TapStandard::EXECUTE_AGENT_SKILL.name.to_string(),
        "agent_id": AgentId(agent_id),
        "skill_id": SkillId(skill_id),
        "input_commitment": input_commitment
    }))
}

fn print_schedule_plan(
    agent_id: sui::types::Address,
    skill_id: sui::types::Address,
    long_term_gas_coin_id: sui::types::Address,
    input_commitment_hex: String,
) -> AnyResult<(), NexusCliError> {
    let input_commitment = decode_hex_arg(&input_commitment_hex, "input-commitment")?;
    json_output(&json!({
        "dry_run": true,
        "builder": "transactions::tap::schedule_skill_execution",
        "function": TapStandard::SCHEDULE_SKILL_EXECUTION.name.to_string(),
        "agent_id": AgentId(agent_id),
        "skill_id": SkillId(skill_id),
        "long_term_gas_coin_id": long_term_gas_coin_id,
        "input_commitment": input_commitment
    }))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        assert_matches::assert_matches,
        nexus_sdk::types::{
            InterfaceRevision,
            TapPaymentPolicy,
            TapSchedulePolicy,
            TapSkillRequirements,
            TapVertexAuthorizationSchema,
        },
    };

    #[tokio::test]
    async fn scaffold_validate_and_publish_artifact_flow() {
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

        let artifact_path = root.join("artifact.json");
        publish_skill_artifact(
            config_path,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
            Some(artifact_path.clone()),
        )
        .await
        .expect("artifact generation succeeds");

        let artifact_text = tokio::fs::read_to_string(artifact_path).await.unwrap();
        let artifact: TapPublishArtifact = serde_json::from_str(&artifact_text).unwrap();
        assert_eq!(artifact.dag_id, sui::types::Address::from_static("0xd"));
        assert_eq!(
            artifact.tap_package_id,
            sui::types::Address::from_static("0xe")
        );
        assert_eq!(artifact.config_digest_hex.len(), 64);
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
}
