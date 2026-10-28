use {
    crate::{
        move_bindings::interface::graph::EdgeKind,
        types::{DagSpec, DEFAULT_ENTRY_GROUP},
    },
    anyhow::{bail, Result as AnyResult},
    petgraph::{
        graph::{DiGraph, NodeIndex},
        visit::{EdgeRef, NodeIndexable, VisitMap, Visitable},
    },
    std::collections::{HashMap, HashSet},
};

type GraphAndVertexEntryGroups = (
    DiGraph<GraphNode, EdgeKind>,
    HashMap<GraphNode, Vec<String>>,
);

// bitmap max size.
const REACHABILITY_MASK_BUDGET_BYTES: usize = 1024 * 1024;
// Use DiGraph's visit map as bitmap.
type ReachabilityMask = <DiGraph<GraphNode, EdgeKind> as Visitable>::Map;

// Index the immutable expanded graph once per validation and share traversal scratch.
struct ValidationAnalysis<'graph> {
    input_ports: Vec<NodeIndex>,
    // Entry payloads and their handles; None records a missing graph node.
    entry_nodes: Vec<(&'graph GraphNode, Option<NodeIndex>)>,
    // Distinct root sets paired with one representative group name.
    root_sets: Vec<(Vec<NodeIndex>, String)>,
    mask: ReachabilityMask,
    stack: Vec<NodeIndex>,
    #[cfg(test)]
    traversal_count: usize,
}

impl<'graph> ValidationAnalysis<'graph> {
    fn new(
        graph: &DiGraph<GraphNode, EdgeKind>,
        vertex_entry_groups: &'graph HashMap<GraphNode, Vec<String>>,
    ) -> Self {
        let mut roots_by_group = vertex_entry_groups
            .values()
            .flatten()
            .map(|group| (group.clone(), Vec::new()))
            .collect::<HashMap<String, Vec<NodeIndex>>>();
        let mut input_ports = Vec::new();
        // GraphNode payload -> NodeIndex handle.
        let mut node_by_graph_node = HashMap::with_capacity(graph.node_count());
        for node in graph.node_indices() {
            // Despite its name, vertex may hold a tool, variant, or port payload.
            let vertex = &graph[node];
            node_by_graph_node.insert(vertex, node);
            if matches!(vertex, GraphNode::InputPort { .. }) {
                input_ports.push(node);
            }
            if let Some(groups) = vertex_entry_groups.get(vertex) {
                for group in groups {
                    roots_by_group.entry(group.clone()).or_default().push(node);
                }
            }
        }
        let entry_nodes = vertex_entry_groups
            .keys()
            .map(|vertex| (vertex, node_by_graph_node.get(&vertex).copied()))
            .collect();

        // A root sets produce identical path regions and concurrency scores.
        // Normalize membership and store one original group name for diagnostics.
        let mut grouped_roots = HashMap::<Vec<NodeIndex>, String>::new();
        for (group, mut nodes) in roots_by_group {
            nodes.sort_unstable();
            nodes.dedup();
            grouped_roots.entry(nodes).or_insert(group);
        }
        let root_sets = grouped_roots.into_iter().collect();

        Self {
            input_ports,
            entry_nodes,
            root_sets,
            mask: graph.visit_map(),
            stack: Vec::with_capacity(graph.node_bound()),
            #[cfg(test)]
            traversal_count: 0,
        }
    }

    // Each walk overwrites the shared bitmap. Callers clone it only when a result
    // must survive the next walk; the bitmap and stack allocations are reused.
    fn traverse(
        &mut self,
        graph: &DiGraph<GraphNode, EdgeKind>,
        starts: impl IntoIterator<Item = NodeIndex>,
        direction: petgraph::Direction,
    ) {
        #[cfg(test)]
        {
            self.traversal_count += 1;
        }
        self.mask.clear();
        self.stack.clear();
        for start in starts {
            if self.mask.visit(start) {
                self.stack.push(start);
            }
        }
        while let Some(node) = self.stack.pop() {
            for neighbor in graph.neighbors_directed(node, direction) {
                if self.mask.visit(neighbor) {
                    self.stack.push(neighbor);
                }
            }
        }
    }

    fn validate_concurrency_rules(
        &mut self,
        graph: &DiGraph<GraphNode, EdgeKind>,
        mask_budget_bytes: usize,
    ) -> AnyResult<()> {
        // Batch forward masks to avoid retaining a reachability map for every root set.
        let batch_width = root_set_batch_width(
            std::mem::size_of_val(self.mask.as_slice()),
            mask_budget_bytes,
        );
        let mut batch_start = 0;
        while batch_start < self.root_sets.len() {
            let batch_end = batch_start + batch_width.min(self.root_sets.len() - batch_start);
            let mut root_masks = Vec::with_capacity(batch_end - batch_start);
            for root_set_index in batch_start..batch_end {
                let roots = self.root_sets[root_set_index].0.clone();
                self.traverse(graph, roots, petgraph::Direction::Outgoing);
                root_masks.push(self.mask.clone());
            }

            // Compute each input's ancestors once per batch and share them across roots.
            // For U unique root sets, P inputs and batch width B, this takes
            // U + P * ceil(U / B) walks
            // The loop counter indexes input_ports; each stored value is a graph NodeIndex.
            for input_port_index in 0..self.input_ports.len() {
                let input_port = self.input_ports[input_port_index];
                self.traverse(graph, [input_port], petgraph::Direction::Incoming);
                for root_set_index in batch_start..batch_end {
                    let group = &self.root_sets[root_set_index].1;
                    let root_mask = &root_masks[root_set_index - batch_start];
                    // A node is on a root-to-input path exactly when a root can
                    // reach it and it can reach the input. Score that exact region.
                    let all_nodes_in_paths = root_mask
                        .intersection(&self.mask)
                        .map(NodeIndex::new)
                        .collect::<HashSet<_>>();
                    if all_nodes_in_paths.is_empty() {
                        bail!(
                            "'{}' is unreachable when invoking group '{group}'",
                            graph[input_port]
                        );
                    }
                    let concurrency = get_net_concurrency_in_subgraph(graph, &all_nodes_in_paths);
                    if concurrency < 0 {
                        bail!(
                            "'{}' is unreachable when invoking group '{group}'",
                            graph[input_port]
                        );
                    }
                    if concurrency > 0 {
                        bail!(
                            "'{}' has a race condition on it when invoking group '{group}'",
                            graph[input_port]
                        );
                    }
                }
            }
            batch_start = batch_end;
        }
        Ok(())
    }

    /// Check the for-each and collect edges are correctly paired and not nesting.
    fn validate_for_each_pairs(&self, graph: &DiGraph<GraphNode, EdgeKind>) -> AnyResult<()> {
        #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
        enum ForEachState {
            Idle,
            InForEach,
        }
        use ForEachState::*;

        let mut stack = Vec::new();
        // A node can be reached both inside and outside a ForEach. Plain reachability
        // cannot distinguish these cases, so visits must include the loop state.
        let mut seen = HashSet::new();
        for seed in &self.entry_nodes {
            let (vertex, node) = *seed;
            let node =
                node.ok_or_else(|| anyhow::anyhow!("'{}' not found in graph nodes.", vertex))?;
            if seen.insert((node, Idle)) {
                stack.push((node, Idle));
            }
        }
        while let Some((node, state)) = stack.pop() {
            // vertex is the payload of any GraphNode variant.
            let vertex = &graph[node];
            if graph.edges(node).count() == 0 && state == InForEach {
                bail!("'{vertex}' has a for-each edge without a corresponding collect");
            }
            for edge in graph.edges(node) {
                let next_state = match (state, edge.weight()) {
                    (Idle, EdgeKind::Static)
                    | (Idle, EdgeKind::Normal)
                    | (Idle, EdgeKind::DoWhile)
                    | (Idle, EdgeKind::Break) => Idle,
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
                    (InForEach, EdgeKind::Static) => {
                        bail!("'{vertex}' has a static edge inside a for-each");
                    }
                };
                let target = edge.target();
                if seen.insert((target, next_state)) {
                    stack.push((target, next_state));
                }
            }
        }
        Ok(())
    }

    fn validate_do_while_nesting(&mut self, graph: &DiGraph<GraphNode, EdgeKind>) -> AnyResult<()> {
        let do_while_edges = graph
            .edge_references()
            .filter(|edge| *edge.weight() == EdgeKind::DoWhile)
            .map(|edge| (edge.source(), edge.target()))
            .collect::<Vec<_>>();
        // Original target tool -> closing output-port handles.
        let mut sources_by_target = HashMap::<NodeIndex, HashSet<NodeIndex>>::new();
        if !do_while_edges.is_empty() {
            let mut vertex_by_name = HashMap::new();
            for node in graph.node_indices() {
                if let GraphNode::Vertex { name } = &graph[node] {
                    vertex_by_name.entry(name.as_str()).or_insert(node);
                }
            }
            // Both are NodeIndex handles; edge_target is the synthetic loop input.
            for (source, edge_target) in do_while_edges {
                let GraphNode::InputPort { vertex, .. } = &graph[edge_target] else {
                    continue;
                };
                // Resolve the synthetic destination back to the original loop target
                // whose body is being checked.
                let original_vertex_name = vertex.strip_suffix("-do-while").unwrap_or(vertex);
                if let Some(&target) = vertex_by_name.get(original_vertex_name) {
                    sources_by_target.entry(target).or_default().insert(source);
                }
            }
        }

        // Closing edges may share a target. Reuse its forward mask, while each
        // closing source gets its own reverse walk and loop-body intersection.
        for (target, sources) in sources_by_target {
            self.traverse(graph, [target], petgraph::Direction::Outgoing);
            let reachable_from_target = self.mask.clone();
            for source in sources {
                self.traverse(graph, [source], petgraph::Direction::Incoming);
                // Union of node handles on all target-to-source paths.
                let mut paths = reachable_from_target
                    .intersection(&self.mask)
                    .map(NodeIndex::new)
                    .collect::<HashSet<_>>();
                // Exclude the closing source so its own DoWhile edge is not nesting.
                // Check every outgoing edge of the remaining region, including edges
                // that leave it; restricting checks to internal edges would miss nesting.
                paths.remove(&source);
                for path_node in paths {
                    for edge in graph.edges(path_node) {
                        if matches!(
                            edge.weight(),
                            EdgeKind::DoWhile
                                | EdgeKind::Break
                                | EdgeKind::ForEach
                                | EdgeKind::Collect
                        ) {
                            bail!(
                                "Do-while edge at '{}' has a nested loop inside.",
                                graph[source]
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Validate function takes a DAG and validates it based on nexus execution
/// rules.
///
/// See our wiki for more information on the rules:
/// <https://docs.talus.network/talus-documentation/devs/index/workflow#rules>
/// <https://docs.talus.network/talus-documentation/devs/index-1/cli#nexus-dag>
pub fn validate(dag: &DagSpec) -> AnyResult<()> {
    // Parse the dag into a petgraph DiGraph.
    let (graph, vertex_entry_groups) = try_into_graph(dag)?;

    if !graph.is_directed() || petgraph::algo::is_cyclic_directed(&graph) {
        bail!("The provided graph contains one or more cycles.");
    }

    // Check that the shape of the graph is correct.
    has_correct_order_of_actions(&graph)?;

    // Indexing cannot report validation errors. Preserve cycle/shape checks first,
    // followed by concurrency, ForEach pairing and DoWhile nesting in that order.
    let mut analysis = ValidationAnalysis::new(&graph, &vertex_entry_groups);

    // Check that no walks in the graph violate the concurrency rules.
    analysis.validate_concurrency_rules(&graph, REACHABILITY_MASK_BUDGET_BYTES)?;

    // Check that for-each and collect edges are correctly paired and not nesting.
    analysis.validate_for_each_pairs(&graph)?;

    // Check that do-whiles don't nest.
    analysis.validate_do_while_nesting(&graph)?;

    Ok(())
}

fn has_correct_order_of_actions(graph: &DiGraph<GraphNode, EdgeKind>) -> AnyResult<()> {
    for node in graph.node_indices() {
        // vertex is the payload of any GraphNode variant.
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
                        // Count of relevant output ports, not a collection.
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

fn root_set_batch_width(mask_bytes: usize, budget_bytes: usize) -> usize {
    // Keep a nonzero batch width for empty graphs and masks larger than the budget.
    // An oversized mask is processed alone, alongside the shared traversal scratch.
    if mask_bytes == 0 || mask_bytes > budget_bytes {
        return 1;
    }
    budget_bytes / mask_bytes
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
// GraphNode is the stored payload; NodeIndex is its handle.
// In port/variant payloads, vertex is the owning tool's name.
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

/// [`DagSpec`] to [petgraph::graph::DiGraph]. Also performs structure checks on the
/// graph.
fn try_into_graph(dag: &DagSpec) -> AnyResult<GraphAndVertexEntryGroups> {
    let mut graph = DiGraph::<GraphNode, EdgeKind>::new();

    // Build a hash map of graph nodes that are part of entry groups. If there
    // are no entry groups specified, we assume all vertices that have entry
    // ports specified are part of the default entry group.
    let mut vertex_entry_groups: HashMap<GraphNode, Vec<String>> = HashMap::new();

    if !dag.entry_groups.is_empty() {
        for entry_group in &dag.entry_groups {
            for vertex in &entry_group.vertices {
                let vertex_ident = GraphNode::Vertex {
                    name: vertex.clone(),
                };

                // Check that the group references a vertex that exists.
                if !dag.vertices.iter().any(|v| &v.name == vertex) {
                    bail!(
                        "Entry group '{}' references a non-existing vertex '{vertex}'.",
                        entry_group.name
                    );
                }

                let mut groups = vertex_entry_groups
                    .remove(&vertex_ident)
                    .unwrap_or_default();
                groups.push(entry_group.name.clone());
                vertex_entry_groups.insert(vertex_ident, groups);
            }
        }

        // Entry ports are only published for entry group members, so the
        // ports of a vertex outside every group would be dropped silently.
        for vertex in &dag.vertices {
            let vertex_ident = GraphNode::Vertex {
                name: vertex.name.clone(),
            };

            if !vertex.entry_ports.is_empty() && !vertex_entry_groups.contains_key(&vertex_ident) {
                bail!("'{vertex_ident}' has entry ports but is not a member of any entry group.");
            }
        }
    } else {
        // If there are no entry groups, all vertices that have entry ports
        // specified are part of the default entry group.
        for vertex in &dag.vertices {
            if vertex.entry_ports.is_empty() {
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
    // GraphNode identity -> NodeIndex; shared ports and variants reuse the same handle.
    let mut graph_nodes: HashMap<GraphNode, NodeIndex> = HashMap::new();

    for edge in &dag.edges {
        // Source tool payload; origin_node below is its graph handle.
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
        graph.add_edge(output_port_node, input_port_node, edge.kind);
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

    // Edges, default values and outputs may only name declared vertices;
    // publishing aborts on chain for any other name.
    for edge in &dag.edges {
        for vertex in [&edge.from.vertex, &edge.to.vertex] {
            if !all_vertices.contains(&GraphNode::Vertex {
                name: vertex.clone(),
            }) {
                bail!(
                    "Edge from '{}' to '{}' references a non-existing vertex '{vertex}'.",
                    edge.from.vertex,
                    edge.to.vertex
                );
            }
        }
    }

    // Check that entry ports are not defined twice and that they have no edges
    // leading into them.
    for vertex in &dag.vertices {
        if vertex.entry_ports.is_empty() {
            continue;
        }

        for entry_port in &vertex.entry_ports {
            let entry_port_ident = GraphNode::InputPort {
                vertex: vertex.name.clone(),
                name: entry_port.name.clone(),
            };

            if graph_nodes.contains_key(&entry_port_ident) {
                bail!(
                    "'{entry_port_ident}' has an edge leading to it and therefore cannot be an entry port.",
                );
            }

            if !all_entry_ports.insert(entry_port_ident.clone()) {
                bail!("'{entry_port_ident}' is defined multiple times.",);
            }
        }
    }

    // Check that none of the default value input ports are in the graph.
    // Tracks input identities, not their default JSON values.
    let mut all_default_values = HashSet::new();

    for default_value in &dag.default_values {
        let vertex = &default_value.vertex;
        // Shadows the declaration with its input-port identity.
        let default_value = GraphNode::InputPort {
            vertex: vertex.clone(),
            name: default_value.input_port.clone(),
        };

        if !all_vertices.contains(&GraphNode::Vertex {
            name: vertex.clone(),
        }) {
            bail!(
                "Default value for '{default_value}' references a non-existing vertex '{vertex}'."
            );
        }

        if graph_nodes.contains_key(&default_value) || all_entry_ports.contains(&default_value) {
            bail!(
                "'{default_value}' is an entry port or has an edge leading into it and therefore cannot have a default value.",
            );
        }

        if !all_default_values.insert(default_value.clone()) {
            bail!("'{default_value}' has multiple default values.");
        }
    }

    for output in &dag.outputs {
        if !all_vertices.contains(&GraphNode::Vertex {
            name: output.vertex.clone(),
        }) {
            bail!(
                "Output '{}.{}.{}' references a non-existing vertex '{}'.",
                output.vertex,
                output.output_variant,
                output.output_port,
                output.vertex
            );
        }
    }

    // Check that outputs are not defined on vertices that have outgoing edges.
    for vertex in &dag.vertices {
        let has_outputs = dag
            .outputs
            .iter()
            .any(|output| output.vertex == vertex.name);
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

    // variant is a (tool name, output-variant name) pair.
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
        crate::types::{
            DagEdge,
            DagEntryPort,
            DagInput,
            DagOutput,
            DagVertex,
            DagVertexKind,
            NexusData,
        },
        assert_matches::assert_matches,
        petgraph::visit::EdgeRef,
        std::time::Instant,
    };

    fn parse(input: &str) -> Result<DagSpec, serde_json::Error> {
        crate::dag::json::parse_dag_spec(input)
    }

    fn serial_alternative_diamond_dag(stages: usize) -> DagSpec {
        let mut vertices = Vec::with_capacity(stages * 3 + 1);
        for stage in 0..=stages {
            vertices.push(test_tool_vertex(format!("split_{stage}"), stage == 0));
            if stage == stages {
                continue;
            }
            vertices.push(test_tool_vertex(format!("left_{stage}"), false));
            vertices.push(test_tool_vertex(format!("right_{stage}"), false));
        }

        let mut edges = Vec::with_capacity(stages * 8);
        for stage in 0..stages {
            let split = format!("split_{stage}");
            let left = format!("left_{stage}");
            let right = format!("right_{stage}");
            let next_split = format!("split_{}", stage + 1);

            for (variant, branch) in [("left", &left), ("right", &right)] {
                edges.push(DagEdge {
                    from: DagOutput {
                        vertex: split.clone(),
                        output_variant: variant.to_string(),
                        output_port: "result".to_string(),
                    },
                    to: DagInput {
                        vertex: branch.clone(),
                        input_port: "input".to_string(),
                    },
                    kind: EdgeKind::Normal,
                });
            }

            for branch in [&left, &right] {
                for variant in ["route_0", "route_1", "route_2"] {
                    edges.push(DagEdge {
                        from: DagOutput {
                            vertex: branch.clone(),
                            output_variant: variant.to_string(),
                            output_port: "result".to_string(),
                        },
                        to: DagInput {
                            vertex: next_split.clone(),
                            input_port: "joined".to_string(),
                        },
                        kind: EdgeKind::Normal,
                    });
                }
            }
        }

        DagSpec {
            vertices,
            edges,
            outputs: vec![DagOutput {
                vertex: format!("split_{stages}"),
                output_variant: "done".to_string(),
                output_port: "result".to_string(),
            }],
            ..DagSpec::default()
        }
    }

    fn test_tool_vertex(name: String, is_entry: bool) -> DagVertex {
        DagVertex {
            kind: DagVertexKind::OffChain {
                tool_fqn: crate::fqn!("com.test.test@1"),
            },
            name,
            entry_ports: if is_entry {
                vec![DagEntryPort {
                    name: "input".to_string(),
                }]
            } else {
                Vec::new()
            },
            post_failure_action: None,
            verifier: None,
        }
    }

    fn normal_test_edge(from: &str, to: &str, input_port: &str) -> DagEdge {
        DagEdge {
            from: DagOutput {
                vertex: from.to_string(),
                output_variant: "route".to_string(),
                output_port: "result".to_string(),
            },
            to: DagInput {
                vertex: to.to_string(),
                input_port: input_port.to_string(),
            },
            kind: EdgeKind::Normal,
        }
    }

    fn shared_sink_graph(
        root_names: &[&str],
        connected_to_sink: &[bool],
        minimum_node_bound: usize,
    ) -> (
        DiGraph<GraphNode, EdgeKind>,
        HashMap<String, NodeIndex>,
        NodeIndex,
    ) {
        assert_eq!(root_names.len(), connected_to_sink.len());
        let mut graph = DiGraph::new();
        let sink = graph.add_node(GraphNode::InputPort {
            vertex: "sink".to_string(),
            name: "input".to_string(),
        });
        let mut roots = HashMap::new();
        for (&name, &connected) in root_names.iter().zip(connected_to_sink) {
            // vertex, variant, and port below are NodeIndex handles.
            let vertex = graph.add_node(GraphNode::Vertex {
                name: name.to_string(),
            });
            let variant = graph.add_node(GraphNode::OutputVariant {
                vertex: name.to_string(),
                name: "route".to_string(),
            });
            let port = graph.add_node(GraphNode::OutputPort {
                vertex: name.to_string(),
                variant: "route".to_string(),
                name: "result".to_string(),
            });
            graph.add_edge(vertex, variant, EdgeKind::Normal);
            graph.add_edge(variant, port, EdgeKind::Normal);
            if connected {
                graph.add_edge(port, sink, EdgeKind::Normal);
            }
            roots.insert(name.to_string(), vertex);
        }
        while graph.node_bound() < minimum_node_bound {
            graph.add_node(GraphNode::Vertex {
                name: format!("padding_{}", graph.node_count()),
            });
        }
        (graph, roots, sink)
    }

    #[test]
    fn entry_root_sets_are_sorted_unique_and_share_exact_regions() {
        let (graph, roots, _) = shared_sink_graph(&["a", "b", "c"], &[true, true, false], 0);
        let ordered = "ordered_ab".to_string();
        let reversed = "reversed_ab".to_string();
        let duplicate = "duplicate_ab".to_string();
        let control = "control_bc".to_string();
        let mut vertex_entry_groups = HashMap::new();
        vertex_entry_groups.insert(
            graph[roots["a"]].clone(),
            vec![
                ordered.clone(),
                reversed.clone(),
                duplicate.clone(),
                duplicate.clone(),
            ],
        );
        vertex_entry_groups.insert(
            graph[roots["b"]].clone(),
            vec![
                ordered.clone(),
                reversed.clone(),
                duplicate.clone(),
                control.clone(),
            ],
        );
        vertex_entry_groups.insert(graph[roots["c"]].clone(), vec![control.clone()]);

        let mut analysis = ValidationAnalysis::new(&graph, &vertex_entry_groups);
        assert_eq!(analysis.root_sets.len(), 2);
        let equivalent_groups =
            HashSet::from([ordered.as_str(), reversed.as_str(), duplicate.as_str()]);
        let equivalent = analysis
            .root_sets
            .iter()
            .find(|(_, group)| equivalent_groups.contains(group.as_str()))
            .unwrap();
        let mut expected_roots = vec![roots["a"], roots["b"]];
        expected_roots.sort_unstable();
        assert_eq!(equivalent.0, expected_roots);
        assert!(equivalent_groups.contains(equivalent.1.as_str()));
        let control_root_set = analysis
            .root_sets
            .iter()
            .find(|(_, group)| group == &control)
            .unwrap();
        assert_ne!(equivalent.0, control_root_set.0);

        analysis.traversal_count = 0;
        let error = analysis
            .validate_concurrency_rules(&graph, REACHABILITY_MASK_BUDGET_BYTES)
            .unwrap_err();
        assert_eq!(analysis.traversal_count, 3);
        let message = error.to_string();
        assert!(message.contains("Input port: sink.input"));
        assert!(message.contains("has a race condition on it when invoking group"));
        assert!(equivalent_groups
            .iter()
            .any(|group| message.contains(&format!("invoking group '{group}'"))));
        assert!(!message.contains("invoking group 'control_bc'"));
    }

    #[test]
    fn identical_entry_groups_share_forward_and_reverse_walks() {
        let (graph, roots, _) = shared_sink_graph(&["root"], &[true], 0);
        let groups = (0..32)
            .map(|index| format!("group_{index}"))
            .collect::<Vec<_>>();
        let vertex_entry_groups = HashMap::from([(graph[roots["root"]].clone(), groups.clone())]);
        let mut analysis = ValidationAnalysis::new(&graph, &vertex_entry_groups);
        assert_eq!(analysis.root_sets.len(), 1);
        assert!(groups.contains(&analysis.root_sets[0].1));

        analysis
            .validate_concurrency_rules(&graph, REACHABILITY_MASK_BUDGET_BYTES)
            .unwrap();

        assert_eq!(analysis.traversal_count, 2);
    }

    #[test]
    fn root_mask_batches_obey_budget_and_progress_for_oversized_masks() {
        let exact_budget_mask = ReachabilityMask::with_capacity(8_388_608);
        let exact_budget_bytes = std::mem::size_of_val(exact_budget_mask.as_slice());
        assert_eq!(exact_budget_bytes, REACHABILITY_MASK_BUDGET_BYTES);
        assert_eq!(
            root_set_batch_width(exact_budget_bytes, REACHABILITY_MASK_BUDGET_BYTES),
            1
        );
        let oversized_mask = ReachabilityMask::with_capacity(8_388_609);
        let oversized_bytes = std::mem::size_of_val(oversized_mask.as_slice());
        assert_eq!(
            oversized_bytes,
            exact_budget_bytes + std::mem::size_of_val(&oversized_mask.as_slice()[0])
        );
        assert!(oversized_bytes > REACHABILITY_MASK_BUDGET_BYTES);
        assert_eq!(
            root_set_batch_width(oversized_bytes, REACHABILITY_MASK_BUDGET_BYTES),
            1
        );

        let names = ["r0", "r1", "r2", "r3", "r4"];
        let (graph, roots, _) = shared_sink_graph(&names, &[true; 5], 65);
        let vertex_entry_groups = names
            .iter()
            .enumerate()
            .map(|(index, name)| (graph[roots[*name]].clone(), vec![format!("group_{index}")]))
            .collect::<HashMap<_, _>>();
        let mut analysis = ValidationAnalysis::new(&graph, &vertex_entry_groups);
        assert_eq!(analysis.root_sets.len(), 5);
        let mask_bytes = std::mem::size_of_val(analysis.mask.as_slice());
        assert_eq!(mask_bytes, 16);
        assert_eq!(root_set_batch_width(16, 32), 2);
        assert_eq!(root_set_batch_width(16, 8), 1);
        assert!(root_set_batch_width(16, 32) > 0);

        analysis.traversal_count = 0;
        analysis.validate_concurrency_rules(&graph, 32).unwrap();
        assert_eq!(analysis.traversal_count, 8);

        let mut oversized = ValidationAnalysis::new(&graph, &vertex_entry_groups);
        oversized.validate_concurrency_rules(&graph, 8).unwrap();
        assert_eq!(oversized.traversal_count, 10);
    }

    #[test]
    fn indexed_for_each_rejects_unresolved_entry() {
        let graph = DiGraph::new();
        let missing = GraphNode::Vertex {
            name: "missing".to_string(),
        };
        let vertex_entry_groups = HashMap::from([(missing, vec!["group".to_string()])]);
        let error = ValidationAnalysis::new(&graph, &vertex_entry_groups)
            .validate_for_each_pairs(&graph)
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "'Vertex: missing' not found in graph nodes."
        );
    }

    #[test]
    fn repeated_do_while_target_is_traversed_once() {
        let mut graph = DiGraph::new();
        let target = graph.add_node(GraphNode::Vertex {
            name: "target".to_string(),
        });
        let first_source = graph.add_node(GraphNode::OutputPort {
            vertex: "loop_a".to_string(),
            variant: "loop".to_string(),
            name: "result".to_string(),
        });
        let second_source = graph.add_node(GraphNode::OutputPort {
            vertex: "loop_b".to_string(),
            variant: "loop".to_string(),
            name: "result".to_string(),
        });
        let feedback_sink = graph.add_node(GraphNode::InputPort {
            vertex: "target-do-while".to_string(),
            name: "feedback".to_string(),
        });
        graph.add_edge(target, first_source, EdgeKind::Normal);
        graph.add_edge(target, second_source, EdgeKind::Normal);
        graph.add_edge(first_source, feedback_sink, EdgeKind::DoWhile);
        graph.add_edge(first_source, feedback_sink, EdgeKind::DoWhile);
        graph.add_edge(second_source, feedback_sink, EdgeKind::DoWhile);

        let entry_groups = HashMap::new();
        let mut analysis = ValidationAnalysis::new(&graph, &entry_groups);
        analysis.validate_do_while_nesting(&graph).unwrap();
        assert_eq!(analysis.traversal_count, 3);
    }

    #[test]
    fn self_loop_reaches_cycle_guard() {
        let dag = DagSpec {
            vertices: vec![test_tool_vertex("loop".to_string(), true)],
            edges: vec![normal_test_edge("loop", "loop", "feedback")],
            ..DagSpec::default()
        };

        assert_matches!(validate(&dag), Err(error) if error.to_string().contains("The provided graph contains one or more cycles."));
    }

    #[test]
    fn disconnected_multi_vertex_cycle_reaches_cycle_guard() {
        let dag = DagSpec {
            vertices: vec![
                test_tool_vertex("entry".to_string(), true),
                test_tool_vertex("tail".to_string(), false),
                test_tool_vertex("cycle_a".to_string(), false),
                test_tool_vertex("cycle_b".to_string(), false),
            ],
            edges: vec![
                normal_test_edge("entry", "tail", "input"),
                normal_test_edge("cycle_a", "cycle_b", "input"),
                normal_test_edge("cycle_b", "cycle_a", "input"),
            ],
            ..DagSpec::default()
        };

        assert_matches!(validate(&dag), Err(error) if error.to_string().contains("The provided graph contains one or more cycles."));
    }

    fn measure_serial_alternative_diamonds(
        stages: usize,
        expected_tools: usize,
        expected_edges: usize,
    ) {
        let dag = serial_alternative_diamond_dag(stages);
        assert_eq!(dag.vertices.len(), expected_tools);
        assert_eq!(dag.edges.len(), expected_edges);
        let (expanded_graph, _) = try_into_graph(&dag).unwrap();
        let expanded_nodes = expanded_graph.node_count();
        let expanded_edges = expanded_graph.edge_count();
        assert_eq!(expanded_nodes, stages * 22 + 1);
        assert_eq!(expanded_edges, stages * 27);

        let started = Instant::now();
        let result = validate(&dag);
        let elapsed = started.elapsed();
        eprintln!(
            "stress stages={stages} logical_tools={} logical_edges={} expanded_nodes={expanded_nodes} expanded_edges={expanded_edges} routes=6^{stages} validate_ms={}",
            dag.vertices.len(),
            dag.edges.len(),
            elapsed.as_millis()
        );
        assert!(result.is_ok(), "stress DAG rejected: {result:?}");
    }

    #[test]
    #[ignore = "release-only stress measurement; run with a 30-second timeout"]
    fn stress_deep_serial_diamonds_24() {
        measure_serial_alternative_diamonds(24, 73, 192);
    }

    #[test]
    #[ignore = "release-only stress measurement; run with a 30-second timeout"]
    fn stress_large_serial_diamonds_333() {
        measure_serial_alternative_diamonds(333, 1_000, 2_664);
    }

    #[test]
    fn for_each_keeps_both_states_at_a_shared_node() {
        let mut graph = DiGraph::new();
        let root = graph.add_node(GraphNode::Vertex {
            name: "root".to_string(),
        });
        let shared = graph.add_node(GraphNode::Vertex {
            name: "shared".to_string(),
        });
        let leaf = graph.add_node(GraphNode::Vertex {
            name: "leaf".to_string(),
        });

        graph.add_edge(root, shared, EdgeKind::Normal);
        graph.add_edge(root, shared, EdgeKind::Normal);
        let root_edges = graph.edges(root).map(|edge| edge.id()).collect::<Vec<_>>();
        assert_eq!(root_edges.len(), 2);
        *graph.edge_weight_mut(root_edges[0]).unwrap() = EdgeKind::Normal;
        *graph.edge_weight_mut(root_edges[1]).unwrap() = EdgeKind::ForEach;
        graph.add_edge(shared, leaf, EdgeKind::Collect);

        let mut vertex_entry_groups = HashMap::new();
        vertex_entry_groups.insert(graph[root].clone(), vec!["test_group".to_string()]);
        let error = ValidationAnalysis::new(&graph, &vertex_entry_groups)
            .validate_for_each_pairs(&graph)
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "'Vertex: shared' has a collect edge without for-each"
        );
    }

    fn synthetic_do_while_graph() -> DiGraph<GraphNode, EdgeKind> {
        let mut graph = DiGraph::new();
        let target = graph.add_node(GraphNode::Vertex {
            name: "target".to_string(),
        });
        let source = graph.add_node(GraphNode::OutputPort {
            vertex: "loop_source".to_string(),
            variant: "loop".to_string(),
            name: "result".to_string(),
        });
        let feedback_sink = graph.add_node(GraphNode::InputPort {
            vertex: "target-do-while".to_string(),
            name: "feedback".to_string(),
        });
        graph.add_edge(source, feedback_sink, EdgeKind::DoWhile);

        graph.add_edge(target, source, EdgeKind::Normal);

        let outside = graph.add_node(GraphNode::Vertex {
            name: "outside".to_string(),
        });
        graph.add_edge(target, outside, EdgeKind::ForEach);
        graph
    }

    #[test]
    fn do_while_checks_forbidden_edges_leaving_the_path_region() {
        let graph = synthetic_do_while_graph();
        let entry_groups = HashMap::new();
        let mut analysis = ValidationAnalysis::new(&graph, &entry_groups);
        let error = analysis.validate_do_while_nesting(&graph).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Do-while edge at 'Output port: loop_source.loop.result' has a nested loop inside."
        );
    }

    // == Various graph shapes ==

    #[test]
    fn test_ig_story_planner_valid() {
        let dag = parse(include_str!("_dags/ig_story_planner_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_ordered_many_default_valid() {
        let dag = parse(include_str!("_dags/default_many_valid.json")).unwrap();

        let validation = validate(&dag);
        assert!(
            validation.is_ok(),
            "unexpected validation error: {validation:?}"
        );
        assert_eq!(
            dag.default_values[0].value,
            serde_json::json!({
                "many": [
                    { "kind": "data", "data": 1 },
                    { "kind": "data", "data": { "ordered": 2 } },
                ],
            }),
        );
        let typed = NexusData::from_json_value(&dag.default_values[0].value).unwrap();
        assert_eq!(typed.to_json_value().unwrap(), dag.default_values[0].value,);
    }

    #[test]
    fn test_immediately_converges_valid() {
        let dag = parse(include_str!("_dags/immediately_converges_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_immediately_converges_invalid() {
        let dag = parse(include_str!("_dags/immediately_converges_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: b.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_intertwined_valid() {
        let dag = parse(include_str!("_dags/intertwined_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_intertwined_invalid() {
        let dag = parse(include_str!("_dags/intertwined_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_multiple_output_ports_invalid() {
        let dag = parse(include_str!("_dags/multiple_output_ports_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_multiple_output_ports_valid() {
        let dag = parse(include_str!("_dags/multiple_output_ports_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_multiple_goals_valid() {
        let dag = parse(include_str!("_dags/multiple_goals_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_dead_ends_valid() {
        let dag = parse(include_str!("_dags/dead_ends_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_multiple_entry_multiple_goal_valid() {
        let dag = parse(include_str!(
            "_dags/multiple_entry_multiple_goal_valid.json"
        ))
        .unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_multiple_entry_multiple_goal_invalid() {
        let dag = parse(include_str!(
            "_dags/multiple_entry_multiple_goal_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: c.1' has a race condition on it when invoking group 'group_a'"));
    }

    #[test]
    fn test_branched_net_zero_invalid() {
        let dag = parse(include_str!("_dags/branched_net_zero_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_entry_groups_valid() {
        let dag = parse(include_str!("_dags/entry_groups_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_entry_groups_twice_valid() {
        let dag = parse(include_str!("_dags/entry_groups_twice_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_entry_groups_ne_invalid() {
        let dag = parse(include_str!("_dags/entry_groups_ne_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: e.2' is unreachable when invoking group 'group_b'"));
    }

    #[test]
    fn test_entry_groups_tm_invalid() {
        let dag = parse(include_str!("_dags/entry_groups_tm_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: e.2' has a race condition on it when invoking group 'group_b'"));
    }

    #[test]
    fn test_entry_groups_unlisted_entry_ports_invalid() {
        let dag = parse(include_str!(
            "_dags/entry_groups_unlisted_entry_ports_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: b' has entry ports but is not a member of any entry group."));
    }

    #[test]
    fn both_loops_valid() {
        let dag = parse(include_str!("_dags/both_loops_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn missing_do_while_invalid() {
        let dag = parse(include_str!("_dags/missing_do_while_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Vertex 'until_11' has a break edge but no corresponding do-while edge."));
    }

    #[test]
    fn missing_break_invalid() {
        let dag = parse(include_str!("_dags/missing_break_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Vertex 'until_11' has a do-while edge but no corresponding break edge."));
    }

    #[test]
    fn collect_without_for_each_invalid() {
        let dag = parse(include_str!("_dags/collect_without_for_each_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output port: a.ok.result' has a collect edge without for-each"));
    }

    #[test]
    fn nested_for_each_invalid() {
        let dag = parse(include_str!("_dags/nested_for_each_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output port: b.ok.result' has a nested for-each"));
    }

    #[test]
    fn do_while_in_for_each_invalid() {
        let dag = parse(include_str!("_dags/do_while_in_for_each_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output port: b.also_ok.loop' has a do-while or break edge inside a for-each"));
    }

    #[test]
    fn unclosed_for_each_invalid() {
        let dag = parse(include_str!("_dags/unclosed_for_each_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: b' has a for-each edge without a corresponding collect"));
    }

    #[test]
    fn do_while_and_break_same_variant_invalid() {
        let dag = parse(include_str!(
            "_dags/do_while_and_break_same_variant_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Output variant: b.ok' has both a do-while and a break edge, but they must branch."));
    }

    #[test]
    fn do_while_nested_invalid() {
        let dag = parse(include_str!("_dags/do_while_nested_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Do-while edge at 'Output port: c.also_ok.loop' has a nested loop inside."));
    }

    #[test]
    fn test_double_do_while_static_valid() {
        let dag = parse(include_str!("_dags/double_do_while_static_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    // == Cyclic or no input graphs ==

    #[test]
    fn test_cyclic_invalid() {
        let dag = parse(include_str!("_dags/cyclic_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: a.input' has an edge leading to it and therefore cannot be an entry port."));
    }

    // == Parser tests ==

    #[test]
    fn test_undefined_connections_invalid() {
        let dag = parse(include_str!("_dags/undefined_connections_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: a' is not connected to the DAG."));
    }

    #[test]
    fn test_edge_references_non_existing_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/edge_references_non_existing_vertex_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Edge from 'a' to 'b' references a non-existing vertex 'b'."));
    }

    #[test]
    fn test_default_value_references_non_existing_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/default_value_references_non_existing_vertex_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Default value for 'Input port: b.input' references a non-existing vertex 'b'."));
    }

    #[test]
    fn test_default_value_defined_twice_invalid() {
        let dag = parse(include_str!(
            "_dags/default_value_defined_twice_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: a.extra' has multiple default values."));
    }

    #[test]
    fn test_output_references_non_existing_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/output_references_non_existing_vertex_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Output 'b.ok.output' references a non-existing vertex 'b'."));
    }

    #[test]
    fn test_output_and_edge_invalid() {
        let dag = parse(include_str!("_dags/output_and_edge_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: a' cannot have both outgoing edges and ports marked as output."));
    }

    #[test]
    fn test_def_val_on_input_port_invalid() {
        let dag = parse(include_str!("_dags/has_def_on_input_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Input port: location_decider.context' is an entry port or has an edge leading into it and therefore cannot have a default value."));
    }

    #[test]
    fn test_references_non_existing_vertex() {
        let dag = parse(include_str!("_dags/references_non_existing_vertex.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("Entry group 'group_a' references a non-existing vertex 'invalid'."));
    }

    #[test]
    fn test_empty_invalid() {
        let dag = parse(include_str!("_dags/empty_invalid.json")).unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("The DAG has no entry vertices or ports."));
    }

    #[test]
    fn test_both_vertex_and_entry_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/both_vertex_and_entry_vertex_invalid.json"
        ))
        .unwrap();

        let res = validate(&dag);

        assert_matches!(res, Err(e) if e.to_string().contains("'Vertex: b' is defined multiple times."));
    }
}
