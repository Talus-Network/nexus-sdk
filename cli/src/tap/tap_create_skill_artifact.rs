use {
    super::*,
    nexus_sdk::{
        move_bindings::interface::{
            agent::{FixedTool, SkillRequirement},
            dag::DAG,
            payment::SkillPaymentPolicy,
            version::InterfaceVersion,
        },
        nexus::workflow::fetch_dag_vertices_bcs,
        types::{tap_input_commitment_from_dag_inputs, validate_requirements},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ArtifactPaymentMode {
    UserFunded,
    AgentFunded,
}

pub(crate) async fn create_skill_artifact(
    skill_name: String,
    dag_id: sui::types::Address,
    interface_revision: u64,
    payment_mode: ArtifactPaymentMode,
    agent_funded_max_budget_mist: Option<u64>,
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
    fixed_tool: Vec<String>,
    out: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Creating TAP skill publish artifact for '{skill_name}'");

    let input_commitment = fetch_input_commitment(dag_id).await?;
    let artifact = build_artifact(
        skill_name,
        dag_id,
        interface_revision,
        input_commitment,
        payment_mode,
        agent_funded_max_budget_mist,
        recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
        fixed_tool,
    )?;
    let artifact_json =
        serde_json::to_string_pretty(&artifact).map_err(|e| NexusCliError::Any(e.into()))?;

    if let Some(parent) = out.parent() {
        create_dir_all(parent).await.map_err(NexusCliError::Io)?;
    }
    tokio::fs::write(&out, artifact_json.as_bytes())
        .await
        .map_err(NexusCliError::Io)?;

    notify_success!("Wrote TAP skill publish artifact to {}", out.display());
    json_output(&create_skill_artifact_result_json(&artifact))
}

#[allow(clippy::too_many_arguments)]
fn build_artifact(
    skill_name: String,
    dag_id: sui::types::Address,
    interface_revision: u64,
    input_commitment: Vec<u8>,
    payment_mode: ArtifactPaymentMode,
    agent_funded_max_budget_mist: Option<u64>,
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
    fixed_tool: Vec<String>,
) -> AnyResult<TapPublishArtifact, NexusCliError> {
    if skill_name.trim().is_empty() {
        return Err(NexusCliError::Any(anyhow!("skill name must not be empty")));
    }

    let payment_policy = payment_policy_from_cli(payment_mode, agent_funded_max_budget_mist)?;
    let schedule_policy = schedule_policy_from_cli(
        &recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    )?;
    let fixed_tools = fixed_tool
        .into_iter()
        .map(parse_fixed_tool)
        .collect::<AnyResult<Vec<_>, _>>()?;

    let requirements = SkillRequirement {
        input_commitment,
        payment_policy,
        schedule_policy,
        fixed_tools,
    };
    validate_requirements(&requirements).map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    Ok(TapPublishArtifact {
        skill_name,
        dag_id,
        interface_revision: InterfaceVersion::new(interface_revision),
        requirements,
    })
}

async fn fetch_input_commitment(dag_id: sui::types::Address) -> AnyResult<Vec<u8>, NexusCliError> {
    let nexus_client = get_read_only_nexus_client().await?;
    fetch_input_commitment_with_client(&nexus_client, dag_id).await
}

async fn fetch_input_commitment_with_client(
    nexus_client: &nexus_sdk::nexus::client::NexusClient,
    dag_id: sui::types::Address,
) -> AnyResult<Vec<u8>, NexusCliError> {
    let crawler = nexus_client.crawler();
    let dag = crawler.get_object::<DAG>(dag_id).await.map_err(|error| {
        NexusCliError::Any(anyhow!(
            "failed to fetch DAG '{dag_id}' for TAP input commitment: {error}"
        ))
    })?;
    let vertices = fetch_dag_vertices_bcs(crawler, &dag.data)
        .await
        .map_err(|error| {
            NexusCliError::Any(anyhow!(
                "failed to fetch DAG '{dag_id}' vertex input data for TAP input commitment: {error}"
            ))
        })?;
    let input_pairs = vertices.iter().flat_map(|(vertex, info)| {
        info.input_ports
            .contents
            .iter()
            .map(move |port| (vertex.name.as_str(), port.name.as_str()))
    });

    Ok(tap_input_commitment_from_dag_inputs(input_pairs))
}

fn payment_policy_from_cli(
    mode: ArtifactPaymentMode,
    agent_funded_max_budget_mist: Option<u64>,
) -> AnyResult<SkillPaymentPolicy, NexusCliError> {
    match mode {
        ArtifactPaymentMode::UserFunded => {
            if let Some(max_budget_mist) = agent_funded_max_budget_mist {
                return Err(NexusCliError::Any(anyhow!(
                    "--agent-funded-max-budget-mist={max_budget_mist} is only valid with --payment-mode agent-funded"
                )));
            }
            Ok(SkillPaymentPolicy::user_funded())
        }
        ArtifactPaymentMode::AgentFunded => {
            let max_budget_mist = agent_funded_max_budget_mist.ok_or_else(|| {
                NexusCliError::Any(anyhow!(
                    "--agent-funded-max-budget-mist is required with --payment-mode agent-funded"
                ))
            })?;
            if max_budget_mist == 0 {
                return Err(NexusCliError::Any(anyhow!(
                    "--agent-funded-max-budget-mist must be greater than zero"
                )));
            }
            Ok(SkillPaymentPolicy::agent_funded(max_budget_mist))
        }
    }
}

fn parse_fixed_tool(value: String) -> AnyResult<FixedTool, NexusCliError> {
    let (registry, fqn) = value.split_once('=').ok_or_else(|| {
        NexusCliError::Any(anyhow!(
            "invalid fixed-tool '{value}': expected '<TOOL_REGISTRY_ID>=<TOOL_FQN>'"
        ))
    })?;
    if fqn.trim().is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "invalid fixed-tool '{value}': tool FQN must not be empty"
        )));
    }

    let tool_registry_id = registry.parse::<sui::types::Address>().map_err(|e| {
        NexusCliError::Any(anyhow!("invalid fixed-tool registry id '{registry}': {e}"))
    })?;

    Ok(FixedTool::new(tool_registry_id, fqn))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            move_bindings::{
                interface::graph,
                move_std::option::Option as MoveOption,
                sui_framework::{
                    linked_table::LinkedTable,
                    object::UID,
                    table::Table,
                    vec_map::VecMap,
                },
            },
            test_utils::{nexus_mocks, sui_mocks},
        },
    };

    #[test]
    fn build_artifact_uses_dag_derived_input_commitment() {
        let artifact = build_artifact(
            "owned sum skill".to_string(),
            sui::types::Address::from_static("0xd"),
            1,
            b"sum-input".to_vec(),
            ArtifactPaymentMode::AgentFunded,
            Some(10_000),
            "once".to_string(),
            0,
            1,
            false,
            vec![format!(
                "{}=xyz.taluslabs.sum@1",
                sui::types::Address::from_static("0xf")
            )],
        )
        .expect("artifact creation succeeds");

        assert_eq!(artifact.skill_name, "owned sum skill");
        assert_eq!(artifact.dag_id, sui::types::Address::from_static("0xd"));
        assert_eq!(artifact.interface_revision, InterfaceVersion::new(1));
        assert_eq!(
            artifact.requirements.input_commitment,
            b"sum-input".to_vec()
        );
        assert_eq!(
            artifact.requirements.payment_policy,
            SkillPaymentPolicy::agent_funded(10_000)
        );
        assert_eq!(artifact.requirements.fixed_tools.len(), 1);
    }

    #[test]
    fn build_artifact_rejects_ambiguous_payment_flags() {
        let error = build_artifact(
            "skill".to_string(),
            sui::types::Address::from_static("0xd"),
            1,
            vec![1],
            ArtifactPaymentMode::UserFunded,
            Some(10),
            "once".to_string(),
            0,
            1,
            false,
            Vec::new(),
        )
        .expect_err("user funded cannot carry agent budget");
        assert!(
            error
                .to_string()
                .contains("only valid with --payment-mode agent-funded"),
            "unexpected error: {error}"
        );

        let error = build_artifact(
            "skill".to_string(),
            sui::types::Address::from_static("0xd"),
            1,
            vec![1],
            ArtifactPaymentMode::AgentFunded,
            None,
            "once".to_string(),
            0,
            1,
            false,
            Vec::new(),
        )
        .expect_err("agent funded requires budget");
        assert!(
            error
                .to_string()
                .contains("--agent-funded-max-budget-mist is required"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parse_fixed_tool_requires_registry_and_fqn() {
        let fixed_tool = parse_fixed_tool(format!(
            "{}=xyz.taluslabs.sum@1",
            sui::types::Address::from_static("0xa")
        ))
        .expect("fixed tool parses");
        assert_eq!(
            fixed_tool.tool_registry_address(),
            sui::types::Address::from_static("0xa")
        );
        assert_eq!(fixed_tool.tool_fqn_string(), "xyz.taluslabs.sum@1");

        let error = parse_fixed_tool("xyz.taluslabs.sum@1".to_string())
            .expect_err("missing separator should fail");
        assert!(
            error.to_string().contains("expected '<TOOL_REGISTRY_ID>"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn fetch_input_commitment_succeeds_without_owned_coins() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_id = sui::types::Address::from_static("0xd");
        let dag_ref = sui_mocks::object_ref_for_id(dag_id);
        let dag = DAG {
            id: UID::new(dag_id),
            vertices: LinkedTable::new(sui::types::Address::from_static("0x10"), 0),
            entry_groups: VecMap { contents: vec![] },
            edges: Table::new(sui::types::Address::from_static("0x11"), 0),
            outputs: Table::new(sui::types::Address::from_static("0x12"), 0),
            defaults_to_input_ports: Table::new(sui::types::Address::from_static("0x13"), 0),
            post_failure_action: MoveOption::from_option(None::<graph::PostFailureAction>),
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_get_object_value_bcs_for(
            &mut ledger_service_mock,
            dag_ref,
            sui::types::Owner::Shared(1),
            &dag,
            nexus_sdk::move_bindings::struct_tag::<DAG>(&nexus_objects),
        );
        sui_mocks::grpc::mock_empty_dynamic_fields(&mut state_service_mock, 1);
        sui_mocks::grpc::mock_empty_batch_get_objects(&mut ledger_service_mock, 1);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await;

        let commitment = fetch_input_commitment_with_client(&client, dag_id)
            .await
            .expect("DAG input read should not require owned coins");

        assert_eq!(
            commitment,
            tap_input_commitment_from_dag_inputs(std::iter::empty::<(&str, &str)>())
        );
    }
}
