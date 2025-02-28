use {
    super::parser::Dag,
    crate::prelude::*,
    petgraph::graph::{DiGraph, NodeIndex},
    std::collections::{HashMap, HashSet},
};

/// Validate function takes a graph and validates it based on nexus execution rules.
///
/// 1. The graph has to be a directed acyclic graph.
/// 2. There has to be at least 1 [entry vertex].
/// 3. [Default values] can only be set on input ports and such input ports must not have any other incoming edges.
/// 4. Graph must follow order of actions: input port N -> 1 tool 1 -> N output variant 1 -> N output port N -> 1 input port
/// 5. Calculate maximum possible spawned concurrent tasks and then subtract the number of concurrent tasks we consume.
///    The result must be equal to 0.
///    - A concurrent task is spawned each time there is more than 1 output port per output variant. We need to, however,
///      consider that a tool can only execute 1 output variant, so we take the max number of output ports per output
///      variant per tool, and then we sum all of these up.
///    - A concurrent task is consumed when there is more than 1 input port per tool. This does not, however, include
///      input ports with default values.
pub(crate) fn validate(graph: &DiGraph<VertexType, ()>) -> AnyResult<()> {
    // 1.
    if !graph.is_directed() || petgraph::algo::is_cyclic_directed(graph) {
        bail!("The provided graph is not a DAG.");
    }

    // 2.
    let entry_vertices = find_entry_vertices(graph);

    if entry_vertices.is_empty() {
        bail!("The DAG has no entry vertices.");
    }

    // 3.
    if has_edges_to_input_ports_with_defaults(graph) {
        bail!("Input ports with default values must not have any incoming edges.");
    }

    // 4.
    if !has_correct_order_of_actions(graph) {
        bail!("Graph must follow the order of actions: input port N -> 1 tool 1 -> N output variant 1 -> N output port N -> 1 input port");
    }

    // 5.
    if !follows_concurrency_rules(graph, entry_vertices) {
        bail!("Graph does not follow concurrency rules.");
    }

    Ok(())
}

fn find_entry_vertices(graph: &DiGraph<VertexType, ()>) -> Vec<NodeIndex> {
    graph
        .node_indices()
        .filter(|&node| {
            graph[node] == VertexType::InputPort
                && graph.neighbors_directed(node, petgraph::Incoming).count() == 0
        })
        .collect()
}

fn has_edges_to_input_ports_with_defaults(graph: &DiGraph<VertexType, ()>) -> bool {
    graph
        .node_indices()
        .filter(|&node| graph[node] == VertexType::InputPortWithDefault)
        .any(|node| graph.neighbors_directed(node, petgraph::Incoming).count() > 0)
}

fn has_correct_order_of_actions(graph: &DiGraph<VertexType, ()>) -> bool {
    graph.node_indices().all(|node| {
        let vertex = &graph[node];
        let neighbors = graph
            .neighbors_directed(node, petgraph::Direction::Outgoing)
            .collect::<Vec<NodeIndex>>();

        // Check if the vertex has the correct number of edges.
        match vertex {
            // Input ports must have exactly 1 outgoing edge.
            VertexType::InputPort if neighbors.len() != 1 => return false,
            VertexType::InputPortWithDefault if neighbors.len() != 1 => return false,
            // Tools can be the last vertex and can have any number of edges.
            VertexType::Tool => (),
            // Output variants must have at least 1 outgoing edge.
            VertexType::OutputVariant if neighbors.is_empty() => return false,
            // Output ports must have exactly 1 outgoing edge.
            VertexType::OutputPort if neighbors.len() != 1 => return false,
            _ => (),
        };

        // Check if the edges are connected in the correct order.
        neighbors.iter().all(|&node| {
            let neighbor = graph[node];

            match vertex {
                VertexType::InputPort => neighbor == VertexType::Tool,
                VertexType::InputPortWithDefault => neighbor == VertexType::Tool,
                VertexType::Tool => neighbor == VertexType::OutputVariant,
                VertexType::OutputVariant => neighbor == VertexType::OutputPort,
                VertexType::OutputPort => neighbor == VertexType::InputPort,
            }
        })
    })
}

fn follows_concurrency_rules(
    graph: &DiGraph<VertexType, ()>,
    entry_vertices: Vec<NodeIndex>,
) -> bool {
    // For each merge point on an input port, check that the net concurrency leading into that point is 0.
    graph
        .node_indices()
        .filter(|&node| {
            graph[node] == VertexType::InputPort
                && graph.neighbors_directed(node, petgraph::Incoming).count() > 1
        })
        .all(|node| {
            // Find all nodes that are included in the paths leading to the merge point.
            let all_nodes_in_paths = find_all_nodes_in_paths_to(graph, node);

            // Find all entry vertices that are included in the paths leading to the merge point. This way we can find
            // our initial concurrency.
            let included_entry_vertices = entry_vertices
                .iter()
                .filter(|entry| all_nodes_in_paths.contains(entry))
                .count();

            // Note that if this fails we can debug which node is causing the issues. If the concurrency is negative on
            // any given node, it means it's unreachable.
            check_concurrency_in_subgraph(graph, &all_nodes_in_paths, included_entry_vertices)
        })
}

fn check_concurrency_in_subgraph(
    graph: &DiGraph<VertexType, ()>,
    nodes: &HashSet<NodeIndex>,
    entry_vertices_count: usize,
) -> bool {
    // Initial concurrency is just the number of entry vertices minus the number of goal "tool" vertices.
    let initial_concurrency = entry_vertices_count as isize - 1;

    let net_concurrency = nodes.iter().fold(initial_concurrency, |acc, &node| {
        match graph[node] {
            VertexType::Tool => {
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
            VertexType::InputPort => acc - 1,
            _ => acc,
        }
    });

    // If the net concurrency is 0, the graph follows the concurrency rules.
    net_concurrency == 0
}

fn find_all_nodes_in_paths_to(
    graph: &DiGraph<VertexType, ()>,
    end: NodeIndex,
) -> HashSet<NodeIndex> {
    let mut visited = HashSet::new();
    let mut stack = graph
        .neighbors_directed(end, petgraph::Incoming)
        .collect::<Vec<NodeIndex>>();

    while let Some(node) = stack.pop() {
        // Skip already visited nodes.
        if !visited.insert(node) {
            continue;
        }

        for neighbor in graph.neighbors_directed(node, petgraph::Incoming) {
            if !visited.contains(&neighbor) {
                stack.push(neighbor);
            }
        }
    }

    visited
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum VertexType {
    InputPort,
    InputPortWithDefault,
    Tool,
    OutputVariant,
    OutputPort,
}

/// [Dag] to [petgraph::graph::DiGraph].
impl TryFrom<Dag> for DiGraph<VertexType, ()> {
    type Error = AnyError;

    fn try_from(dag: Dag) -> AnyResult<Self> {
        let mut graph = DiGraph::<VertexType, ()>::new();

        // Find default values for input ports. This way we differentiate
        // between input ports with default values and input ports without
        // default values.
        let mut default_values = Vec::new();
        let dag_default_values = dag.default_values.unwrap_or_default();

        for default_value in &dag_default_values {
            let input_port_node = [
                default_value.vertex.as_str(),
                "",
                default_value.input_port.as_str(),
            ];

            default_values.push(input_port_node);
        }

        // Edges are always between an output port and an input port. We also
        // need to create edges between the tool, the output variant and the
        // output port if they don't exist yet.
        let mut nodes_named: HashMap<[&str; 3], NodeIndex> = HashMap::new();

        for edge in &dag.edges {
            // Create unique keys for each node in this edge.
            let origin_vertex = [edge.from.vertex.as_str(), "", ""];
            let output_variant = [
                edge.from.vertex.as_str(),
                edge.from.output_variant.as_str(),
                "",
            ];
            let output_port = [
                edge.from.vertex.as_str(),
                edge.from.output_variant.as_str(),
                edge.from.output_port.as_str(),
            ];
            let destination_vertex = [edge.to.vertex.as_str(), "", ""];
            let input_port = [edge.to.vertex.as_str(), "", edge.to.input_port.as_str()];

            // Create nodes if they don't exist yet.
            let origin_node = nodes_named.get(&origin_vertex).copied().unwrap_or_else(|| {
                let node = graph.add_node(VertexType::Tool);

                nodes_named.insert(origin_vertex, node);

                node
            });

            let output_variant_node =
                nodes_named
                    .get(&output_variant)
                    .copied()
                    .unwrap_or_else(|| {
                        let node = graph.add_node(VertexType::OutputVariant);

                        nodes_named.insert(output_variant, node);

                        node
                    });

            let output_port_node = nodes_named.get(&output_port).copied().unwrap_or_else(|| {
                let node = graph.add_node(VertexType::OutputPort);

                nodes_named.insert(output_port, node);

                node
            });

            let destination_node = nodes_named
                .get(&destination_vertex)
                .copied()
                .unwrap_or_else(|| {
                    let node = graph.add_node(VertexType::Tool);

                    nodes_named.insert(destination_vertex, node);

                    node
                });

            let input_port_node = nodes_named.get(&input_port).copied().unwrap_or_else(|| {
                let node = graph.add_node(match default_values.contains(&input_port) {
                    true => VertexType::InputPortWithDefault,
                    false => VertexType::InputPort,
                });

                nodes_named.insert(input_port, node);

                node
            });

            // Create edges between the nodes if they don't exist yet.
            if !graph.contains_edge(origin_node, output_variant_node) {
                graph.add_edge(origin_node, output_variant_node, ());
            }

            if !graph.contains_edge(output_variant_node, output_port_node) {
                graph.add_edge(output_variant_node, output_port_node, ());
            }

            if !graph.contains_edge(output_port_node, input_port_node) {
                graph.add_edge(output_port_node, input_port_node, ());
            }

            if !graph.contains_edge(input_port_node, destination_node) {
                graph.add_edge(input_port_node, destination_node, ());
            }
        }

        // Create edges between the entry vertices and their input ports.
        for entry_vertex in &dag.entry_vertices {
            // Note that we don't need to insert the nodes as they must exist
            // at this point.
            let entry_node = nodes_named
                .get(&[entry_vertex.name.as_str(), "", ""])
                .copied()
                .ok_or_else(|| {
                    anyhow!(
                        "Entry vertex '{}' does not exist in the graph.",
                        entry_vertex.name
                    )
                })?;

            for input_port in &entry_vertex.input_ports {
                let input_port = [entry_vertex.name.as_str(), "", input_port.as_str()];

                // Opposite of the tool node, the input ports must not exist
                // at this time.
                let input_port_node = graph.add_node(match default_values.contains(&input_port) {
                    true => VertexType::InputPortWithDefault,
                    false => VertexType::InputPort,
                });

                graph.add_edge(input_port_node, entry_node, ());
            }
        }

        Ok(graph)
    }
}
