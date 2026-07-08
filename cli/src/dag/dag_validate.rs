use {
    crate::{command_title, loading, prelude::*},
    nexus_sdk::{
        dag::{json::parse_dag_spec, validator::validate},
        types::DagSpec,
    },
};

/// Validate if a JSON file at the provided location is a valid Nexus DAG. If so,
/// return the parsed DAG.
pub(crate) async fn validate_dag(path: PathBuf) -> AnyResult<DagSpec, NexusCliError> {
    command_title!("Validating Nexus DAG at '{path}'", path = path.display());

    let parsing_handle = loading!("Parsing JSON file...");

    // Read file.
    let file = match tokio::fs::read_to_string(path).await {
        Ok(file) => file,
        Err(e) => {
            parsing_handle.error();

            return Err(NexusCliError::Io(e));
        }
    };

    let dag = match parse_dag_spec(file.as_str()) {
        Ok(dag) => dag,
        Err(e) => {
            parsing_handle.error();

            return Err(NexusCliError::Any(anyhow!(e)));
        }
    };

    parsing_handle.success();

    let validation_handle = loading!("Validating Nexus DAG...");

    // Validate the dag.
    match validate(&dag) {
        Ok(()) => {
            validation_handle.success();

            Ok(dag)
        }
        Err(e) => {
            validation_handle.error();

            Err(NexusCliError::Any(anyhow!(
                "{e}\n\nSee more about DAG rules at <https://github.com/Talus-Network/nexus-next/wiki/Package:-Workflow#rules>",
            )))
        }
    }
}
