//! This module contains a struct representation of the Nexus DAG JSON file.
//! First line of validation. If try_from fails, there is an error in the
//! configuration and vice versa, if it succeeds, we should be certain that the
//! configuration structure is correct.
//!
//! # Example
//!
//! ```no_run
//! let graph: Dag = include_str!("./_dags/dead_ends_valid.json").try_into()?;
//!
//! assert!(graph.is_ok());
//! ```

use {crate::prelude::*, petgraph::graph::DiGraph, std::collections::HashMap};

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Dag {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) edges: Vec<Edge>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Vertex {
    pub(crate) name: String,
    pub(crate) input_ports: Vec<Port>,
    pub(crate) output_variants: Vec<OutputVariant>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum VertexType {
    InputPort,
    InputPortWithDefault,
    Tool,
    OutputVariant,
    OutputPort,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Port {
    pub(crate) name: String,
    pub(crate) default: Option<Data>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct Data {
    pub(crate) r#type: String,
    pub(crate) storage: Option<Vec<u8>>,
    pub(crate) keys: Option<Vec<Vec<u8>>>,
    pub(crate) data: Option<Vec<Vec<u8>>>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct OutputVariant {
    pub(crate) name: String,
    pub(crate) output_ports: Option<Vec<Port>>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Edge {
    pub(crate) from: FromPort,
    pub(crate) to: ToPort,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FromPort {
    pub(crate) vertex: String,
    pub(crate) output_variant: String,
    pub(crate) output_port: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ToPort {
    pub(crate) vertex: String,
    pub(crate) input_port: String,
}

/// == Configuration Impls ==

impl TryFrom<&str> for Dag {
    type Error = AnyError;

    fn try_from(s: &str) -> AnyResult<Self> {
        serde_json::from_str(s).map_err(AnyError::from)
    }
}

// Convert Configuration to DiGraph.
impl TryFrom<Dag> for DiGraph<VertexType, ()> {
    type Error = AnyError;

    fn try_from(configuration: Dag) -> AnyResult<Self> {
        let mut graph = DiGraph::<VertexType, ()>::new();

        // We need to associate the petgraph index with our vertex path.
        let mut vertex_indices = HashMap::new();

        // We add each input port, tool, output variant and output port as a node in the graph.
        for vertex in &configuration.vertices {
            // Add the tool node to the graph.
            let tool_node = graph.add_node(VertexType::Tool);

            // Add each input port to the graph and save the input port name to node index mapping as we will need this
            // to add edges.
            for input_port in &vertex.input_ports {
                let input_port_node = graph.add_node(match input_port.default {
                    Some(_) => VertexType::InputPortWithDefault,
                    None => VertexType::InputPort,
                });

                graph.add_edge(input_port_node, tool_node, ());

                vertex_indices.insert(
                    (vertex.name.clone(), None, input_port.name.clone()),
                    input_port_node,
                );
            }

            // Add each output variant to the graph.
            for output_variant in &vertex.output_variants {
                let output_variant_node = graph.add_node(VertexType::OutputVariant);

                graph.add_edge(tool_node, output_variant_node, ());

                let Some(output_ports) = &output_variant.output_ports else {
                    continue;
                };

                // Add each output port to the graph and save the output port name to node index mapping as we will need
                // this to add edges.
                for output_port in output_ports {
                    let output_port_node = graph.add_node(VertexType::OutputPort);

                    graph.add_edge(output_variant_node, output_port_node, ());

                    vertex_indices.insert(
                        (
                            vertex.name.clone(),
                            Some(output_variant.name.clone()),
                            output_port.name.clone(),
                        ),
                        output_port_node,
                    );
                }
            }
        }

        // Add edges between tools to our graph.
        for edge in &configuration.edges {
            let from = vertex_indices
                .get(&(
                    edge.from.vertex.clone(),
                    Some(edge.from.output_variant.clone()),
                    edge.from.output_port.clone(),
                ))
                .ok_or_else(|| anyhow!("The vertex {:?} is not defined.", edge.from))?;

            let to = vertex_indices
                .get(&(edge.to.vertex.clone(), None, edge.to.input_port.clone()))
                .ok_or_else(|| anyhow!("The vertex {:?} is not defined.", edge.to))?;

            graph.add_edge(*from, *to, ());
        }

        Ok(graph)
    }
}
