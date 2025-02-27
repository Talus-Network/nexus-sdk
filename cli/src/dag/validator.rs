use {
    crate::{dag::parser::VertexType, prelude::*},
    petgraph::graph::{DiGraph, NodeIndex},
    std::collections::HashSet,
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
            // Tools must have at least 1 outgoing edge.
            VertexType::Tool if neighbors.is_empty() => return false,
            // Output variants can be the last vertex and can have any number of edges
            VertexType::OutputVariant => (),
            // Output ports must have 0 or 1 outgoing edges.
            VertexType::OutputPort if neighbors.len() > 1 => return false,
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

    let net_concurrency = nodes.into_iter().fold(initial_concurrency, |acc, &node| {
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
