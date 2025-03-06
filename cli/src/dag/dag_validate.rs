use {
    crate::{
        command_title,
        dag::{
            parser::Dag,
            validator::{validate, NodeIdent},
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
    let graph: DiGraph<NodeIdent, ()> = match dag.clone().try_into() {
        Ok(graph) => graph,
        Err(e) => {
            validation_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "{e}\n\nSee more about DAG rules at <https://github.com/Talus-Network/nexus-next/wiki/Package:-Workflow#rules>",
            )));
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

            Err(NexusCliError::Any(anyhow!(
                "{e}\n\nSee more about DAG rules at <https://github.com/Talus-Network/nexus-next/wiki/Package:-Workflow#rules>",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    // == Various graph shapes ==

    #[test]
    fn test_ig_story_planner_valid() {
        let conf: Dag = include_str!("_dags/ig_story_planner_valid.json")
            .try_into()
            .unwrap();

        assert!(validate(&conf.try_into().unwrap()).is_ok());
    }

    #[test]
    fn test_immediately_converges_valid() {
        let conf: Dag = include_str!("_dags/immediately_converges_valid.json")
            .try_into()
            .unwrap();

        assert!(validate(&conf.try_into().unwrap()).is_ok());
    }

    #[test]
    fn test_immediately_converges_invalid() {
        let conf: Dag = include_str!("_dags/immediately_converges_invalid.json")
            .try_into()
            .unwrap();

        let res = validate(&conf.try_into().unwrap());

        assert_matches!(res, Err(e) if e.to_string().contains("Graph does not follow concurrency rules."));
    }

    #[test]
    fn test_intertwined_valid() {
        let conf: Dag = include_str!("_dags/intertwined_valid.json")
            .try_into()
            .unwrap();

        assert!(validate(&conf.try_into().unwrap()).is_ok());
    }

    #[test]
    fn test_intertwined_invalid() {
        let conf: Dag = include_str!("_dags/intertwined_invalid.json")
            .try_into()
            .unwrap();

        let res = validate(&conf.try_into().unwrap());

        assert_matches!(res, Err(e) if e.to_string().contains("Graph does not follow concurrency rules."));
    }

    #[test]
    fn test_multiple_output_ports_invalid() {
        let conf: Dag = include_str!("_dags/multiple_output_ports_invalid.json")
            .try_into()
            .unwrap();

        let res = validate(&conf.try_into().unwrap());

        assert_matches!(res, Err(e) if e.to_string().contains("Graph does not follow concurrency rules."));
    }

    #[test]
    fn test_multiple_output_ports_valid() {
        let conf: Dag = include_str!("_dags/multiple_output_ports_valid.json")
            .try_into()
            .unwrap();

        assert!(validate(&conf.try_into().unwrap()).is_ok());
    }

    #[test]
    fn test_multiple_goals_valid() {
        let conf: Dag = include_str!("_dags/multiple_goals_valid.json")
            .try_into()
            .unwrap();

        assert!(validate(&conf.try_into().unwrap()).is_ok());
    }

    #[test]
    fn test_dead_ends_valid() {
        let conf: Dag = include_str!("_dags/dead_ends_valid.json")
            .try_into()
            .unwrap();

        assert!(validate(&conf.try_into().unwrap()).is_ok());
    }

    #[test]
    fn test_multiple_entry_multiple_goal_valid() {
        let conf: Dag = include_str!("_dags/multiple_entry_multiple_goal_valid.json")
            .try_into()
            .unwrap();

        assert!(validate(&conf.try_into().unwrap()).is_ok());
    }

    #[test]
    fn test_multiple_entry_multiple_goal_invalid() {
        let conf: Dag = include_str!("_dags/multiple_entry_multiple_goal_invalid.json")
            .try_into()
            .unwrap();

        let res = validate(&conf.try_into().unwrap());

        assert_matches!(res, Err(e) if e.to_string().contains("Graph does not follow concurrency rules."));
    }

    #[test]
    fn test_branched_net_zero_invalid() {
        let conf: Dag = include_str!("_dags/branched_net_zero_invalid.json")
            .try_into()
            .unwrap();

        let res = validate(&conf.try_into().unwrap());

        assert_matches!(res, Err(e) if e.to_string().contains("Graph does not follow concurrency rules."));
    }

    // == Cyclic or no input graphs ==

    #[test]
    fn test_cyclic_invalid() {
        let conf: Dag = include_str!("_dags/undefined_connections_invalid.json")
            .try_into()
            .unwrap();

        let res: AnyResult<DiGraph<NodeIdent, ()>> = conf.try_into();

        assert_matches!(res, Err(e) if e.to_string().contains("Entry 'Vertex: a' does not exist in the graph."));
    }

    #[test]
    fn test_empty_invalid() {
        let conf: Dag = include_str!("_dags/empty_invalid.json").try_into().unwrap();

        let res = validate(&conf.try_into().unwrap());

        assert_matches!(res, Err(e) if e.to_string().contains("The DAG has no entry vertices."));
    }

    // == Parser tests ==

    #[test]
    fn test_undefined_connections_invalid() {
        let conf: Dag = include_str!("_dags/undefined_connections_invalid.json")
            .try_into()
            .unwrap();

        let res: AnyResult<DiGraph<NodeIdent, ()>> = conf.try_into();

        assert_matches!(res, Err(e) if e.to_string().contains("Entry 'Vertex: a' does not exist in the graph."));
    }

    #[test]
    fn test_def_val_on_input_port_invalid() {
        let conf: Dag = include_str!("_dags/has_def_on_input_invalid.json")
            .try_into()
            .unwrap();

        let res: AnyResult<DiGraph<NodeIdent, ()>> = conf.try_into();

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: location_decider.context' is already present in the graph or has an edge leading into it and therefore cannot have a default value."));
    }

    #[test]
    fn test_multiple_same_entry_ports_invalid() {
        let conf: Dag = include_str!("_dags/has_multiple_same_entry_inputs_invalid.json")
            .try_into()
            .unwrap();

        let res: AnyResult<DiGraph<NodeIdent, ()>> = conf.try_into();

        assert_matches!(res, Err(e) if e.to_string().contains("Entry 'Input port: location_decider.messages' is specified multiple times."));
    }
}
