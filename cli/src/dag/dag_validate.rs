use {
    crate::{
        command_title,
        dag::{
            parser::{Dag, VertexType},
            validator::validate,
        },
        loading,
        prelude::*,
    },
    petgraph::graph::DiGraph,
};

/// Validate if a JSON file at the provided location is a valid Nexus DAG. If so,
/// return the parsed DAG.
pub(crate) async fn validate_dag(path: PathBuf) -> AnyResult<Dag, NexusCliError> {
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

    // Parse into [crate::dag::parser::Dag].
    let dag: Dag = match file.as_str().try_into() {
        Ok(dag) => dag,
        Err(e) => {
            parsing_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    parsing_handle.success();

    let validation_handle = loading!("Validating Nexus DAG...");

    // Parse the struct into a [petgraph::graph::DiGraph].
    let graph: DiGraph<VertexType, ()> = match dag.clone().try_into() {
        Ok(graph) => graph,
        Err(e) => {
            validation_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    // Validate the graph.
    match validate(&graph) {
        Ok(()) => {
            validation_handle.success();

            Ok(dag)
        }
        Err(e) => {
            validation_handle.error();

            return Err(NexusCliError::Any(e));
        }
    }
}
