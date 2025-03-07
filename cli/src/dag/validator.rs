use {
    crate::{
        dag::parser::{Dag, DefaultValue, FromPort, ToPort, Vertex},
        prelude::*,
    },
    petgraph::graph::{DiGraph, NodeIndex},
    std::collections::{HashMap, HashSet},
};

/// Validate function takes a graph and validates it based on nexus execution
/// rules.
///
/// See our wiki for more information on the rules:
/// <https://github.com/Talus-Network/nexus-next/wiki/Package:-Workflow#rules>
/// <https://github.com/Talus-Network/nexus-next/wiki/CLI#nexus-dag>
pub(crate) fn validate(graph: &DiGraph<NodeIdent, ()>) -> AnyResult<()> {
    if !graph.is_directed() || petgraph::algo::is_cyclic_directed(graph) {
        bail!("The provided graph contains one or more cycles.");
    }

    // Check that the shape of the graph is correct.
    has_correct_order_of_actions(graph)?;

    // Check that no walks in the graph violate the concurrency rules.
    if !follows_concurrency_rules(graph) {
        bail!("Graph does not follow concurrency rules.");
    }

    Ok(())
}

fn has_correct_order_of_actions(graph: &DiGraph<NodeIdent, ()>) -> AnyResult<()> {
    for node in graph.node_indices() {
        let vertex = &graph[node];
        let neighbors = graph
            .neighbors_directed(node, petgraph::Direction::Outgoing)
            .collect::<Vec<NodeIndex>>();

        // Check if the vertex has the correct number of edges.
        match vertex.kind {
            // Input ports must have exactly 1 outgoing edge.
            VertexKind::InputPort if neighbors.len() != 1 => {
                bail!("'{vertex}' must have exactly 1 outgoing edge")
            }
            // Tools can be the last vertex and can have any number of edges.
            VertexKind::Tool => (),
            // Output variants must have at least 1 outgoing edge.
            VertexKind::OutputVariant if neighbors.is_empty() => {
                bail!("'{vertex}' must have at least 1 outgoing edge")
            }
            // Output ports must have exactly 1 outgoing edge.
            VertexKind::OutputPort if neighbors.len() != 1 => {
                bail!("'{vertex}' must have exactly 1 outgoing edge")
            }
            _ => (),
        };

        // Check if the edges are connected in the correct order.
        for node in neighbors {
            let neighbor = graph[node].clone();

            let is_ok = match vertex.kind {
                VertexKind::InputPort => neighbor.kind == VertexKind::Tool,
                VertexKind::Tool => neighbor.kind == VertexKind::OutputVariant,
                VertexKind::OutputVariant => neighbor.kind == VertexKind::OutputPort,
                VertexKind::OutputPort => neighbor.kind == VertexKind::InputPort,
            };

            if !is_ok {
                bail!("The edge from '{vertex}' to '{neighbor}' is invalid.");
            }
        }
    }

    Ok(())
}

fn follows_concurrency_rules(graph: &DiGraph<NodeIdent, ()>) -> bool {
    // For each input port, check that the net concurrency leading into that
    // node is 1.
    graph
        .node_indices()
        .filter(|&node| graph[node].kind == VertexKind::InputPort)
        .all(|node| {
            // TODO: this needs to be limited to an entry group.
            //
            // Find all nodes that are included in the paths leading to the merge point.
            let all_nodes_in_paths = find_all_nodes_in_paths_to(graph, node);

            // Note that if this fails we can debug which node is causing the issues. If the concurrency is negative on
            // any given node, it means it's unreachable.
            check_concurrency_in_subgraph(graph, &all_nodes_in_paths)
        })
}

fn check_concurrency_in_subgraph(
    graph: &DiGraph<NodeIdent, ()>,
    nodes: &HashSet<NodeIndex>,
) -> bool {
    let net_concurrency = nodes.iter().fold(0, |acc, &node| {
        match graph[node].kind {
            VertexKind::Tool => {
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
            VertexKind::InputPort => acc - 1,
            _ => acc,
        }
    });

    // If the net concurrency is 1, the graph follows the concurrency rules.
    net_concurrency == 1
}

fn find_all_nodes_in_paths_to(
    graph: &DiGraph<NodeIdent, ()>,
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

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum VertexKind {
    InputPort,
    Tool,
    OutputVariant,
    OutputPort,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct NodeIdent {
    kind: VertexKind,
    vertex: (Option<String>, Option<String>, Option<String>),
}

impl From<FromPort> for (NodeIdent, NodeIdent, NodeIdent) {
    fn from(from_port: FromPort) -> Self {
        let vertex = Some(from_port.vertex);
        let variant = Some(from_port.output_variant);
        let port = Some(from_port.output_port);

        let tool = (vertex.clone(), None, None);
        let output_variant = (vertex.clone(), variant.clone(), None);
        let output_port = (vertex, variant, port);

        (
            NodeIdent {
                kind: VertexKind::Tool,
                vertex: tool,
            },
            NodeIdent {
                kind: VertexKind::OutputVariant,
                vertex: output_variant,
            },
            NodeIdent {
                kind: VertexKind::OutputPort,
                vertex: output_port,
            },
        )
    }
}

impl From<ToPort> for (NodeIdent, NodeIdent) {
    fn from(to_port: ToPort) -> Self {
        let vertex = Some(to_port.vertex);

        let tool = (vertex.clone(), None, None);
        let input_port = (vertex, None, Some(to_port.input_port));

        (
            NodeIdent {
                kind: VertexKind::Tool,
                vertex: tool,
            },
            NodeIdent {
                kind: VertexKind::InputPort,
                vertex: input_port,
            },
        )
    }
}

impl From<Vertex> for NodeIdent {
    fn from(vertex: Vertex) -> Self {
        let vertex = (Some(vertex.name), None, None);

        Self {
            kind: VertexKind::Tool,
            vertex,
        }
    }
}

impl From<(Option<String>, Option<String>, Option<String>)> for NodeIdent {
    fn from(vertex: (Option<String>, Option<String>, Option<String>)) -> Self {
        match vertex {
            (Some(_), None, None) => Self {
                kind: VertexKind::Tool,
                vertex,
            },
            (Some(_), Some(_), None) => Self {
                kind: VertexKind::OutputVariant,
                vertex,
            },
            (Some(_), Some(_), Some(_)) => Self {
                kind: VertexKind::OutputPort,
                vertex,
            },
            (Some(_), None, Some(_)) => Self {
                kind: VertexKind::InputPort,
                vertex,
            },
            _ => unreachable!(),
        }
    }
}

impl From<DefaultValue> for NodeIdent {
    fn from(default_value: DefaultValue) -> Self {
        let vertex = (
            Some(default_value.vertex),
            None,
            Some(default_value.input_port),
        );

        Self {
            kind: VertexKind::InputPort,
            vertex,
        }
    }
}

impl std::fmt::Display for NodeIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.vertex {
            (Some(vertex), None, None) => write!(f, "Vertex: {}", vertex),
            (Some(vertex), Some(variant), None) => {
                write!(f, "Output variant: {}.{}", vertex, variant)
            }
            (Some(vertex), Some(variant), Some(port)) => {
                write!(f, "Output port: {}.{}.{}", vertex, variant, port)
            }
            (Some(vertex), None, Some(port)) => write!(f, "Input port: {}.{}", vertex, port),
            _ => unreachable!(),
        }
    }
}

/// [Dag] to [petgraph::graph::DiGraph].
impl TryFrom<Dag> for DiGraph<NodeIdent, ()> {
    type Error = AnyError;

    fn try_from(dag: Dag) -> AnyResult<Self> {
        let mut graph = DiGraph::<NodeIdent, ()>::new();

        // Edges are always between an output port and an input port. We also
        // need to create edges between the tool, the output variant and the
        // output port if they don't exist yet.
        let mut node_idents: HashMap<NodeIdent, NodeIndex> = HashMap::new();

        for edge in dag.edges {
            // Create unique keys for each node in this edge.
            let (origin_vertex, output_variant, output_port) = edge.from.into();
            let (destination_vertex, input_port) = edge.to.into();

            // Create nodes if they don't exist yet.
            let origin_node = node_idents.get(&origin_vertex).copied().unwrap_or_else(|| {
                let node = graph.add_node(origin_vertex.clone());

                node_idents.insert(origin_vertex.clone(), node);

                node
            });

            let output_variant_node =
                node_idents
                    .get(&output_variant)
                    .copied()
                    .unwrap_or_else(|| {
                        let node = graph.add_node(output_variant.clone());

                        node_idents.insert(output_variant.clone(), node);

                        node
                    });

            let output_port_node = node_idents.get(&output_port).copied().unwrap_or_else(|| {
                let node = graph.add_node(output_port.clone());

                node_idents.insert(output_port.clone(), node);

                node
            });

            let destination_node = node_idents
                .get(&destination_vertex)
                .copied()
                .unwrap_or_else(|| {
                    let node = graph.add_node(destination_vertex.clone());

                    node_idents.insert(destination_vertex.clone(), node);

                    node
                });

            let input_port_node = node_idents.get(&input_port).copied().unwrap_or_else(|| {
                let node = graph.add_node(input_port.clone());

                node_idents.insert(input_port.clone(), node);

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

        // Check that there is at least one entry vertex.
        if dag.entry_vertices.is_empty() {
            bail!("The DAG has no entry vertices.");
        }

        // Ensure we don't have duplicate vertices.
        let mut all_entry_vertices = HashSet::new();
        let mut all_vertices = HashSet::new();
        let mut all_entry_input_ports = HashSet::new();

        // Check that all entry vertices are in the graph. Note that connecting
        // entry input ports to these entry vertices is not necessary as they do
        // not matter for the validation.
        for entry_vertex in &dag.entry_vertices {
            let entry_vertex_ident = (Some(entry_vertex.name.clone()), None, None).into();

            if !node_idents.contains_key(&entry_vertex_ident) {
                bail!("Entry '{entry_vertex_ident}' is not connected to the DAG.",);
            }

            if !all_entry_vertices.insert(entry_vertex_ident.clone()) {
                bail!("Entry '{entry_vertex_ident}' is a duplicate vertex.",);
            }

            // Add entry input ports to the map so we can check that they do not
            // have a default value.
            for input_port in &entry_vertex.input_ports {
                let input_port_ident: NodeIdent = (
                    Some(entry_vertex.name.clone()),
                    None,
                    Some(input_port.clone()),
                )
                    .into();

                if !all_entry_input_ports.insert(input_port_ident.clone()) {
                    bail!("Entry '{input_port_ident}' is defined multiple times.",);
                }
            }
        }

        // Check that all normal vertices are in the graph.
        for vertex in &dag.vertices {
            let vertex_ident = vertex.clone().into();

            if !node_idents.contains_key(&vertex_ident) {
                bail!("'{vertex_ident}' is not connected to the DAG.",);
            }

            if !all_vertices.insert(vertex_ident.clone()) {
                bail!("'{vertex_ident}' is a duplicate vertex.",);
            }
        }

        // Ensure vertex is not specified as a vertex and an entry vertex.
        match all_vertices.intersection(&all_entry_vertices).next() {
            Some(vertex) => bail!("{vertex} is both a vertex and an entry vertex."),
            None => (),
        }

        // Check that all entry groups reference vertices that are entry vertices.
        let entry_groups = dag.entry_groups.unwrap_or_default();

        for entry_group in &entry_groups {
            for vertex in &entry_group.vertices {
                let vertex_ident = (Some(vertex.clone()), None, None).into();
                let entry_group_name = &entry_group.name;

                if !all_entry_vertices.contains(&vertex_ident) {
                    bail!(
                        "'{vertex_ident}' is not an entry vertex but is referenced in the '{entry_group_name}' entry group.",
                    );
                }
            }
        }

        // Check that none of the default value input ports are in the graph.
        let default_values = dag.default_values.unwrap_or_default();

        for default_value in default_values {
            let default_value = default_value.into();

            if node_idents.contains_key(&default_value)
                || all_entry_input_ports.contains(&default_value)
            {
                bail!(
                    "'{default_value}' is already present in the graph or has an edge leading into it and therefore cannot have a default value.",
                );
            }
        }

        Ok(graph)
    }
}
