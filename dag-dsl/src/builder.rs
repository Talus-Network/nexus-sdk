//! Relaxed, flat [`DagBuilder`] — string-keyed ports, validation at
//! [`DagBuilder::build`]. Works with any tool without requiring a typed
//! descriptor. See the crate-level docs for the strict counterpart.

use {
    crate::error::DagError,
    nexus_sdk::{
        types::{
            Dag,
            Data,
            DefaultValue,
            Edge,
            EdgeKind,
            EntryGroup,
            EntryPort,
            FromPort,
            StorageKind,
            ToPort,
            Vertex,
            VertexKind,
        },
        ToolFqn,
    },
    std::collections::HashSet,
};

// ---------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------

/// Handle returned when a vertex is added to the builder.
///
/// Used to construct [`OutPortRef`] / [`InPortRef`] references without
/// retyping the vertex name, and passed to builder methods that refer back
/// to the vertex (`entry_port`, `default_value`, etc.). `VertexRef` is
/// [`Clone`]; keep a copy as long as needed.
#[derive(Clone, Debug)]
pub struct VertexRef {
    name: String,
}

impl VertexRef {
    /// Construct a handle referring to a vertex by name.
    ///
    /// Useful when the original handle returned from [`DagBuilder::offchain`]
    /// or [`DagBuilder::onchain`] is not in scope (e.g. when the DAG is
    /// assembled across helper functions). The handle itself does not
    /// guarantee the vertex exists in any builder — a dangling handle
    /// surfaces as an "unknown vertex" [`DagError`] at
    /// [`DagBuilder::build`].
    ///
    /// [`DagError`]: crate::DagError
    pub fn named(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Construct a reference to an output variant + port on this vertex.
    ///
    /// Port types are not checked by the relaxed builder; see
    /// [`crate::TypedDagBuilder`] for compile-time type safety.
    pub fn out(&self, variant: impl Into<String>, port: impl Into<String>) -> OutPortRef {
        OutPortRef {
            vertex: self.name.clone(),
            variant: variant.into(),
            port: port.into(),
        }
    }

    /// Construct a reference to an input port on this vertex.
    pub fn inp(&self, port: impl Into<String>) -> InPortRef {
        InPortRef {
            vertex: self.name.clone(),
            port: port.into(),
        }
    }

    /// The wire-format name of this vertex (as registered in the DAG's
    /// `vertices` array).
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Reference to a vertex's output port on a specific variant.
///
/// Produced by [`VertexRef::out`] and consumed by the edge-creating methods
/// on [`DagBuilder`].
#[derive(Clone, Debug)]
pub struct OutPortRef {
    pub(crate) vertex: String,
    pub(crate) variant: String,
    pub(crate) port: String,
}

impl From<OutPortRef> for FromPort {
    fn from(p: OutPortRef) -> Self {
        FromPort {
            vertex: p.vertex,
            output_variant: p.variant,
            output_port: p.port,
        }
    }
}

/// Reference to a vertex's input port.
///
/// Produced by [`VertexRef::inp`] and consumed by the edge-creating methods
/// on [`DagBuilder`].
#[derive(Clone, Debug)]
pub struct InPortRef {
    pub(crate) vertex: String,
    pub(crate) port: String,
}

impl From<InPortRef> for ToPort {
    fn from(p: InPortRef) -> Self {
        ToPort {
            vertex: p.vertex,
            input_port: p.port,
        }
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Relaxed, flat builder for a [`Dag`].
///
/// Accumulate vertices, edges, entry groups, default values, and outputs;
/// then call [`DagBuilder::build`] to validate and emit a `Dag`.
///
/// - Port types are strings — no compile-time check on payload compatibility.
/// - Topology, edge-kind pairing, and reference validity are checked at
///   [`DagBuilder::build`] — both locally (duplicate names, unknown vertex
///   references) and by delegating to `nexus_sdk::dag::validator::validate`.
#[derive(Default, Debug)]
pub struct DagBuilder {
    vertices: Vec<Vertex>,
    edges: Vec<Edge>,
    default_values: Vec<DefaultValue>,
    entry_groups: Vec<EntryGroup>,
    outputs: Vec<FromPort>,
    // Used for fast duplicate-vertex detection during construction.
    vertex_names: HashSet<String>,
    // Monotonic counter used by the scoped layer to name anonymous scopes
    // (`foreach_N`, `dowhile_N`). Crate-private accessor below.
    scope_counter: u64,
}

impl DagBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next anonymous scope index — used by the scoped layer to
    /// name auto-generated loop scopes (`foreach_N`, `dowhile_N`). Returned
    /// indices are monotonic per-builder.
    pub(crate) fn next_scope_index(&mut self) -> u64 {
        let i = self.scope_counter;
        self.scope_counter += 1;
        i
    }

    /// Accessor for the accumulated vertices (useful for tests and
    /// inspection before [`DagBuilder::build`]).
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Accessor for the accumulated edges.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Add an off-chain tool vertex with the given name and FQN.
    ///
    /// Returns a handle ([`VertexRef`]) used for subsequent references
    /// (edges, default values, entry groups, outputs).
    pub fn offchain(&mut self, name: impl Into<String>, fqn: ToolFqn) -> VertexRef {
        let name = name.into();
        self.vertex_names.insert(name.clone());
        self.vertices.push(Vertex {
            name: name.clone(),
            kind: VertexKind::OffChain { tool_fqn: fqn },
            entry_ports: None,
        });
        VertexRef { name }
    }

    /// Add an on-chain tool vertex with the given name and FQN.
    pub fn onchain(&mut self, name: impl Into<String>, fqn: ToolFqn) -> VertexRef {
        let name = name.into();
        self.vertex_names.insert(name.clone());
        self.vertices.push(Vertex {
            name: name.clone(),
            kind: VertexKind::OnChain { tool_fqn: fqn },
            entry_ports: None,
        });
        VertexRef { name }
    }

    /// Declare an entry port on a previously-added vertex.
    ///
    /// Entry ports are those which must be supplied input when executing the
    /// DAG; each entry port belongs implicitly to the default entry group
    /// unless the vertex is placed in a named [`DagBuilder::entry_group`].
    ///
    /// Silently no-ops if the [`VertexRef`] does not correspond to any
    /// tracked vertex — this is only possible if the handle was fabricated
    /// outside the builder, which is not a supported use case.
    pub fn entry_port(&mut self, vertex: &VertexRef, port: impl Into<String>) -> &mut Self {
        let port = port.into();
        if let Some(v) = self.vertices.iter_mut().find(|v| v.name == vertex.name) {
            v.entry_ports
                .get_or_insert_with(Vec::new)
                .push(EntryPort { name: port });
        }
        self
    }

    /// Add a normal (data-flow) edge.
    pub fn edge(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.push_edge(from, to, EdgeKind::Normal)
    }

    /// Add a for-each edge — the destination vertex runs once per item in
    /// the source port's collection.
    pub fn edge_for_each(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.push_edge(from, to, EdgeKind::ForEach)
    }

    /// Add a collect edge — gathers per-iteration outputs back into a
    /// collection at the destination port.
    pub fn edge_collect(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.push_edge(from, to, EdgeKind::Collect)
    }

    /// Add a do-while edge — loops back, re-executing the destination while
    /// the source port condition holds.
    pub fn edge_do_while(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.push_edge(from, to, EdgeKind::DoWhile)
    }

    /// Add a break edge — exits a do-while loop.
    pub fn edge_break(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.push_edge(from, to, EdgeKind::Break)
    }

    /// Add a static edge — supplies a value from outside a loop body into
    /// the loop.
    pub fn edge_static(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.push_edge(from, to, EdgeKind::Static)
    }

    fn push_edge(&mut self, from: OutPortRef, to: InPortRef, kind: EdgeKind) -> &mut Self {
        self.edges.push(Edge {
            from: from.into(),
            to: to.into(),
            kind,
        });
        self
    }

    /// Declare an entry group containing the given vertex names.
    ///
    /// All entry ports of listed vertices must be supplied when the DAG is
    /// executed via that group. Omit to let every entry port default to the
    /// implicit default group (see `DEFAULT_ENTRY_GROUP`).
    pub fn entry_group<S>(
        &mut self,
        name: impl Into<String>,
        vertices: impl IntoIterator<Item = S>,
    ) -> &mut Self
    where
        S: Into<String>,
    {
        self.entry_groups.push(EntryGroup {
            name: name.into(),
            vertices: vertices.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// Provide a default (pre-configured) value for the given input port on
    /// a vertex.
    pub fn default_value(
        &mut self,
        vertex: &VertexRef,
        input_port: impl Into<String>,
        value: Data,
    ) -> &mut Self {
        self.default_values.push(DefaultValue {
            vertex: vertex.name.clone(),
            input_port: input_port.into(),
            value,
        });
        self
    }

    /// Convenience: provide an inline JSON default for the given input
    /// port.
    pub fn inline_default(
        &mut self,
        vertex: &VertexRef,
        input_port: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> &mut Self {
        self.default_value(
            vertex,
            input_port,
            Data {
                storage: StorageKind::Inline,
                data: value.into(),
            },
        )
    }

    /// Convenience: provide a Walrus-backed default for the given input
    /// port.
    pub fn walrus_default(
        &mut self,
        vertex: &VertexRef,
        input_port: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> &mut Self {
        self.default_value(
            vertex,
            input_port,
            Data {
                storage: StorageKind::Walrus,
                data: value.into(),
            },
        )
    }

    /// Mark the given vertex output port as an output of the DAG.
    pub fn output(&mut self, port: OutPortRef) -> &mut Self {
        self.outputs.push(port.into());
        self
    }

    /// Finalize the builder — run local consistency checks, then delegate
    /// to `nexus_sdk::dag::validator::validate` for wire-level rule
    /// checking (acyclicity, for-each/collect pairing, do-while nesting,
    /// concurrency, etc.).
    ///
    /// All locally-detectable problems are aggregated and returned together
    /// — one error per discovered issue. Wire-level validation stops on
    /// first failure (a limitation of the existing sdk validator) and is
    /// surfaced as a single [`DagError::WireValidation`].
    pub fn build(self) -> Result<Dag, Vec<DagError>> {
        let mut errors = Vec::new();

        // Duplicate vertex names are tracked at add-time via `vertex_names`,
        // but the HashSet only catches first insertion. Re-scan the
        // vertices vec to surface duplicates that actually made it in.
        let mut seen: HashSet<&str> = HashSet::new();
        for v in &self.vertices {
            if !seen.insert(v.name.as_str()) {
                errors.push(DagError::DuplicateVertex {
                    name: v.name.clone(),
                });
            }
        }

        // Edge references
        for e in &self.edges {
            if !self.vertex_names.contains(&e.from.vertex) {
                errors.push(DagError::EdgeFromUnknownVertex {
                    vertex: e.from.vertex.clone(),
                    variant: e.from.output_variant.clone(),
                    port: e.from.output_port.clone(),
                });
            }
            if !self.vertex_names.contains(&e.to.vertex) {
                errors.push(DagError::EdgeToUnknownVertex {
                    vertex: e.to.vertex.clone(),
                    port: e.to.input_port.clone(),
                });
            }
        }

        // Entry group references
        for g in &self.entry_groups {
            for v in &g.vertices {
                if !self.vertex_names.contains(v) {
                    errors.push(DagError::EntryGroupUnknownVertex {
                        group: g.name.clone(),
                        vertex: v.clone(),
                    });
                }
            }
        }

        // Default value references
        for d in &self.default_values {
            if !self.vertex_names.contains(&d.vertex) {
                errors.push(DagError::DefaultValueUnknownVertex {
                    vertex: d.vertex.clone(),
                    port: d.input_port.clone(),
                });
            }
        }

        // Output references
        for o in &self.outputs {
            if !self.vertex_names.contains(&o.vertex) {
                errors.push(DagError::OutputUnknownVertex {
                    vertex: o.vertex.clone(),
                    variant: o.output_variant.clone(),
                    port: o.output_port.clone(),
                });
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let dag = Dag {
            vertices: self.vertices,
            edges: self.edges,
            default_values: none_if_empty(self.default_values),
            entry_groups: none_if_empty(self.entry_groups),
            outputs: none_if_empty(self.outputs),
        };

        // Delegate wire-level checks to the existing sdk validator. It
        // takes `Dag` by value; `Dag` is Clone.
        if let Err(e) = nexus_sdk::dag::validator::validate(dag.clone()) {
            return Err(vec![DagError::WireValidation {
                message: e.to_string(),
            }]);
        }

        Ok(dag)
    }
}

fn none_if_empty<T>(v: Vec<T>) -> Option<Vec<T>> {
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}
