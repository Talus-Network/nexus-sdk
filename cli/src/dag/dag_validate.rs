use {
    crate::{command_title, dag::validator::validate, loading, prelude::*},
    nexus_sdk::types::Dag,
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
    let dag: Dag = match serde_json::from_str(file.as_str()) {
        Ok(dag) => dag,
        Err(e) => {
            parsing_handle.error();

            return Err(NexusCliError::Any(anyhow!(e)));
        }
    };

    parsing_handle.success();

    let validation_handle = loading!("Validating Nexus DAG...");

    // Validate the dag.
    match validate(dag.clone()) {
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
    use {super::*, assert_matches::assert_matches, nexus_sdk::types::Dag};

    // == Various graph shapes ==

    #[test]
    fn test_ig_story_planner_valid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/ig_story_planner_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_immediately_converges_valid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/immediately_converges_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_immediately_converges_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/immediately_converges_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: b.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_intertwined_valid() {
        let dag: Dag = serde_json::from_str(include_str!("_dags/intertwined_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_intertwined_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/intertwined_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_multiple_output_ports_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/multiple_output_ports_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_multiple_output_ports_valid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/multiple_output_ports_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_multiple_goals_valid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/multiple_goals_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_dead_ends_valid() {
        let dag: Dag = serde_json::from_str(include_str!("_dags/dead_ends_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_multiple_entry_multiple_goal_valid() {
        let dag: Dag = serde_json::from_str(include_str!(
            "_dags/multiple_entry_multiple_goal_valid.json"
        ))
        .unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_multiple_entry_multiple_goal_invalid() {
        let dag: Dag = serde_json::from_str(include_str!(
            "_dags/multiple_entry_multiple_goal_invalid.json"
        ))
        .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: c.1' has a race condition on it when invoking group 'group_a'"));
    }

    #[test]
    fn test_branched_net_zero_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/branched_net_zero_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_entry_groups_valid() {
        let dag: Dag = serde_json::from_str(include_str!("_dags/entry_groups_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_entry_groups_twice_valid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/entry_groups_twice_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn test_entry_groups_ne_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/entry_groups_ne_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: e.2' is unreachable when invoking group 'group_b'"));
    }

    #[test]
    fn test_entry_groups_tm_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/entry_groups_tm_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: e.2' has a race condition on it when invoking group 'group_b'"));
    }

    // == Cyclic or no input graphs ==

    #[test]
    fn test_cyclic_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/undefined_connections_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Entry 'Vertex: a' is not connected to the DAG."));
    }

    // == Parser tests ==

    #[test]
    fn test_undefined_connections_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/undefined_connections_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Entry 'Vertex: a' is not connected to the DAG."));
    }

    #[test]
    fn test_def_val_on_input_port_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/has_def_on_input_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: location_decider.context' is already present in the graph or has an edge leading into it and therefore cannot have a default value."));
    }

    #[test]
    fn test_multiple_same_entry_ports_invalid() {
        let dag: Dag = serde_json::from_str(include_str!(
            "_dags/has_multiple_same_entry_inputs_invalid.json"
        ))
        .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Entry 'Input port: location_decider.messages' is defined multiple times."));
    }

    #[test]
    fn test_empty_invalid() {
        let dag: Dag = serde_json::from_str(include_str!("_dags/empty_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("The DAG has no entry vertices."));
    }

    #[test]
    fn test_normal_vertex_in_group_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/normal_vertex_in_group_invalid.json"))
                .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Entry group 'group_b' references a non-entry 'Vertex: e'."));
    }

    #[test]
    fn test_both_vertex_and_entry_vertex_invalid() {
        let dag: Dag = serde_json::from_str(include_str!(
            "_dags/both_vertex_and_entry_vertex_invalid.json"
        ))
        .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: b' is both a vertex and an entry vertex."));
    }
}
