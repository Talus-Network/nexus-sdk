use {
    crate::prelude::*,
    nexus_sdk::types::{Dag, DEFAULT_ENTRY_GROUP},
    petgraph::graph::{DiGraph, NodeIndex},
    std::collections::{HashMap, HashSet},
};

/// Validate function takes a DAG and validates it based on nexus execution
/// rules.
///
/// See our wiki for more information on the rules:
/// <https://talus-labs.gitbook.io/talus-documentation/devs/index/workflow#rules>
/// <https://talus-labs.gitbook.io/talus-documentation/devs/index-1/cli#nexus-dag>
pub(crate) fn validate(dag: Dag) -> AnyResult<()> {
    // Parse the dag into a petgraph DiGraph.
    let graph: DiGraph<GraphNode, ()> = try_into_graph(dag)?;

    if !graph.is_directed() || petgraph::algo::is_cyclic_directed(&graph) {
        bail!("The provided graph contains one or more cycles.");
    }

    // Check that the shape of the graph is correct.
    has_correct_order_of_actions(&graph)?;

    // Check that no walks in the graph violate the concurrency rules.
    follows_concurrency_rules(&graph)?;

    Ok(())
}

fn has_correct_order_of_actions(graph: &DiGraph<GraphNode, ()>) -> AnyResult<()> {
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
fn follows_concurrency_rules(graph: &DiGraph<GraphNode, ()>) -> AnyResult<()> {
    // Get all distinct groups of entry vertices.
    let groups = graph
        .node_indices()
        .flat_map(|node| match &graph[node] {
            GraphNode::Vertex { groups, .. } => groups.clone().unwrap_or_default(),
            _ => vec![],
        })
        .collect::<HashSet<_>>();

    // For each group...
    for group in groups {
        // ... find the entry vertices in that group.
        let entry_vertices = graph
            .node_indices()
            .filter(|&node| match &graph[node] {
                GraphNode::Vertex {
                    groups: Some(groups),
                    ..
                } => groups.contains(&group),
                _ => false,
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
    graph: &DiGraph<GraphNode, ()>,
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
            // Input ports with no default values reduce concurrency.
            GraphNode::InputPort { .. } => acc - 1,
            _ => acc,
        }
    });

    // If the net concurrency is 0, the graph follows the concurrency rules.
    net_concurrency
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum GraphNode {
    InputPort {
        vertex: String,
        name: String,
    },
    Vertex {
        name: String,
        /// `groups` is:
        /// - `Some(vec) if !vec.is_empty()` when it's an entry vertex
        /// - `Some(vec) if vec.is_empty()` is unreachable as default group is
        ///   provided in case one is missing
        /// - `None` when it's a normal vertex
        groups: Option<Vec<String>>,
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
            GraphNode::Vertex { name, .. } => write!(f, "Vertex: {name}"),
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
fn try_into_graph(dag: Dag) -> AnyResult<DiGraph<GraphNode, ()>> {
    let mut graph = DiGraph::<GraphNode, ()>::new();

    // Check that there is at least one entry vertex.
    if dag.entry_vertices.is_empty() {
        bail!("The DAG has no entry vertices.");
    }

    // For each entry vertex, find a set of entry groups it belongs to.
    let mut vertex_entry_groups: HashMap<String, Vec<String>> = HashMap::new();

    for entry_vertex in &dag.entry_vertices {
        match &dag.entry_groups {
            None => {
                vertex_entry_groups.insert(
                    entry_vertex.name.clone(),
                    vec![DEFAULT_ENTRY_GROUP.to_string()],
                );
            }
            Some(entry_groups) => {
                let entry_groups = entry_groups
                    .iter()
                    .filter(|entry_group| entry_group.vertices.contains(&entry_vertex.name))
                    .map(|entry_group| entry_group.name.clone())
                    .collect::<Vec<String>>();

                // If the entry vertex does not belong to any group, add it
                // to the default group.
                match entry_groups.len() {
                    0 => {
                        vertex_entry_groups.insert(
                            entry_vertex.name.clone(),
                            vec![DEFAULT_ENTRY_GROUP.to_string()],
                        );
                    }
                    _ => {
                        vertex_entry_groups.insert(entry_vertex.name.clone(), entry_groups);
                    }
                }
            }
        }
    }

    // Check that entry groups only reference entry vertices and that entry
    // group names don't use the reserved keyword.
    let entry_groups = dag.entry_groups.unwrap_or_default();

    for entry_group in &entry_groups {
        if entry_group.name == DEFAULT_ENTRY_GROUP {
            bail!(
                "Entry group '{}' uses a reserved keyword.",
                DEFAULT_ENTRY_GROUP
            );
        }

        for vertex in &entry_group.vertices {
            if !dag
                .entry_vertices
                .iter()
                .any(|entry_vertex| entry_vertex.name == *vertex)
            {
                bail!(
                    "Entry group '{}' references a non-entry 'Vertex: {vertex}'.",
                    entry_group.name
                );
            }
        }
    }

    // Edges are always between an output port and an input port. We also
    // need to create edges between the tool, the output variant and the
    // output port if they don't exist yet.
    let mut graph_nodes: HashMap<GraphNode, NodeIndex> = HashMap::new();

    for edge in dag.edges {
        let origin_vertex = GraphNode::Vertex {
            name: edge.from.vertex.clone(),
            groups: vertex_entry_groups.get(&edge.from.vertex).cloned(),
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

        let destination_vertex = GraphNode::Vertex {
            name: edge.to.vertex.clone(),
            groups: vertex_entry_groups.get(&edge.to.vertex).cloned(),
        };

        let input_port = GraphNode::InputPort {
            vertex: edge.to.vertex.clone(),
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
            graph.add_edge(origin_node, output_variant_node, ());
        }

        if !graph.contains_edge(input_port_node, destination_node) {
            graph.add_edge(input_port_node, destination_node, ());
        }

        graph.add_edge(output_variant_node, output_port_node, ());
        graph.add_edge(output_port_node, input_port_node, ());
    }

    // Ensure we don't have duplicate vertices.
    let mut all_entry_vertices = HashSet::new();
    let mut all_vertices = HashSet::new();
    let mut all_entry_input_ports = HashSet::new();

    // Check that all entry vertices are in the graph. Note that connecting
    // entry input ports to these entry vertices is not necessary as they do
    // not matter for the validation.
    for entry_vertex in &dag.entry_vertices {
        let entry_vertex_ident = GraphNode::Vertex {
            name: entry_vertex.name.clone(),
            groups: vertex_entry_groups.get(&entry_vertex.name).cloned(),
        };

        if !graph_nodes.contains_key(&entry_vertex_ident) {
            bail!("Entry '{entry_vertex_ident}' is not connected to the DAG.",);
        }

        if !all_entry_vertices.insert(entry_vertex_ident.clone()) {
            bail!("Entry '{entry_vertex_ident}' is defined multiple times.",);
        }

        // Add entry input ports to the map so we can check that they do not
        // have a default value.
        for input_port in &entry_vertex.input_ports {
            let input_port_ident = GraphNode::InputPort {
                vertex: entry_vertex.name.clone(),
                name: input_port.clone(),
            };

            if !all_entry_input_ports.insert(input_port_ident.clone()) {
                bail!("Entry '{input_port_ident}' is defined multiple times.",);
            }
        }
    }

    // Check that all normal vertices are in the graph.
    for vertex in &dag.vertices {
        let vertex_ident = GraphNode::Vertex {
            name: vertex.name.clone(),
            groups: vertex_entry_groups.get(&vertex.name).cloned(),
        };

        if !graph_nodes.contains_key(&vertex_ident) {
            bail!("'{vertex_ident}' is not connected to the DAG.",);
        }

        if !all_vertices.insert(vertex_ident.clone()) {
            bail!("'{vertex_ident}' is defined multiple times.",);
        }
    }

    // Ensure vertex is not specified as a vertex and an entry vertex.
    if let Some(vertex) = all_vertices.intersection(&all_entry_vertices).next() {
        bail!("'{vertex}' is both a vertex and an entry vertex.")
    }

    // Check that none of the default value input ports are in the graph.
    let default_values = dag.default_values.unwrap_or_default();

    for default_value in default_values {
        let default_value = GraphNode::InputPort {
            vertex: default_value.vertex,
            name: default_value.input_port,
        };

        if graph_nodes.contains_key(&default_value)
            || all_entry_input_ports.contains(&default_value)
        {
            bail!(
                    "'{default_value}' is already present in the graph or has an edge leading into it and therefore cannot have a default value.",
                );
        }
    }

    Ok(graph)
}
