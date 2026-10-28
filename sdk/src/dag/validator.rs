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

type GraphAndToolEntryGroupNames = (
    DiGraph<GraphNode, EdgeKind>,
    HashMap<GraphNode, Vec<String>>,
);

// bitmap max size.
const REACHABILITY_MASK_BUDGET_BYTES: usize = 1024 * 1024;
// Use DiGraph's visit map as bitmap.
type ReachabilityMask = <DiGraph<GraphNode, EdgeKind> as Visitable>::Map;

// Index the immutable expanded graph once per validation and share traversal scratch.
struct ValidationAnalysis {
    graph: DiGraph<GraphNode, EdgeKind>,
    input_port_node_indices: Vec<NodeIndex>,
    // Entry tool nodes and optional indices; None records a missing graph node.
    entry_tool_nodes_with_indices: Vec<(GraphNode, Option<NodeIndex>)>,
    // Distinct root sets paired with one representative group name.
    entry_node_index_groups: Vec<(Vec<NodeIndex>, String)>,
    reachability_mask: ReachabilityMask,
    pending_node_indices: Vec<NodeIndex>,
    #[cfg(test)]
    traversal_count: usize,
}

impl ValidationAnalysis {
    fn new(
        graph: DiGraph<GraphNode, EdgeKind>,
        entry_group_names_by_tool_node: HashMap<GraphNode, Vec<String>>,
    ) -> Self {
        let mut entry_node_indices_by_group_name = entry_group_names_by_tool_node
            .values()
            .flatten()
            .map(|entry_group_name| (entry_group_name.clone(), Vec::new()))
            .collect::<HashMap<String, Vec<NodeIndex>>>();
        let mut input_port_node_indices = Vec::new();
        let mut node_index_by_node = HashMap::with_capacity(graph.node_count());
        for node_index in graph.node_indices() {
            let node = &graph[node_index];
            node_index_by_node.insert(node, node_index);
            if matches!(node, GraphNode::InputPort { .. }) {
                input_port_node_indices.push(node_index);
            }
            if let Some(entry_group_names) = entry_group_names_by_tool_node.get(node) {
                for entry_group_name in entry_group_names {
                    entry_node_indices_by_group_name
                        .entry(entry_group_name.clone())
                        .or_default()
                        .push(node_index);
                }
            }
        }
        let entry_tool_nodes_with_indices = entry_group_names_by_tool_node
            .into_keys()
            .map(|tool_node| {
                let tool_node_index = node_index_by_node.get(&&tool_node).copied();
                (tool_node, tool_node_index)
            })
            .collect();

        // A root sets produce identical path regions and concurrency scores.
        // Normalize membership and store one original group name for diagnostics.
        let mut group_name_by_entry_node_indices = HashMap::<Vec<NodeIndex>, String>::new();
        for (entry_group_name, mut tool_node_indices) in entry_node_indices_by_group_name {
            tool_node_indices.sort_unstable();
            tool_node_indices.dedup();
            group_name_by_entry_node_indices
                .entry(tool_node_indices)
                .or_insert(entry_group_name);
        }
        let entry_node_index_groups = group_name_by_entry_node_indices.into_iter().collect();

        Self {
            input_port_node_indices,
            entry_tool_nodes_with_indices,
            entry_node_index_groups,
            reachability_mask: graph.visit_map(),
            pending_node_indices: Vec::with_capacity(graph.node_bound()),
            graph,
            #[cfg(test)]
            traversal_count: 0,
        }
    }

    // Each walk overwrites the shared bitmap. Callers clone it only when a result
    // must survive the next walk; the bitmap and stack allocations are reused.
    fn traverse(
        &mut self,
        start_node_indices: impl IntoIterator<Item = NodeIndex>,
        direction: petgraph::Direction,
    ) {
        #[cfg(test)]
        {
            self.traversal_count += 1;
        }
        self.reachability_mask.clear();
        self.pending_node_indices.clear();
        for start_node_index in start_node_indices {
            if self.reachability_mask.visit(start_node_index) {
                self.pending_node_indices.push(start_node_index);
            }
        }
        while let Some(node_index) = self.pending_node_indices.pop() {
            for neighbor_node_index in self.graph.neighbors_directed(node_index, direction) {
                if self.reachability_mask.visit(neighbor_node_index) {
                    self.pending_node_indices.push(neighbor_node_index);
                }
            }
        }
    }

    fn validate_concurrency_rules(&mut self, mask_budget_bytes: usize) -> AnyResult<()> {
        // Batch forward masks to avoid retaining a reachability map for every root set.
        let max_groups_per_batch = root_set_batch_width(
            std::mem::size_of_val(self.reachability_mask.as_slice()),
            mask_budget_bytes,
        );
        let mut batch_start_index = 0;
        while batch_start_index < self.entry_node_index_groups.len() {
            let batch_end_index = batch_start_index
                + max_groups_per_batch.min(self.entry_node_index_groups.len() - batch_start_index);
            let mut entry_group_reachability_masks =
                Vec::with_capacity(batch_end_index - batch_start_index);
            for node_group_index in batch_start_index..batch_end_index {
                let entry_node_indices = self.entry_node_index_groups[node_group_index].0.clone();
                self.traverse(entry_node_indices, petgraph::Direction::Outgoing);
                entry_group_reachability_masks.push(self.reachability_mask.clone());
            }

            // Compute each input's ancestors once per batch and share them across roots.
            // For U unique root sets, P inputs and batch width B, this takes
            // U + P * ceil(U / B) walks
            for input_port_position in 0..self.input_port_node_indices.len() {
                let input_port_node_index = self.input_port_node_indices[input_port_position];
                self.traverse([input_port_node_index], petgraph::Direction::Incoming);
                for node_group_index in batch_start_index..batch_end_index {
                    let entry_group_name = &self.entry_node_index_groups[node_group_index].1;
                    let entry_group_reachability_mask =
                        &entry_group_reachability_masks[node_group_index - batch_start_index];
                    // A node is on a root-to-input path exactly when a root can
                    // reach it and it can reach the input. Score that exact region.
                    let path_node_indices = entry_group_reachability_mask
                        .intersection(&self.reachability_mask)
                        .map(NodeIndex::new)
                        .collect::<HashSet<_>>();
                    if path_node_indices.is_empty() {
                        bail!(
                            "'{}' is unreachable when invoking group '{entry_group_name}'",
                            self.graph[input_port_node_index]
                        );
                    }
                    let net_concurrency =
                        get_net_concurrency_in_subgraph(&self.graph, &path_node_indices);
                    if net_concurrency < 0 {
                        bail!(
                            "'{}' is unreachable when invoking group '{entry_group_name}'",
                            self.graph[input_port_node_index]
                        );
                    }
                    if net_concurrency > 0 {
                        bail!(
                            "'{}' has a race condition on it when invoking group '{entry_group_name}'",
                            self.graph[input_port_node_index]
                        );
                    }
                }
            }
            batch_start_index = batch_end_index;
        }
        Ok(())
    }

    /// Check the for-each and collect edges are correctly paired and not nesting.
    fn validate_for_each_pairs(&self) -> AnyResult<()> {
        #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
        enum ForEachState {
            Idle,
            InForEach,
        }
        use ForEachState::*;

        let mut pending_node_states = Vec::new();
        // A node can be reached both inside and outside a ForEach. Plain reachability
        // cannot distinguish these cases, so visits must include the loop state.
        let mut seen_node_states = HashSet::new();
        for entry_tool_node_with_index in &self.entry_tool_nodes_with_indices {
            let (tool_node, tool_node_index) = entry_tool_node_with_index;
            let tool_node_index = tool_node_index
                .as_ref()
                .copied()
                .ok_or_else(|| anyhow::anyhow!("'{}' not found in graph nodes.", tool_node))?;
            if seen_node_states.insert((tool_node_index, Idle)) {
                pending_node_states.push((tool_node_index, Idle));
            }
        }
        while let Some((node_index, for_each_state)) = pending_node_states.pop() {
            let node = &self.graph[node_index];
            if self.graph.edges(node_index).count() == 0 && for_each_state == InForEach {
                bail!("'{node}' has a for-each edge without a corresponding collect");
            }
            for edge in self.graph.edges(node_index) {
                let next_for_each_state = match (for_each_state, edge.weight()) {
                    (Idle, EdgeKind::Static)
                    | (Idle, EdgeKind::Normal)
                    | (Idle, EdgeKind::DoWhile)
                    | (Idle, EdgeKind::Break) => Idle,
                    (Idle, EdgeKind::ForEach) => InForEach,
                    (Idle, EdgeKind::Collect) => {
                        bail!("'{node}' has a collect edge without for-each");
                    }
                    (InForEach, EdgeKind::Normal) => InForEach,
                    (InForEach, EdgeKind::ForEach) => {
                        bail!("'{node}' has a nested for-each");
                    }
                    (InForEach, EdgeKind::Collect) => Idle,
                    (InForEach, EdgeKind::DoWhile) | (InForEach, EdgeKind::Break) => {
                        bail!("'{node}' has a do-while or break edge inside a for-each");
                    }
                    (InForEach, EdgeKind::Static) => {
                        bail!("'{node}' has a static edge inside a for-each");
                    }
                };
                let target_node_index = edge.target();
                if seen_node_states.insert((target_node_index, next_for_each_state)) {
                    pending_node_states.push((target_node_index, next_for_each_state));
                }
            }
        }
        Ok(())
    }

    fn validate_do_while_nesting(&mut self) -> AnyResult<()> {
        let do_while_endpoint_node_indices = self
            .graph
            .edge_references()
            .filter(|edge| *edge.weight() == EdgeKind::DoWhile)
            .map(|edge| (edge.source(), edge.target()))
            .collect::<Vec<_>>();
        // Original target tool-node index -> closing output-port indices.
        let mut source_node_indices_by_target_node_index =
            HashMap::<NodeIndex, HashSet<NodeIndex>>::new();
        if !do_while_endpoint_node_indices.is_empty() {
            let mut tool_node_index_by_name = HashMap::new();
            for node_index in self.graph.node_indices() {
                if let GraphNode::Tool { tool_node_name } = &self.graph[node_index] {
                    tool_node_index_by_name
                        .entry(tool_node_name.as_str())
                        .or_insert(node_index);
                }
            }
            for (source_node_index, target_input_port_node_index) in do_while_endpoint_node_indices
            {
                let GraphNode::InputPort { tool_node_name, .. } =
                    &self.graph[target_input_port_node_index]
                else {
                    continue;
                };
                // Resolve the synthetic destination back to the original loop target
                // whose body is being checked.
                let original_tool_node_name = tool_node_name
                    .strip_suffix("-do-while")
                    .unwrap_or(tool_node_name);
                if let Some(&target_tool_node_index) =
                    tool_node_index_by_name.get(original_tool_node_name)
                {
                    source_node_indices_by_target_node_index
                        .entry(target_tool_node_index)
                        .or_default()
                        .insert(source_node_index);
                }
            }
        }

        // Closing edges may share a target. Reuse its forward mask, while each
        // closing source gets its own reverse walk and loop-body intersection.
        for (target_tool_node_index, source_node_indices) in
            source_node_indices_by_target_node_index
        {
            self.traverse([target_tool_node_index], petgraph::Direction::Outgoing);
            let target_reachability_mask = self.reachability_mask.clone();
            for source_node_index in source_node_indices {
                self.traverse([source_node_index], petgraph::Direction::Incoming);
                // Union of node indices on all target-to-source paths.
                let mut path_node_indices = target_reachability_mask
                    .intersection(&self.reachability_mask)
                    .map(NodeIndex::new)
                    .collect::<HashSet<_>>();
                // Exclude the closing source so its own DoWhile edge is not nesting.
                // Check every outgoing edge of the remaining region, including edges
                // that leave it; restricting checks to internal edges would miss nesting.
                path_node_indices.remove(&source_node_index);
                for path_node_index in path_node_indices {
                    for edge in self.graph.edges(path_node_index) {
                        if matches!(
                            edge.weight(),
                            EdgeKind::DoWhile
                                | EdgeKind::Break
                                | EdgeKind::ForEach
                                | EdgeKind::Collect
                        ) {
                            bail!(
                                "Do-while edge at '{}' has a nested loop inside.",
                                self.graph[source_node_index]
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
    let (graph, entry_group_names_by_tool_node) = try_into_graph(dag)?;

    if petgraph::algo::is_cyclic_directed(&graph) {
        bail!("The provided graph contains one or more cycles.");
    }

    // Check that the shape of the graph is correct.
    has_correct_order_of_actions(&graph)?;

    // Indexing cannot report validation errors. Preserve cycle/shape checks first,
    // followed by concurrency, ForEach pairing and DoWhile nesting in that order.
    let mut analysis = ValidationAnalysis::new(graph, entry_group_names_by_tool_node);

    // Check that no walks in the graph violate the concurrency rules.
    analysis.validate_concurrency_rules(REACHABILITY_MASK_BUDGET_BYTES)?;

    // Check that for-each and collect edges are correctly paired and not nesting.
    analysis.validate_for_each_pairs()?;

    // Check that do-whiles don't nest.
    analysis.validate_do_while_nesting()?;

    Ok(())
}

fn has_correct_order_of_actions(graph: &DiGraph<GraphNode, EdgeKind>) -> AnyResult<()> {
    for node_index in graph.node_indices() {
        let node = &graph[node_index];
        let neighbor_node_indices = graph
            .neighbors_directed(node_index, petgraph::Direction::Outgoing)
            .collect::<Vec<NodeIndex>>();

        // Check if the node has the correct number of edges.
        match node {
            // Input ports must have exactly 1 outgoing edge.
            GraphNode::InputPort { .. } if neighbor_node_indices.len() != 1 => {
                bail!("'{node}' must have exactly 1 outgoing edge")
            }
            // Tool nodes can be terminal and can have any number of edges.
            GraphNode::Tool { .. } => (),
            // Output variants must have at least 1 outgoing edge.
            GraphNode::OutputVariant { .. } if neighbor_node_indices.is_empty() => {
                bail!("'{node}' must have at least 1 outgoing edge")
            }
            // Output ports must have exactly 1 outgoing edge.
            GraphNode::OutputPort { .. } if neighbor_node_indices.len() != 1 => {
                bail!("'{node}' must have exactly 1 outgoing edge")
            }
            _ => (),
        };

        // Check if the edges are connected in the correct order.
        for neighbor_node_index in neighbor_node_indices {
            let neighbor_node = &graph[neighbor_node_index];

            let is_valid_edge = match node {
                GraphNode::InputPort { .. } => matches!(neighbor_node, GraphNode::Tool { .. }),
                GraphNode::Tool { .. } => matches!(neighbor_node, GraphNode::OutputVariant { .. }),
                GraphNode::OutputVariant { .. } => {
                    matches!(neighbor_node, GraphNode::OutputPort { .. })
                }
                GraphNode::OutputPort { .. } => {
                    matches!(neighbor_node, GraphNode::InputPort { .. })
                }
            };

            if !is_valid_edge {
                bail!("The edge from '{node}' to '{neighbor_node}' is invalid.");
            }
        }
    }

    Ok(())
}

fn get_net_concurrency_in_subgraph(
    graph: &DiGraph<GraphNode, EdgeKind>,
    node_indices: &HashSet<NodeIndex>,
) -> isize {
    let net_concurrency = node_indices.iter().fold(0, |concurrency_sum, &node_index| {
        match graph[node_index] {
            GraphNode::Tool { .. } => {
                // Calculate the maximum number of concurrent tasks that can be spawned by this tool.
                let max_tool_concurrency = graph
                    .neighbors_directed(node_index, petgraph::Outgoing)
                    // Only filter variants that are in the paths.
                    .filter(|output_variant_node_index| node_indices.contains(output_variant_node_index))
                    .map(|output_variant_node_index| {
                        let output_port_count = graph
                            .neighbors_directed(output_variant_node_index, petgraph::Outgoing)
                            // Only filter ports that are in the paths.
                            .filter(|output_port_node_index| node_indices.contains(output_port_node_index))
                            .count() as isize;

                        // Subtract 1 because if there's only 1 output port, there's no concurrency.
                        output_port_count - 1
                    })
                    .fold(0, isize::max);

                // Add 1 as we only want to consume concurrency if there's more than 1 input port.
                concurrency_sum + max_tool_concurrency + 1
            }
            // Input ports reduce concurrency.
            GraphNode::InputPort { .. } => concurrency_sum - 1,
            _ => concurrency_sum,
        }
    });

    // If the net concurrency is 0, the graph follows the concurrency rules.
    net_concurrency
}

fn root_set_batch_width(mask_size_bytes: usize, budget_bytes: usize) -> usize {
    // Keep a nonzero batch width for empty graphs and masks larger than the budget.
    // An oversized mask is processed alone, alongside the shared traversal scratch.
    if mask_size_bytes == 0 || mask_size_bytes > budget_bytes {
        return 1;
    }
    budget_bytes / mask_size_bytes
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum GraphNode {
    InputPort {
        tool_node_name: String,
        port_name: String,
    },
    Tool {
        tool_node_name: String,
    },
    OutputVariant {
        tool_node_name: String,
        variant_name: String,
    },
    OutputPort {
        tool_node_name: String,
        variant_name: String,
        port_name: String,
    },
}

impl std::fmt::Display for GraphNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphNode::InputPort {
                tool_node_name,
                port_name,
            } => {
                write!(formatter, "Input port: {tool_node_name}.{port_name}")
            }
            // Keep public diagnostics in terms of DAG vertices.
            GraphNode::Tool { tool_node_name } => write!(formatter, "Vertex: {tool_node_name}"),
            GraphNode::OutputVariant {
                tool_node_name,
                variant_name,
            } => {
                write!(formatter, "Output variant: {tool_node_name}.{variant_name}")
            }
            GraphNode::OutputPort {
                tool_node_name,
                variant_name,
                port_name,
            } => {
                write!(
                    formatter,
                    "Output port: {tool_node_name}.{variant_name}.{port_name}"
                )
            }
        }
    }
}

/// [`DagSpec`] to [petgraph::graph::DiGraph]. Also performs structure checks on the
/// graph.
fn try_into_graph(dag: &DagSpec) -> AnyResult<GraphAndToolEntryGroupNames> {
    let mut graph = DiGraph::<GraphNode, EdgeKind>::new();

    // Build a hash map of graph nodes that are part of entry groups. If there
    // are no entry groups specified, we assume all vertices that have entry
    // ports specified are part of the default entry group.
    let mut entry_group_names_by_tool_node: HashMap<GraphNode, Vec<String>> = HashMap::new();

    if !dag.entry_groups.is_empty() {
        for entry_group in &dag.entry_groups {
            for vertex_name in &entry_group.vertices {
                let tool_node = GraphNode::Tool {
                    tool_node_name: vertex_name.clone(),
                };

                // Check that the group references a vertex that exists.
                if !dag
                    .vertices
                    .iter()
                    .any(|vertex| &vertex.name == vertex_name)
                {
                    bail!(
                        "Entry group '{}' references a non-existing vertex '{vertex_name}'.",
                        entry_group.name
                    );
                }

                let mut entry_group_names = entry_group_names_by_tool_node
                    .remove(&tool_node)
                    .unwrap_or_default();
                entry_group_names.push(entry_group.name.clone());
                entry_group_names_by_tool_node.insert(tool_node, entry_group_names);
            }
        }

        // Entry ports are only published for entry group members, so the
        // ports of a vertex outside every group would be dropped silently.
        for vertex in &dag.vertices {
            let tool_node = GraphNode::Tool {
                tool_node_name: vertex.name.clone(),
            };

            if !vertex.entry_ports.is_empty()
                && !entry_group_names_by_tool_node.contains_key(&tool_node)
            {
                bail!("'{tool_node}' has entry ports but is not a member of any entry group.");
            }
        }
    } else {
        // If there are no entry groups, all vertices that have entry ports
        // specified are part of the default entry group.
        for vertex in &dag.vertices {
            if vertex.entry_ports.is_empty() {
                continue;
            }

            let tool_node = GraphNode::Tool {
                tool_node_name: vertex.name.clone(),
            };

            entry_group_names_by_tool_node.insert(tool_node, vec![DEFAULT_ENTRY_GROUP.to_string()]);
        }
    }

    // Check that there is at least one entry point.
    if entry_group_names_by_tool_node.is_empty() {
        bail!("The DAG has no entry vertices or ports.");
    }

    // Edges are always between an output port and an input port. We also
    // need to create edges between the tool, the output variant and the
    // output port if they don't exist yet.
    let mut node_index_by_node: HashMap<GraphNode, NodeIndex> = HashMap::new();

    for edge in &dag.edges {
        let origin_tool_node = GraphNode::Tool {
            tool_node_name: edge.from.vertex.clone(),
        };

        let output_variant_node = GraphNode::OutputVariant {
            tool_node_name: edge.from.vertex.clone(),
            variant_name: edge.from.output_variant.clone(),
        };

        let output_port_node = GraphNode::OutputPort {
            tool_node_name: edge.from.vertex.clone(),
            variant_name: edge.from.output_variant.clone(),
            port_name: edge.from.output_port.clone(),
        };

        // To avoid looping, we create a "phantom" destination tool node for
        // do-while edges.
        let destination_tool_node_name = match edge.kind {
            EdgeKind::DoWhile => format!("{}-do-while", edge.to.vertex),
            _ => edge.to.vertex.clone(),
        };

        let destination_tool_node = GraphNode::Tool {
            tool_node_name: destination_tool_node_name.clone(),
        };

        let input_port_node = GraphNode::InputPort {
            tool_node_name: destination_tool_node_name.clone(),
            port_name: edge.to.input_port.clone(),
        };

        // Create nodes if they don't exist yet.
        let origin_tool_node_index = node_index_by_node
            .get(&origin_tool_node)
            .copied()
            .unwrap_or_else(|| {
                let tool_node_index = graph.add_node(origin_tool_node.clone());

                node_index_by_node.insert(origin_tool_node.clone(), tool_node_index);

                tool_node_index
            });

        let output_variant_node_index = node_index_by_node
            .get(&output_variant_node)
            .copied()
            .unwrap_or_else(|| {
                let output_variant_node_index = graph.add_node(output_variant_node.clone());

                node_index_by_node.insert(output_variant_node.clone(), output_variant_node_index);

                output_variant_node_index
            });

        let output_port_node_index = node_index_by_node
            .get(&output_port_node)
            .copied()
            .unwrap_or_else(|| {
                let output_port_node_index = graph.add_node(output_port_node.clone());

                node_index_by_node.insert(output_port_node.clone(), output_port_node_index);

                output_port_node_index
            });

        let destination_tool_node_index = node_index_by_node
            .get(&destination_tool_node)
            .copied()
            .unwrap_or_else(|| {
                let tool_node_index = graph.add_node(destination_tool_node.clone());

                node_index_by_node.insert(destination_tool_node.clone(), tool_node_index);

                tool_node_index
            });

        let input_port_node_index = node_index_by_node
            .get(&input_port_node)
            .copied()
            .unwrap_or_else(|| {
                let input_port_node_index = graph.add_node(input_port_node.clone());

                node_index_by_node.insert(input_port_node.clone(), input_port_node_index);

                input_port_node_index
            });

        // Check that these edges don't already exist.
        if graph.contains_edge(output_variant_node_index, output_port_node_index) {
            bail!("Edge from '{output_variant_node}' to '{output_port_node}' already exists.",);
        }

        if graph.contains_edge(output_port_node_index, input_port_node_index) {
            bail!("Edge from '{output_port_node}' to '{input_port_node}' already exists.",);
        }

        // These are allowed.
        if !graph.contains_edge(origin_tool_node_index, output_variant_node_index) {
            graph.add_edge(
                origin_tool_node_index,
                output_variant_node_index,
                EdgeKind::Normal,
            );
        }

        if !graph.contains_edge(input_port_node_index, destination_tool_node_index) {
            graph.add_edge(
                input_port_node_index,
                destination_tool_node_index,
                EdgeKind::Normal,
            );
        }

        graph.add_edge(
            output_variant_node_index,
            output_port_node_index,
            EdgeKind::Normal,
        );
        graph.add_edge(output_port_node_index, input_port_node_index, edge.kind);
    }

    // Ensure we don't have duplicate vertices.
    let mut declared_tool_nodes = HashSet::new();
    let mut entry_port_nodes = HashSet::new();

    // Check that all normal vertices are in the graph and are unique.
    for vertex in &dag.vertices {
        let tool_node = GraphNode::Tool {
            tool_node_name: vertex.name.clone(),
        };

        // If the dag has no edges and only 1 vertex, we add this vertex as a
        // graph node.
        if dag.edges.is_empty() && dag.vertices.len() == 1 {
            let tool_node_index = graph.add_node(tool_node.clone());

            node_index_by_node.insert(tool_node.clone(), tool_node_index);
        }

        if !node_index_by_node.contains_key(&tool_node) {
            bail!("'{tool_node}' is not connected to the DAG.",);
        }

        if !declared_tool_nodes.insert(tool_node.clone()) {
            bail!("'{tool_node}' is defined multiple times.",);
        }
    }

    // Edges, default values and outputs may only name declared vertices;
    // publishing aborts on chain for any other name.
    for edge in &dag.edges {
        for vertex_name in [&edge.from.vertex, &edge.to.vertex] {
            if !declared_tool_nodes.contains(&GraphNode::Tool {
                tool_node_name: vertex_name.clone(),
            }) {
                bail!(
                    "Edge from '{}' to '{}' references a non-existing vertex '{vertex_name}'.",
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
            let entry_port_node = GraphNode::InputPort {
                tool_node_name: vertex.name.clone(),
                port_name: entry_port.name.clone(),
            };

            if node_index_by_node.contains_key(&entry_port_node) {
                bail!(
                    "'{entry_port_node}' has an edge leading to it and therefore cannot be an entry port.",
                );
            }

            if !entry_port_nodes.insert(entry_port_node.clone()) {
                bail!("'{entry_port_node}' is defined multiple times.",);
            }
        }
    }

    // Check that none of the default value input ports are in the graph.
    let mut default_input_port_nodes = HashSet::new();

    for default_value in &dag.default_values {
        let vertex_name = &default_value.vertex;
        let default_input_port_node = GraphNode::InputPort {
            tool_node_name: vertex_name.clone(),
            port_name: default_value.input_port.clone(),
        };

        if !declared_tool_nodes.contains(&GraphNode::Tool {
            tool_node_name: vertex_name.clone(),
        }) {
            bail!(
                "Default value for '{default_input_port_node}' references a non-existing vertex '{vertex_name}'."
            );
        }

        if node_index_by_node.contains_key(&default_input_port_node)
            || entry_port_nodes.contains(&default_input_port_node)
        {
            bail!(
                "'{default_input_port_node}' is an entry port or has an edge leading into it and therefore cannot have a default value.",
            );
        }

        if !default_input_port_nodes.insert(default_input_port_node.clone()) {
            bail!("'{default_input_port_node}' has multiple default values.");
        }
    }

    for output in &dag.outputs {
        if !declared_tool_nodes.contains(&GraphNode::Tool {
            tool_node_name: output.vertex.clone(),
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

        let tool_node = GraphNode::Tool {
            tool_node_name: vertex.name.clone(),
        };

        if has_outputs && has_edges {
            bail!("'{tool_node}' cannot have both outgoing edges and ports marked as output.");
        }
    }

    // Check that each vertex that has a do-while edge also has a break edge and vice versa.
    let do_while_vertex_names = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::DoWhile)
        .map(|edge| edge.from.vertex.clone())
        .collect::<HashSet<_>>();

    let break_vertex_names = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Break)
        .map(|edge| edge.from.vertex.clone())
        .collect::<HashSet<_>>();

    for vertex_name in &do_while_vertex_names {
        if !break_vertex_names.contains(vertex_name) {
            bail!("Vertex '{vertex_name}' has a do-while edge but no corresponding break edge.");
        }
    }

    for vertex_name in &break_vertex_names {
        if !do_while_vertex_names.contains(vertex_name) {
            bail!("Vertex '{vertex_name}' has a break edge but no corresponding do-while edge.");
        }
    }

    // Check that do-whiles and breaks always branch.
    let do_while_vertex_variant_name_pairs = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::DoWhile)
        .map(|edge| (edge.from.vertex.clone(), edge.from.output_variant.clone()))
        .collect::<HashSet<_>>();

    let break_vertex_variant_name_pairs = dag
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Break)
        .map(|edge| (edge.from.vertex.clone(), edge.from.output_variant.clone()))
        .collect::<HashSet<_>>();

    // Each pair contains a vertex name and an output-variant name.
    if let Some(vertex_variant_name_pair) = do_while_vertex_variant_name_pairs
        .intersection(&break_vertex_variant_name_pairs)
        .next()
    {
        let conflicting_output_variant_node = GraphNode::OutputVariant {
            tool_node_name: vertex_variant_name_pair.0.clone(),
            variant_name: vertex_variant_name_pair.1.clone(),
        };

        bail!("'{conflicting_output_variant_node}' has both a do-while and a break edge, but they must branch.");
    }

    Ok((graph, entry_group_names_by_tool_node))
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

    fn parse(json_source: &str) -> Result<DagSpec, serde_json::Error> {
        crate::dag::json::parse_dag_spec(json_source)
    }

    fn serial_alternative_diamond_dag(stage_count: usize) -> DagSpec {
        let mut vertices = Vec::with_capacity(stage_count * 3 + 1);
        for stage_index in 0..=stage_count {
            vertices.push(test_tool_vertex(
                format!("split_{stage_index}"),
                stage_index == 0,
            ));
            if stage_index == stage_count {
                continue;
            }
            vertices.push(test_tool_vertex(format!("left_{stage_index}"), false));
            vertices.push(test_tool_vertex(format!("right_{stage_index}"), false));
        }

        let mut edges = Vec::with_capacity(stage_count * 8);
        for stage_index in 0..stage_count {
            let split_vertex_name = format!("split_{stage_index}");
            let left_vertex_name = format!("left_{stage_index}");
            let right_vertex_name = format!("right_{stage_index}");
            let next_split_vertex_name = format!("split_{}", stage_index + 1);

            for (variant_name, vertex_name) in
                [("left", &left_vertex_name), ("right", &right_vertex_name)]
            {
                edges.push(DagEdge {
                    from: DagOutput {
                        vertex: split_vertex_name.clone(),
                        output_variant: variant_name.to_string(),
                        output_port: "result".to_string(),
                    },
                    to: DagInput {
                        vertex: vertex_name.clone(),
                        input_port: "input".to_string(),
                    },
                    kind: EdgeKind::Normal,
                });
            }

            for vertex_name in [&left_vertex_name, &right_vertex_name] {
                for variant_name in ["route_0", "route_1", "route_2"] {
                    edges.push(DagEdge {
                        from: DagOutput {
                            vertex: vertex_name.clone(),
                            output_variant: variant_name.to_string(),
                            output_port: "result".to_string(),
                        },
                        to: DagInput {
                            vertex: next_split_vertex_name.clone(),
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
                vertex: format!("split_{stage_count}"),
                output_variant: "done".to_string(),
                output_port: "result".to_string(),
            }],
            ..DagSpec::default()
        }
    }

    fn test_tool_vertex(vertex_name: String, is_entry_vertex: bool) -> DagVertex {
        DagVertex {
            kind: DagVertexKind::OffChain {
                tool_fqn: crate::fqn!("com.test.test@1"),
            },
            name: vertex_name,
            entry_ports: if is_entry_vertex {
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

    fn normal_test_edge(
        source_vertex_name: &str,
        target_vertex_name: &str,
        input_port_name: &str,
    ) -> DagEdge {
        DagEdge {
            from: DagOutput {
                vertex: source_vertex_name.to_string(),
                output_variant: "route".to_string(),
                output_port: "result".to_string(),
            },
            to: DagInput {
                vertex: target_vertex_name.to_string(),
                input_port: input_port_name.to_string(),
            },
            kind: EdgeKind::Normal,
        }
    }

    fn shared_sink_graph(
        root_tool_node_names: &[&str],
        root_connection_flags: &[bool],
        minimum_node_bound: usize,
    ) -> (
        DiGraph<GraphNode, EdgeKind>,
        HashMap<String, NodeIndex>,
        NodeIndex,
    ) {
        assert_eq!(root_tool_node_names.len(), root_connection_flags.len());
        let mut graph = DiGraph::new();
        let sink_node_index = graph.add_node(GraphNode::InputPort {
            tool_node_name: "sink".to_string(),
            port_name: "input".to_string(),
        });
        let mut root_tool_node_index_by_name = HashMap::new();
        for (&tool_node_name, &is_connected_to_sink) in
            root_tool_node_names.iter().zip(root_connection_flags)
        {
            let tool_node_index = graph.add_node(GraphNode::Tool {
                tool_node_name: tool_node_name.to_string(),
            });
            let output_variant_node_index = graph.add_node(GraphNode::OutputVariant {
                tool_node_name: tool_node_name.to_string(),
                variant_name: "route".to_string(),
            });
            let output_port_node_index = graph.add_node(GraphNode::OutputPort {
                tool_node_name: tool_node_name.to_string(),
                variant_name: "route".to_string(),
                port_name: "result".to_string(),
            });
            graph.add_edge(tool_node_index, output_variant_node_index, EdgeKind::Normal);
            graph.add_edge(
                output_variant_node_index,
                output_port_node_index,
                EdgeKind::Normal,
            );
            if is_connected_to_sink {
                graph.add_edge(output_port_node_index, sink_node_index, EdgeKind::Normal);
            }
            root_tool_node_index_by_name.insert(tool_node_name.to_string(), tool_node_index);
        }
        while graph.node_bound() < minimum_node_bound {
            graph.add_node(GraphNode::Tool {
                tool_node_name: format!("padding_{}", graph.node_count()),
            });
        }
        (graph, root_tool_node_index_by_name, sink_node_index)
    }

    #[test]
    fn entry_root_sets_are_sorted_unique_and_share_exact_regions() {
        let (graph, root_tool_node_index_by_name, _) =
            shared_sink_graph(&["a", "b", "c"], &[true, true, false], 0);
        let ordered_group_name = "ordered_ab".to_string();
        let reversed_group_name = "reversed_ab".to_string();
        let duplicate_group_name = "duplicate_ab".to_string();
        let control_group_name = "control_bc".to_string();
        let mut entry_group_names_by_tool_node = HashMap::new();
        entry_group_names_by_tool_node.insert(
            graph[root_tool_node_index_by_name["a"]].clone(),
            vec![
                ordered_group_name.clone(),
                reversed_group_name.clone(),
                duplicate_group_name.clone(),
                duplicate_group_name.clone(),
            ],
        );
        entry_group_names_by_tool_node.insert(
            graph[root_tool_node_index_by_name["b"]].clone(),
            vec![
                ordered_group_name.clone(),
                reversed_group_name.clone(),
                duplicate_group_name.clone(),
                control_group_name.clone(),
            ],
        );
        entry_group_names_by_tool_node.insert(
            graph[root_tool_node_index_by_name["c"]].clone(),
            vec![control_group_name.clone()],
        );

        let mut analysis = ValidationAnalysis::new(graph, entry_group_names_by_tool_node);
        assert_eq!(analysis.entry_node_index_groups.len(), 2);
        let equivalent_group_names = HashSet::from([
            ordered_group_name.as_str(),
            reversed_group_name.as_str(),
            duplicate_group_name.as_str(),
        ]);
        let equivalent_node_group = analysis
            .entry_node_index_groups
            .iter()
            .find(|(_, entry_group_name)| {
                equivalent_group_names.contains(entry_group_name.as_str())
            })
            .unwrap();
        let mut expected_root_tool_node_indices = vec![
            root_tool_node_index_by_name["a"],
            root_tool_node_index_by_name["b"],
        ];
        expected_root_tool_node_indices.sort_unstable();
        assert_eq!(equivalent_node_group.0, expected_root_tool_node_indices);
        assert!(equivalent_group_names.contains(equivalent_node_group.1.as_str()));
        let control_node_group = analysis
            .entry_node_index_groups
            .iter()
            .find(|(_, entry_group_name)| entry_group_name == &control_group_name)
            .unwrap();
        assert_ne!(equivalent_node_group.0, control_node_group.0);

        analysis.traversal_count = 0;
        let error = analysis
            .validate_concurrency_rules(REACHABILITY_MASK_BUDGET_BYTES)
            .unwrap_err();
        assert_eq!(analysis.traversal_count, 3);
        let error_message = error.to_string();
        assert!(error_message.contains("Input port: sink.input"));
        assert!(error_message.contains("has a race condition on it when invoking group"));
        assert!(equivalent_group_names
            .iter()
            .any(|entry_group_name| error_message
                .contains(&format!("invoking group '{entry_group_name}'"))));
        assert!(!error_message.contains("invoking group 'control_bc'"));
    }

    #[test]
    fn identical_entry_groups_share_forward_and_reverse_walks() {
        let (graph, root_tool_node_index_by_name, _) = shared_sink_graph(&["root"], &[true], 0);
        let entry_group_names = (0..32)
            .map(|group_index| format!("group_{group_index}"))
            .collect::<Vec<_>>();
        let entry_group_names_by_tool_node = HashMap::from([(
            graph[root_tool_node_index_by_name["root"]].clone(),
            entry_group_names.clone(),
        )]);
        let mut analysis = ValidationAnalysis::new(graph, entry_group_names_by_tool_node);
        assert_eq!(analysis.entry_node_index_groups.len(), 1);
        assert!(entry_group_names.contains(&analysis.entry_node_index_groups[0].1));

        analysis
            .validate_concurrency_rules(REACHABILITY_MASK_BUDGET_BYTES)
            .unwrap();

        assert_eq!(analysis.traversal_count, 2);
    }

    #[test]
    fn root_mask_batches_obey_budget_and_progress_for_oversized_masks() {
        let exact_budget_mask = ReachabilityMask::with_capacity(8_388_608);
        let exact_budget_mask_size_bytes = std::mem::size_of_val(exact_budget_mask.as_slice());
        assert_eq!(exact_budget_mask_size_bytes, REACHABILITY_MASK_BUDGET_BYTES);
        assert_eq!(
            root_set_batch_width(exact_budget_mask_size_bytes, REACHABILITY_MASK_BUDGET_BYTES),
            1
        );
        let oversized_mask = ReachabilityMask::with_capacity(8_388_609);
        let oversized_mask_size_bytes = std::mem::size_of_val(oversized_mask.as_slice());
        assert_eq!(
            oversized_mask_size_bytes,
            exact_budget_mask_size_bytes + std::mem::size_of_val(&oversized_mask.as_slice()[0])
        );
        assert!(oversized_mask_size_bytes > REACHABILITY_MASK_BUDGET_BYTES);
        assert_eq!(
            root_set_batch_width(oversized_mask_size_bytes, REACHABILITY_MASK_BUDGET_BYTES),
            1
        );

        let root_tool_node_names = ["r0", "r1", "r2", "r3", "r4"];
        let (graph, root_tool_node_index_by_name, _) =
            shared_sink_graph(&root_tool_node_names, &[true; 5], 65);
        let entry_group_names_by_tool_node = root_tool_node_names
            .iter()
            .enumerate()
            .map(|(group_index, tool_node_name)| {
                (
                    graph[root_tool_node_index_by_name[*tool_node_name]].clone(),
                    vec![format!("group_{group_index}")],
                )
            })
            .collect::<HashMap<_, _>>();
        let mut analysis = ValidationAnalysis::new(graph, entry_group_names_by_tool_node);
        assert_eq!(analysis.entry_node_index_groups.len(), 5);
        let mask_size_bytes = std::mem::size_of_val(analysis.reachability_mask.as_slice());
        assert_eq!(mask_size_bytes, 16);
        assert_eq!(root_set_batch_width(16, 32), 2);
        assert_eq!(root_set_batch_width(16, 8), 1);
        assert!(root_set_batch_width(16, 32) > 0);

        analysis.traversal_count = 0;
        analysis.validate_concurrency_rules(32).unwrap();
        assert_eq!(analysis.traversal_count, 8);

        analysis.traversal_count = 0;
        analysis.validate_concurrency_rules(8).unwrap();
        assert_eq!(analysis.traversal_count, 10);
    }

    #[test]
    fn indexed_for_each_rejects_unresolved_entry() {
        let graph = DiGraph::new();
        let missing_tool_node = GraphNode::Tool {
            tool_node_name: "missing".to_string(),
        };
        let entry_group_names_by_tool_node =
            HashMap::from([(missing_tool_node, vec!["group".to_string()])]);
        let error = ValidationAnalysis::new(graph, entry_group_names_by_tool_node)
            .validate_for_each_pairs()
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "'Vertex: missing' not found in graph nodes."
        );
    }

    #[test]
    fn repeated_do_while_target_is_traversed_once() {
        let mut graph = DiGraph::new();
        let target_tool_node_index = graph.add_node(GraphNode::Tool {
            tool_node_name: "target".to_string(),
        });
        let first_source_node_index = graph.add_node(GraphNode::OutputPort {
            tool_node_name: "loop_a".to_string(),
            variant_name: "loop".to_string(),
            port_name: "result".to_string(),
        });
        let second_source_node_index = graph.add_node(GraphNode::OutputPort {
            tool_node_name: "loop_b".to_string(),
            variant_name: "loop".to_string(),
            port_name: "result".to_string(),
        });
        let feedback_sink_node_index = graph.add_node(GraphNode::InputPort {
            tool_node_name: "target-do-while".to_string(),
            port_name: "feedback".to_string(),
        });
        graph.add_edge(
            target_tool_node_index,
            first_source_node_index,
            EdgeKind::Normal,
        );
        graph.add_edge(
            target_tool_node_index,
            second_source_node_index,
            EdgeKind::Normal,
        );
        graph.add_edge(
            first_source_node_index,
            feedback_sink_node_index,
            EdgeKind::DoWhile,
        );
        graph.add_edge(
            first_source_node_index,
            feedback_sink_node_index,
            EdgeKind::DoWhile,
        );
        graph.add_edge(
            second_source_node_index,
            feedback_sink_node_index,
            EdgeKind::DoWhile,
        );

        let entry_group_names_by_tool_node = HashMap::new();
        let mut analysis = ValidationAnalysis::new(graph, entry_group_names_by_tool_node);
        analysis.validate_do_while_nesting().unwrap();
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
        stage_count: usize,
        expected_vertex_count: usize,
        expected_edge_count: usize,
    ) {
        let dag = serial_alternative_diamond_dag(stage_count);
        assert_eq!(dag.vertices.len(), expected_vertex_count);
        assert_eq!(dag.edges.len(), expected_edge_count);
        let (expanded_graph, _) = try_into_graph(&dag).unwrap();
        let expanded_node_count = expanded_graph.node_count();
        let expanded_edge_count = expanded_graph.edge_count();
        assert_eq!(expanded_node_count, stage_count * 22 + 1);
        assert_eq!(expanded_edge_count, stage_count * 27);

        let validation_started_at = Instant::now();
        let validation_result = validate(&dag);
        let validation_duration = validation_started_at.elapsed();
        eprintln!(
            "stress stages={stage_count} logical_tools={} logical_edges={} expanded_nodes={expanded_node_count} expanded_edges={expanded_edge_count} routes=6^{stage_count} validate_ms={}",
            dag.vertices.len(),
            dag.edges.len(),
            validation_duration.as_millis()
        );
        assert!(
            validation_result.is_ok(),
            "stress DAG rejected: {validation_result:?}"
        );
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
        let root_tool_node_index = graph.add_node(GraphNode::Tool {
            tool_node_name: "root".to_string(),
        });
        let shared_tool_node_index = graph.add_node(GraphNode::Tool {
            tool_node_name: "shared".to_string(),
        });
        let leaf_tool_node_index = graph.add_node(GraphNode::Tool {
            tool_node_name: "leaf".to_string(),
        });

        graph.add_edge(
            root_tool_node_index,
            shared_tool_node_index,
            EdgeKind::Normal,
        );
        graph.add_edge(
            root_tool_node_index,
            shared_tool_node_index,
            EdgeKind::Normal,
        );
        let root_edge_indices = graph
            .edges(root_tool_node_index)
            .map(|edge| edge.id())
            .collect::<Vec<_>>();
        assert_eq!(root_edge_indices.len(), 2);
        *graph.edge_weight_mut(root_edge_indices[0]).unwrap() = EdgeKind::Normal;
        *graph.edge_weight_mut(root_edge_indices[1]).unwrap() = EdgeKind::ForEach;
        graph.add_edge(
            shared_tool_node_index,
            leaf_tool_node_index,
            EdgeKind::Collect,
        );

        let mut entry_group_names_by_tool_node = HashMap::new();
        entry_group_names_by_tool_node.insert(
            graph[root_tool_node_index].clone(),
            vec!["test_group".to_string()],
        );
        let error = ValidationAnalysis::new(graph, entry_group_names_by_tool_node)
            .validate_for_each_pairs()
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "'Vertex: shared' has a collect edge without for-each"
        );
    }

    fn synthetic_do_while_graph() -> DiGraph<GraphNode, EdgeKind> {
        let mut graph = DiGraph::new();
        let target_tool_node_index = graph.add_node(GraphNode::Tool {
            tool_node_name: "target".to_string(),
        });
        let source_node_index = graph.add_node(GraphNode::OutputPort {
            tool_node_name: "loop_source".to_string(),
            variant_name: "loop".to_string(),
            port_name: "result".to_string(),
        });
        let feedback_sink_node_index = graph.add_node(GraphNode::InputPort {
            tool_node_name: "target-do-while".to_string(),
            port_name: "feedback".to_string(),
        });
        graph.add_edge(
            source_node_index,
            feedback_sink_node_index,
            EdgeKind::DoWhile,
        );

        graph.add_edge(target_tool_node_index, source_node_index, EdgeKind::Normal);

        let outside_tool_node_index = graph.add_node(GraphNode::Tool {
            tool_node_name: "outside".to_string(),
        });
        graph.add_edge(
            target_tool_node_index,
            outside_tool_node_index,
            EdgeKind::ForEach,
        );
        graph
    }

    #[test]
    fn do_while_checks_forbidden_edges_leaving_the_path_region() {
        let graph = synthetic_do_while_graph();
        let entry_group_names_by_tool_node = HashMap::new();
        let mut analysis = ValidationAnalysis::new(graph, entry_group_names_by_tool_node);
        let error = analysis.validate_do_while_nesting().unwrap_err();
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

        let validation_result = validate(&dag);
        assert!(
            validation_result.is_ok(),
            "unexpected validation error: {validation_result:?}"
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
        let default_input_data = NexusData::from_json_value(&dag.default_values[0].value).unwrap();
        assert_eq!(
            default_input_data.to_json_value().unwrap(),
            dag.default_values[0].value,
        );
    }

    #[test]
    fn test_immediately_converges_valid() {
        let dag = parse(include_str!("_dags/immediately_converges_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_immediately_converges_invalid() {
        let dag = parse(include_str!("_dags/immediately_converges_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: b.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_intertwined_valid() {
        let dag = parse(include_str!("_dags/intertwined_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn test_intertwined_invalid() {
        let dag = parse(include_str!("_dags/intertwined_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
    }

    #[test]
    fn test_multiple_output_ports_invalid() {
        let dag = parse(include_str!("_dags/multiple_output_ports_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
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

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: c.1' has a race condition on it when invoking group 'group_a'"));
    }

    #[test]
    fn test_branched_net_zero_invalid() {
        let dag = parse(include_str!("_dags/branched_net_zero_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: d.1' has a race condition on it when invoking group '_default_group'"));
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

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: e.2' is unreachable when invoking group 'group_b'"));
    }

    #[test]
    fn test_entry_groups_tm_invalid() {
        let dag = parse(include_str!("_dags/entry_groups_tm_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: e.2' has a race condition on it when invoking group 'group_b'"));
    }

    #[test]
    fn test_entry_groups_unlisted_entry_ports_invalid() {
        let dag = parse(include_str!(
            "_dags/entry_groups_unlisted_entry_ports_invalid.json"
        ))
        .unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Vertex: b' has entry ports but is not a member of any entry group."));
    }

    #[test]
    fn both_loops_valid() {
        let dag = parse(include_str!("_dags/both_loops_valid.json")).unwrap();

        assert!(validate(&dag).is_ok());
    }

    #[test]
    fn missing_do_while_invalid() {
        let dag = parse(include_str!("_dags/missing_do_while_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("Vertex 'until_11' has a break edge but no corresponding do-while edge."));
    }

    #[test]
    fn missing_break_invalid() {
        let dag = parse(include_str!("_dags/missing_break_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("Vertex 'until_11' has a do-while edge but no corresponding break edge."));
    }

    #[test]
    fn collect_without_for_each_invalid() {
        let dag = parse(include_str!("_dags/collect_without_for_each_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Output port: a.ok.result' has a collect edge without for-each"));
    }

    #[test]
    fn nested_for_each_invalid() {
        let dag = parse(include_str!("_dags/nested_for_each_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Output port: b.ok.result' has a nested for-each"));
    }

    #[test]
    fn do_while_in_for_each_invalid() {
        let dag = parse(include_str!("_dags/do_while_in_for_each_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Output port: b.also_ok.loop' has a do-while or break edge inside a for-each"));
    }

    #[test]
    fn unclosed_for_each_invalid() {
        let dag = parse(include_str!("_dags/unclosed_for_each_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Vertex: b' has a for-each edge without a corresponding collect"));
    }

    #[test]
    fn do_while_and_break_same_variant_invalid() {
        let dag = parse(include_str!(
            "_dags/do_while_and_break_same_variant_invalid.json"
        ))
        .unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Output variant: b.ok' has both a do-while and a break edge, but they must branch."));
    }

    #[test]
    fn do_while_nested_invalid() {
        let dag = parse(include_str!("_dags/do_while_nested_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("Do-while edge at 'Output port: c.also_ok.loop' has a nested loop inside."));
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

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: a.input' has an edge leading to it and therefore cannot be an entry port."));
    }

    // == Parser tests ==

    #[test]
    fn test_undefined_connections_invalid() {
        let dag = parse(include_str!("_dags/undefined_connections_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Vertex: a' is not connected to the DAG."));
    }

    #[test]
    fn test_edge_references_non_existing_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/edge_references_non_existing_vertex_invalid.json"
        ))
        .unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("Edge from 'a' to 'b' references a non-existing vertex 'b'."));
    }

    #[test]
    fn test_default_value_references_non_existing_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/default_value_references_non_existing_vertex_invalid.json"
        ))
        .unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("Default value for 'Input port: b.input' references a non-existing vertex 'b'."));
    }

    #[test]
    fn test_default_value_defined_twice_invalid() {
        let dag = parse(include_str!(
            "_dags/default_value_defined_twice_invalid.json"
        ))
        .unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: a.extra' has multiple default values."));
    }

    #[test]
    fn test_output_references_non_existing_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/output_references_non_existing_vertex_invalid.json"
        ))
        .unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("Output 'b.ok.output' references a non-existing vertex 'b'."));
    }

    #[test]
    fn test_output_and_edge_invalid() {
        let dag = parse(include_str!("_dags/output_and_edge_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Vertex: a' cannot have both outgoing edges and ports marked as output."));
    }

    #[test]
    fn test_def_val_on_input_port_invalid() {
        let dag = parse(include_str!("_dags/has_def_on_input_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Input port: location_decider.context' is an entry port or has an edge leading into it and therefore cannot have a default value."));
    }

    #[test]
    fn test_references_non_existing_vertex() {
        let dag = parse(include_str!("_dags/references_non_existing_vertex.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("Entry group 'group_a' references a non-existing vertex 'invalid'."));
    }

    #[test]
    fn test_empty_invalid() {
        let dag = parse(include_str!("_dags/empty_invalid.json")).unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("The DAG has no entry vertices or ports."));
    }

    #[test]
    fn test_both_vertex_and_entry_vertex_invalid() {
        let dag = parse(include_str!(
            "_dags/both_vertex_and_entry_vertex_invalid.json"
        ))
        .unwrap();

        let validation_result = validate(&dag);

        assert_matches!(validation_result, Err(error) if error.to_string().contains("'Vertex: b' is defined multiple times."));
    }
}
