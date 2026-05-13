use super::*;

pub(crate) async fn announce_endpoint_revision(
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
            agent_id,
            skill_id,
            &artifact,
            endpoint_object_id,
            active_for_new_executions,
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&announce_result_json(&artifact, &result).map_err(NexusCliError::Any)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn announce_missing_artifact_fails_before_rpc_client() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let error = announce_endpoint_revision(
            tempdir.path().join("missing-artifact.json"),
            sui::types::Address::from_static("0xa"),
            11,
            None,
            true,
            None,
            DEFAULT_GAS_BUDGET,
        )
        .await
        .expect_err("missing artifact should fail");

        assert!(
            error.to_string().contains("No such file") || error.to_string().contains("not found"),
            "unexpected error: {error}"
        );
    }
}
