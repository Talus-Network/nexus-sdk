use {
    crate::types::{Dag, EdgeKind, DEFAULT_ENTRY_GROUP},
    anyhow::{bail, Result as AnyResult},
    petgraph::{
        graph::{DiGraph, NodeIndex},
        visit::EdgeRef,
    },
    std::collections::{HashMap, HashSet},
};

type GraphAndVertexEntryGroups = (
    DiGraph<GraphNode, EdgeKind>,
    HashMap<GraphNode, Vec<String>>,
);

/// Validate function takes a DAG and validates it based on nexus execution
/// rules.
///
/// See our wiki for more information on the rules:
/// <https://docs.talus.network/talus-documentation/devs/index/workflow#rules>
/// <https://docs.talus.network/talus-documentation/devs/index-1/cli#nexus-dag>
pub fn validate(dag: Dag) -> AnyResult<()> {
    // Parse the dag into a petgraph DiGraph.
    let (graph, vertex_entry_groups) = try_into_graph(dag)?;

    if !graph.is_directed() || petgraph::algo::is_cyclic_directed(&graph) {
        bail!("The provided graph contains one or more cycles.");
    }

    // Check that the shape of the graph is correct.
    has_correct_order_of_actions(&graph)?;

    // Check that no walks in the graph violate the concurrency rules.
    follows_concurrency_rules(&graph, &vertex_entry_groups)?;

    // Check that for-each and collect edges are correctly paired and not nesting.
    validate_for_each_pairs(&graph, &vertex_entry_groups)?;

    // Check that do-whiles don't nest.
    validate_do_while_nesting(&graph)?;

    Ok(())
}

fn has_correct_order_of_actions(graph: &DiGraph<GraphNode, EdgeKind>) -> AnyResult<()> {
    for node in graph.node_indices() {
        let vertex = &graph[node];
        let neighbors = graph
            .neighbors_directed(node, petgraph::Direction::Outgoing)
            .collect::<Vec<NodeIndex>>();

        // Check if the vertex has the correct number of edges.
        match vertex {
            // Input ports must have exactly 1 outgoing edge.
            GraphNode::InputPort { .. } if neighbors.len() != 1 => {
                bail!("'{vertex}' must have exactly 1 outgoing edge")
            }
            // Tools can be the last vertex and can have any number of edges.
            GraphNode::Vertex { .. } => (),
            // Output variants must have at least 1 outgoing edge.
            GraphNode::OutputVariant { .. } if neighbors.is_empty() => {
                bail!("'{vertex}' must have at least 1 outgoing edge")
            }
            // Output ports must have exactly 1 outgoing edge.
            GraphNode::OutputPort { .. } if neighbors.len() != 1 => {
                bail!("'{vertex}' must have exactly 1 outgoing edge")
            }
            _ => (),
        };

        // Check if the edges are connected in the correct order.
        for node in neighbors {
            let neighbor = graph[node].clone();

            let is_ok = match vertex {
                GraphNode::InputPort { .. } => matches!(neighbor, GraphNode::Vertex { .. }),
                GraphNode::Vertex { .. } => matches!(neighbor, GraphNode::OutputVariant { .. }),
                GraphNode::OutputVariant { .. } => matches!(neighbor, GraphNode::OutputPort { .. }),
                GraphNode::OutputPort { .. } => matches!(neighbor, GraphNode::InputPort { .. }),
            };

            if !is_ok {
                bail!("The edge from '{vertex}' to '{neighbor}' is invalid.");
            }
        }
    }

    Ok(())
}

/// For each distinct group of entry vertices, check that the net concurrency
/// leading from these nodes into any input port is always 0.
fn follows_concurrency_rules(
    graph: &DiGraph<GraphNode, EdgeKind>,
    vertex_entry_groups: &HashMap<GraphNode, Vec<String>>,
) -> AnyResult<()> {
    // Get all distinct groups of entry vertices.
    let groups = vertex_entry_groups
        .values()
        .flatten()
        .collect::<HashSet<_>>();

    // For each group...
    for group in groups {
        // ... find the entry vertices in that group.
        let entry_vertices = graph
            .node_indices()
            .filter(|&node| {
                vertex_entry_groups
                    .get(&graph[node])
                    .unwrap_or(&vec![])
                    .contains(group)
            })
            .collect::<Vec<_>>();

        let input_ports = graph
            .node_indices()
            .filter(|&node| matches!(graph[node], GraphNode::InputPort { .. }));

        // And then for each input port ...
        for input_port in input_ports {
            // ... find all nodes that are included in the paths leading to
            // the input port.
            let all_nodes_in_paths = entry_vertices
                .iter()
                .flat_map(|&entry_vertex| {
                    let min_intermediate_nodes = 0;
                    let max_intermediate_nodes = None;

                    petgraph::algo::all_simple_paths(
                        graph,
                        entry_vertex,
                        input_port,
                        min_intermediate_nodes,
                        max_intermediate_nodes,
                    )
                    .flat_map(|path: HashSet<_>| path)
                })
                .collect::<HashSet<_>>();

            // If there is no path to this input port then it is unreachable.
            if all_nodes_in_paths.is_empty() {
                bail!(
                    "'{}' is unreachable when invoking group '{}'",
                    graph[input_port],
                    group
                );
            }

            let concurrency = get_net_concurrency_in_subgraph(graph, &all_nodes_in_paths);

            if concurrency < 0 {
                bail!(
                    "'{}' is unreachable when invoking group '{}'",
                    graph[input_port],
                    group
                )
            }

            if concurrency > 0 {
                bail!(
                    "'{}' has a race condition on it when invoking group '{}'",
                    graph[input_port],
                    group
                )
            }
        }
    }

    Ok(())
}

fn get_net_concurrency_in_subgraph(
    graph: &DiGraph<GraphNode, EdgeKind>,
    nodes: &HashSet<NodeIndex>,
) -> isize {
    let net_concurrency = nodes.iter().fold(0, |acc, &node| {
        match graph[node] {
            GraphNode::Vertex { .. } => {
                // Calculate the maximum number of concurrent tasks that can be spawned by this tool.
                let max_tool_concurrency = graph
                    .neighbors_directed(node, petgraph::Outgoing)
                    // Only filter variants that are in the paths.
                    .filter(|variant| nodes.contains(variant))
                    .map(|variant| {
                        let output_ports = graph
                            .neighbors_directed(variant, petgraph::Outgoing)
                            // Only filter ports that are in the paths.
                            .filter(|port| nodes.contains(port))
                            .count() as isize;

                        // Subtract 1 because if there's only 1 output port, there's no concurrency.
                        output_ports - 1
                    })
                    .fold(0, isize::max);

                // Add 1 as we only want to consume concurrency if there's more than 1 input port.
                acc + max_tool_concurrency + 1
            }
            // Input ports reduce concurrency.
            GraphNode::InputPort { .. } => acc - 1,
            _ => acc,
        }
    });

    // If the net concurrency is 0, the graph follows the concurrency rules.
    net_concurrency
}

/// Check the for-each and collect edges are correctly paired and not nesting.
fn validate_for_each_pairs(
    graph: &DiGraph<GraphNode, EdgeKind>,
    vertex_entry_groups: &HashMap<GraphNode, Vec<String>>,
) -> AnyResult<()> {
    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    enum ForEachState {
        Idle,
        InForEach,
    }

    use ForEachState::*;

    // Stack for DFS: (node, vertex, state)
    let mut stack = vec![];

    for vertex in vertex_entry_groups.keys() {
        let node = graph
            .node_indices()
            .find(|&n| &graph[n] == vertex)
            .ok_or_else(|| anyhow::anyhow!("'{vertex}' not found in graph nodes."))?;

        stack.push((node, vertex, Idle));
    }

    while let Some((node, vertex, state)) = stack.pop() {
        if graph.edges(node).count() == 0 {
            if state == InForEach {
                bail!(
                    "'{}' has a for-each edge without a corresponding collect",
                    vertex
                );
            }

            continue;
        }

        for edge in graph.edges(node) {
            let next_state = match (state, edge.weight()) {
                (Idle, EdgeKind::Normal) => Idle,
                (Idle, EdgeKind::DoWhile) => Idle,
                (Idle, EdgeKind::Break) => Idle,
                (Idle, EdgeKind::ForEach) => InForEach,
                (Idle, EdgeKind::Collect) => {
                    bail!("'{vertex}' has a collect edge without for-each");
                }

                (InForEach, EdgeKind::Normal) => InForEach,
                (InForEach, EdgeKind::ForEach) => {
                    bail!("'{vertex}' has a nested for-each");
                }
                (InForEach, EdgeKind::Collect) => Idle,
                (InForEach, EdgeKind::DoWhile) | (InForEach, EdgeKind::Break) => {
                    bail!("'{vertex}' has a do-while or break edge inside a for-each");
                }
            };

            stack.push((edge.target(), &graph[edge.target()], next_state));
        }
    }

    Ok(())
}

fn validate_do_while_nesting(graph: &DiGraph<GraphNode, EdgeKind>) -> AnyResult<()> {
    // Find original destination vertices for do-while edges.
    let vertices = graph
        .edge_references()
        .filter_map(|edge| match edge.weight() {
            EdgeKind::DoWhile => {
                let destination_node = graph.node_indices().find(|&n| {
                    if let GraphNode::InputPort { vertex, .. } = &graph[edge.target()] {
                        // The destination vertex name is the input port's vertex name without the "-do-while" suffix.
                        let original_vertex_name =
                            vertex.strip_suffix("-do-while").unwrap_or(vertex);
                        if let GraphNode::Vertex { name: v_name, .. } = &graph[n] {
                            return v_name == original_vertex_name;
                        }
                    }

                    false
                });

                destination_node.map(|n| (edge.source(), n))
            }
            _ => None,
        })
        .collect::<HashSet<_>>();

    for (source, target) in &vertices {
        // Check that none of the paths in the do-while loop contain another
        // loop edge.
        let min_intermediate_nodes = 0;
        let max_intermediate_nodes = None;

        let paths = petgraph::algo::all_simple_paths(
            graph,
            *target,
            *source,
            min_intermediate_nodes,
            max_intermediate_nodes,
        )
        .flat_map(|mut path: Vec<_>| {
            // Ignore the last node as it's the source node.
            path.pop();
            path
        })
        .collect::<HashSet<_>>();

        for path_node in paths {
            for edge in graph.edges(path_node) {
                if matches!(
                    edge.weight(),
                    EdgeKind::DoWhile | EdgeKind::Break | EdgeKind::ForEach | EdgeKind::Collect
                ) {
                    bail!(
                        "Do-while edge at '{}' has a nested loop inside.",
                        graph[*source]
                    );
                }
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum GraphNode {
    InputPort {
        vertex: String,
        name: String,
    },
    Vertex {
        name: String,
    },
    OutputVariant {
        vertex: String,
        name: String,
    },
    OutputPort {
        vertex: String,
        variant: String,
        name: String,
    },
}

impl std::fmt::Display for GraphNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphNode::InputPort { vertex, name } => write!(f, "Input port: {vertex}.{name}"),
            GraphNode::Vertex { name } => write!(f, "Vertex: {name}"),
            GraphNode::OutputVariant { vertex, name } => {
                write!(f, "Output variant: {vertex}.{name}")
            }
            GraphNode::OutputPort {
                vertex,
                variant,
                name,
            } => {
                write!(f, "Output port: {vertex}.{variant}.{name}")
            }
        }
    }
}

/// [Dag] to [petgraph::graph::DiGraph]. Also performs structure checks on the
/// graph.
fn try_into_graph(dag: Dag) -> AnyResult<GraphAndVertexEntryGroups> {
    let mut graph = DiGraph::<GraphNode, EdgeKind>::new();

    // Build a hash map of graph nodes that are part of entry groups. If there
    // are no entry groups specified, we assume all vertices that have entry
    // ports specified are part of the default entry group.
    let mut vertex_entry_groups: HashMap<GraphNode, Vec<String>> = HashMap::new();

    if let Some(entry_groups) = &dag.entry_groups {
        for entry_group in entry_groups {
            for vertex in &entry_group.vertices {
                let vertex_ident = GraphNode::Vertex {
                    name: vertex.clone(),
                };

                // Check that the group references a vertex that exists.
                if !dag.vertices.iter().any(|v| &v.name == vertex) {
                    bail!(
                        "Entry group '{}' references a non-existing vertex '{}'.",
                        entry_group.name,
                        vertex
                    );
                }

                let mut groups = vertex_entry_groups
                    .remove(&vertex_ident)
                    .unwrap_or_default();
                groups.push(entry_group.name.clone());
                vertex_entry_groups.insert(vertex_ident, groups);
            }
        }
    } else {
        // If there are no entry groups, all vertices that have entry ports
        // specified are part of the default entry group.
        for vertex in &dag.vertices {
            if vertex.entry_ports.is_none() {
                continue;
            }

            let vertex_ident = GraphNode::Vertex {
                name: vertex.name.clone(),
            };

            vertex_entry_groups.insert(vertex_ident, vec![DEFAULT_ENTRY_GROUP.to_string()]);
        }
    }

    // Check that there is at least one entry point.
    if vertex_entry_groups.is_empty() {
        bail!("The DAG has no entry vertices or ports.");
    }

    // Edges are always between an output port and an input port. We also
    // need to create edges between the tool, the output variant and the
    // output port if they don't exist yet.
    let mut graph_nodes: HashMap<GraphNode, NodeIndex> = HashMap::new();

    for edge in &dag.edges {
        let origin_vertex = GraphNode::Vertex {
            name: edge.from.vertex.clone(),
        };

        let output_variant = GraphNode::OutputVariant {
            vertex: edge.from.vertex.clone(),
            name: edge.from.output_variant.clone(),
        };

        let output_port = GraphNode::OutputPort {
            vertex: edge.from.vertex.clone(),
            variant: edge.from.output_variant.clone(),
            name: edge.from.output_port.clone(),
        };

        // To avoid looping, we create a "phantom" destination vertex for
        // do-while edges.
        let destination_vertex_name = match edge.kind {
            EdgeKind::DoWhile => format!("{}-do-while", edge.to.vertex),
            _ => edge.to.vertex.clone(),
        };

        let destination_vertex = GraphNode::Vertex {
            name: destination_vertex_name.clone(),
        };

        let input_port = GraphNode::InputPort {
            vertex: destination_vertex_name.clone(),
            name: edge.to.input_port.clone(),
        };

        // Create nodes if they don't exist yet.
        let origin_node = graph_nodes.get(&origin_vertex).copied().unwrap_or_else(|| {
            let node = graph.add_node(origin_vertex.clone());

            graph_nodes.insert(origin_vertex.clone(), node);

            node
        });

        let output_variant_node = graph_nodes
            .get(&output_variant)
            .copied()
            .unwrap_or_else(|| {
                let node = graph.add_node(output_variant.clone());

                graph_nodes.insert(output_variant.clone(), node);

                node
            });

        let output_port_node = graph_nodes.get(&output_port).copied().unwrap_or_else(|| {
            let node = graph.add_node(output_port.clone());

            graph_nodes.insert(output_port.clone(), node);

            node
        });

        let destination_node = graph_nodes
            .get(&destination_vertex)
            .copied()
            .unwrap_or_else(|| {
                let node = graph.add_node(destination_vertex.clone());

                graph_nodes.insert(destination_vertex.clone(), node);

                node
            });

        let input_port_node = graph_nodes.get(&input_port).copied().unwrap_or_else(|| {
            let node = graph.add_node(input_port.clone());

            graph_nodes.insert(input_port.clone(), node);

            node
        });

        // Check that these edges don't already exist.
        if graph.contains_edge(output_variant_node, output_port_node) {
            bail!("Edge from '{output_variant}' to '{output_port}' already exists.",);
        }

        if graph.contains_edge(output_port_node, input_port_node) {
            bail!("Edge from '{output_port}' to '{input_port}' already exists.",);
        }

        // These are allowed.
        if !graph.contains_edge(origin_node, output_variant_node) {
            graph.add_edge(origin_node, output_variant_node, EdgeKind::Normal);
        }

        if !graph.contains_edge(input_port_node, destination_node) {
            graph.add_edge(input_port_node, destination_node, EdgeKind::Normal);
        }

        graph.add_edge(output_variant_node, output_port_node, EdgeKind::Normal);
        graph.add_edge(output_port_node, input_port_node, edge.kind.clone());
    }

    // Ensure we don't have duplicate vertices.
    let mut all_vertices = HashSet::new();
    let mut all_entry_ports = HashSet::new();

    // Check that all normal vertices are in the graph and are unique.
    for vertex in &dag.vertices {
        let vertex_ident = GraphNode::Vertex {
            name: vertex.name.clone(),
        };

        // If the dag has no edges and only 1 vertex, we add this vertex as a
        // graph node.
        if dag.edges.is_empty() && dag.vertices.len() == 1 {
            let node = graph.add_node(vertex_ident.clone());

            graph_nodes.insert(vertex_ident.clone(), node);
        }

        if !graph_nodes.contains_key(&vertex_ident) {
            bail!("'{vertex_ident}' is not connected to the DAG.",);
        }

        if !all_vertices.insert(vertex_ident.clone()) {
            bail!("'{vertex_ident}' is defined multiple times.",);
        }
    }

    // Check that entry ports are not defined twice and that they have no edges
    // leading into them.
    for vertex in &dag.vertices {
        let Some(entry_ports) = &vertex.entry_ports else {
            continue;
        };

        for entry_port in entry_ports {
            let entry_port_ident = GraphNode::InputPort {
                vertex: vertex.name.clone(),
                name: entry_port.name.clone(),
            };

            if graph_nodes.contains_key(&entry_port_ident) {
                bail!("'{entry_port_ident}' has an edge leading to it and therefore cannot be an entry port.",);
            }

            if !all_entry_ports.insert(entry_port_ident.clone()) {
                bail!("'{entry_port_ident}' is defined multiple times.",);
            }
        }
    }

    // Check that none of the default value input ports are in the graph.
    let default_values = dag.default_values.unwrap_or_default();

    for default_value in default_values {
        let default_value = GraphNode::InputPort {
            vertex: default_value.vertex,
            name: default_value.input_port,
        };

        if graph_nodes.contains_key(&default_value) || all_entry_ports.contains(&default_value) {
            bail!(
                "'{default_value}' is an entry port or has an edge leading into it and therefore cannot have a default value.",
            );
        }
    }

    // Check that outputs are not defined on vertices that have outgoing edges.
    for vertex in &dag.vertices {
        let outputs = dag.outputs.clone().unwrap_or_default();

        let has_outputs = outputs.iter().any(|output| output.vertex == vertex.name);
        let has_edges = dag.edges.iter().any(|edge| edge.from.vertex == vertex.name);

        let vertex_ident = GraphNode::Vertex {
            name: vertex.name.clone(),
        };

        if has_outputs && has_edges {
            bail!("'{vertex_ident}' cannot have both outgoing edges and ports marked as output.");
        }
    }

    // Check that each vertex that has a do-while edge also has a break edge and vice versa.
    let do_while_edge_vertices = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::DoWhile)
        .map(|edge| edge.from.vertex.clone())
        .collect::<HashSet<_>>();

    let break_edge_vertices = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Break)
        .map(|edge| edge.from.vertex.clone())
        .collect::<HashSet<_>>();

    for vertex in &do_while_edge_vertices {
        if !break_edge_vertices.contains(vertex) {
            bail!("Vertex '{vertex}' has a do-while edge but no corresponding break edge.");
        }
    }

    for vertex in &break_edge_vertices {
        if !do_while_edge_vertices.contains(vertex) {
            bail!("Vertex '{vertex}' has a break edge but no corresponding do-while edge.");
        }
    }

    // Check that do-whiles and breaks always branch.
    let do_while_variants = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::DoWhile)
        .map(|edge| (edge.from.vertex.clone(), edge.from.output_variant.clone()))
        .collect::<HashSet<_>>();

    let break_variants = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Break)
        .map(|edge| (edge.from.vertex.clone(), edge.from.output_variant.clone()))
        .collect::<HashSet<_>>();

    if let Some(variant) = do_while_variants.intersection(&break_variants).next() {
        let node = GraphNode::OutputVariant {
            vertex: variant.0.clone(),
            name: variant.1.clone(),
        };

        bail!("'{node}' has both a do-while and a break edge, but they must branch.");
    }

    Ok((graph, vertex_entry_groups))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::{Dag, FromPort},
        assert_matches::assert_matches,
    };

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

    #[test]
    fn both_loops_valid() {
        let dag: Dag = serde_json::from_str(include_str!("_dags/both_loops_valid.json")).unwrap();

        assert!(validate(dag).is_ok());
    }

    #[test]
    fn missing_do_while_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/missing_do_while_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Vertex 'until_11' has a break edge but no corresponding do-while edge."));
    }

    #[test]
    fn missing_break_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/missing_break_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Vertex 'until_11' has a do-while edge but no corresponding break edge."));
    }

    #[test]
    fn collect_without_for_each_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/collect_without_for_each_invalid.json"))
                .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output port: a.ok.result' has a collect edge without for-each"));
    }

    #[test]
    fn nested_for_each_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/nested_for_each_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output port: b.ok.result' has a nested for-each"));
    }

    #[test]
    fn do_while_in_for_each_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/do_while_in_for_each_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output port: b.also_ok.loop' has a do-while or break edge inside a for-each"));
    }

    #[test]
    fn unclosed_for_each_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/unclosed_for_each_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: b' has a for-each edge without a corresponding collect"));
    }

    #[test]
    fn do_while_and_break_same_variant_invalid() {
        let dag: Dag = serde_json::from_str(include_str!(
            "_dags/do_while_and_break_same_variant_invalid.json"
        ))
        .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output variant: b.ok' has both a do-while and a break edge, but they must branch."));
    }

    #[test]
    fn do_while_nested_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/do_while_nested_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Do-while edge at 'Output port: c.also_ok.loop' has a nested loop inside."));
    }

    // == Cyclic or no input graphs ==

    #[test]
    fn test_cyclic_invalid() {
        let dag: Dag = serde_json::from_str(include_str!("_dags/cyclic_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: a.input' has an edge leading to it and therefore cannot be an entry port."));
    }

    // == Parser tests ==

    #[test]
    fn test_undefined_connections_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/undefined_connections_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: a' is not connected to the DAG."));
    }

    #[test]
    fn test_encrypted_port_output_valid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/encrypted_port_output_valid.json")).unwrap();

        assert!(dag.edges.first().unwrap().from.encrypted);
        assert_eq!(
            *dag.outputs.unwrap().first().unwrap(),
            FromPort {
                vertex: "b".to_string(),
                output_variant: "1".to_string(),
                output_port: "1.0".to_string(),
                encrypted: true,
            }
        )
    }

    #[test]
    fn test_output_and_edge_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/output_and_edge_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: a' cannot have both outgoing edges and ports marked as output."));
    }

    #[test]
    fn test_def_val_on_input_port_invalid() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/has_def_on_input_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: location_decider.context' is an entry port or has an edge leading into it and therefore cannot have a default value."));
    }

    #[test]
    fn test_references_non_existing_vertex() {
        let dag: Dag =
            serde_json::from_str(include_str!("_dags/references_non_existing_vertex.json"))
                .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Entry group 'group_a' references a non-existing vertex 'invalid'."));
    }

    #[test]
    fn test_empty_invalid() {
        let dag: Dag = serde_json::from_str(include_str!("_dags/empty_invalid.json")).unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("The DAG has no entry vertices or ports."));
    }

    #[test]
    fn test_both_vertex_and_entry_vertex_invalid() {
        let dag: Dag = serde_json::from_str(include_str!(
            "_dags/both_vertex_and_entry_vertex_invalid.json"
        ))
        .unwrap();

        let res = validate(dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: b' is defined multiple times."));
    }
}
