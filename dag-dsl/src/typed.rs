//! Strict, flat layer of the DSL.
//!
//! Adds compile-time port-payload type safety on top of the relaxed
//! [`DagBuilder`]. Tool authors (or the `nexus-toolkit` derive macro, or
//! `nexus-dsl-codegen`) implement [`ToolDescriptor`] for their tool type;
//! the DSL user then says `dag.add::<MyTool>("name")` and receives a handle
//! whose `.inp` / `.out` fields expose typed port handles
//! ([`InPort<T>`] / [`OutPort<Variant, T>`]).
//!
//! Connections are made via the `connect` family (`connect`,
//! `connect_for_each`, `connect_collect`, `connect_do_while`,
//! `connect_break`, `connect_static`) — each method's generic bounds encode
//! the kind's arity (e.g. `connect_for_each` requires source `Vec<T>` and
//! destination `T`).
//!
//! For tools without descriptors, [`TypedDagBuilder::add_untyped`] produces
//! a handle that participates in topology checks but not in type checks,
//! and [`TypedDagBuilder::raw`] exposes the underlying relaxed
//! [`DagBuilder`] for full escape.

use {
    crate::{
        builder::{DagBuilder, InPortRef, OutPortRef, VertexRef},
        error::DagError,
    },
    nexus_sdk::{types::Dag, ToolFqn},
    std::marker::PhantomData,
};

// ---------------------------------------------------------------------------
// Variant markers
// ---------------------------------------------------------------------------

/// Marker for the default `ok` output variant.
pub struct Ok;
/// Marker for the default `err` output variant.
pub struct Err;

// ---------------------------------------------------------------------------
// Typed port handles
// ---------------------------------------------------------------------------

/// A typed input port handle — carries the target vertex name, port name,
/// and a phantom payload type `T` matched by `connect_*` methods.
#[derive(Debug)]
pub struct InPort<T> {
    vertex: String,
    port: String,
    _marker: PhantomData<fn(T)>,
}

impl<T> Clone for InPort<T> {
    fn clone(&self) -> Self {
        Self {
            vertex: self.vertex.clone(),
            port: self.port.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> InPort<T> {
    /// Construct a typed input port handle.
    ///
    /// Typically produced by a [`ToolDescriptor`] implementation rather
    /// than constructed directly by DSL users.
    pub fn new(vertex: impl Into<String>, port: impl Into<String>) -> Self {
        Self {
            vertex: vertex.into(),
            port: port.into(),
            _marker: PhantomData,
        }
    }

    /// Lower this typed handle to the relaxed [`InPortRef`], discarding the
    /// payload type parameter.
    pub fn untyped(self) -> InPortRef {
        InPortRef {
            vertex: self.vertex,
            port: self.port,
        }
    }
}

/// A typed output port handle — carries the source vertex name, variant,
/// port name, and phantom `(Variant, T)` types matched by `connect_*`
/// methods.
#[derive(Debug)]
pub struct OutPort<V, T> {
    vertex: String,
    variant: String,
    port: String,
    _marker: PhantomData<fn(V, T)>,
}

impl<V, T> Clone for OutPort<V, T> {
    fn clone(&self) -> Self {
        Self {
            vertex: self.vertex.clone(),
            variant: self.variant.clone(),
            port: self.port.clone(),
            _marker: PhantomData,
        }
    }
}

impl<V, T> OutPort<V, T> {
    /// Construct a typed output port handle.
    ///
    /// Typically produced by a [`ToolDescriptor`] implementation rather
    /// than constructed directly by DSL users.
    pub fn new(
        vertex: impl Into<String>,
        variant: impl Into<String>,
        port: impl Into<String>,
    ) -> Self {
        Self {
            vertex: vertex.into(),
            variant: variant.into(),
            port: port.into(),
            _marker: PhantomData,
        }
    }

    /// Lower this typed handle to the relaxed [`OutPortRef`], discarding
    /// the variant and payload type parameters.
    pub fn untyped(self) -> OutPortRef {
        OutPortRef {
            vertex: self.vertex,
            variant: self.variant,
            port: self.port,
        }
    }
}

// ---------------------------------------------------------------------------
// Descriptor trait
// ---------------------------------------------------------------------------

/// A tool's contract for the strict DSL layer.
///
/// Implementors expose the tool's FQN plus two associated types: an
/// `Inputs` struct whose fields are [`InPort<T>`] handles (one per input
/// port) and an `Outputs` struct whose fields are per-variant handles,
/// each of which is a struct whose fields are [`OutPort<Variant, T>`]
/// handles (one per output port on that variant).
///
/// Three production paths produce descriptors, all interchangeable:
///
/// 1. Hand-written `impl ToolDescriptor for MyTool { ... }`.
/// 2. `#[derive(NexusTool)]` in `nexus-toolkit` (emits the impl alongside
///    the `Tool` impl).
/// 3. `nexus-dsl-codegen` reading a committed `tool-meta.json` artifact.
pub trait ToolDescriptor {
    /// Struct exposing one typed [`InPort<T>`] per input port of the tool.
    type Inputs;
    /// Struct exposing one per-variant handle (itself a struct exposing
    /// typed [`OutPort<Variant, T>`] handles).
    type Outputs;

    /// The canonical [`ToolFqn`] used to register the tool on-chain.
    fn fqn() -> ToolFqn;

    /// Construct the typed inputs view bound to a specific vertex name.
    fn inputs_for(vertex_name: &str) -> Self::Inputs;

    /// Construct the typed outputs view bound to a specific vertex name.
    fn outputs_for(vertex_name: &str) -> Self::Outputs;
}

// ---------------------------------------------------------------------------
// Vertex handles
// ---------------------------------------------------------------------------

/// Handle returned when a typed vertex is added via
/// [`TypedDagBuilder::add`] or [`TypedDagBuilder::add_onchain`].
///
/// The `inp` and `out` fields expose this vertex's typed ports; use them
/// as the arguments to `connect_*` methods.
pub struct TypedVertexRef<D: ToolDescriptor> {
    /// The vertex's wire-format name.
    pub name: String,
    /// Typed view of the vertex's input ports.
    pub inp: D::Inputs,
    /// Typed view of the vertex's output variants + ports.
    pub out: D::Outputs,
    _marker: PhantomData<fn() -> D>,
}

/// Handle for a vertex added via [`TypedDagBuilder::add_untyped`] — no
/// compile-time port-payload checks. Participates in topology and
/// edge-kind checks at [`TypedDagBuilder::build`].
#[derive(Clone, Debug)]
pub struct UntypedVertexRef {
    inner: VertexRef,
}

impl UntypedVertexRef {
    /// Construct a reference to an output variant + port on this vertex.
    pub fn out(&self, variant: impl Into<String>, port: impl Into<String>) -> OutPortRef {
        self.inner.out(variant, port)
    }

    /// Construct a reference to an input port on this vertex.
    pub fn inp(&self, port: impl Into<String>) -> InPortRef {
        self.inner.inp(port)
    }

    /// The wire-format name of this vertex.
    pub fn name(&self) -> &str {
        self.inner.name()
    }
}

// ---------------------------------------------------------------------------
// Typed builder
// ---------------------------------------------------------------------------

/// Strict, flat builder for a [`Dag`].
///
/// Wraps a [`DagBuilder`]; every typed operation lowers to the same edge
/// primitives. The type-level invariants are enforced by the `connect_*`
/// method signatures — a port-payload mismatch is a compile error.
#[derive(Default)]
pub struct TypedDagBuilder {
    inner: DagBuilder,
}

impl TypedDagBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self {
            inner: DagBuilder::new(),
        }
    }

    /// Add an off-chain tool vertex with a typed descriptor.
    pub fn add<D: ToolDescriptor>(&mut self, name: impl Into<String>) -> TypedVertexRef<D> {
        let name = name.into();
        self.inner.offchain(&name, D::fqn());
        TypedVertexRef {
            inp: D::inputs_for(&name),
            out: D::outputs_for(&name),
            name,
            _marker: PhantomData,
        }
    }

    /// Add an on-chain tool vertex with a typed descriptor.
    pub fn add_onchain<D: ToolDescriptor>(&mut self, name: impl Into<String>) -> TypedVertexRef<D> {
        let name = name.into();
        self.inner.onchain(&name, D::fqn());
        TypedVertexRef {
            inp: D::inputs_for(&name),
            out: D::outputs_for(&name),
            name,
            _marker: PhantomData,
        }
    }

    /// Escape hatch: add an off-chain tool vertex with no descriptor. The
    /// returned handle participates in topology and edge-kind checks but
    /// not in type checks.
    pub fn add_untyped(&mut self, name: impl Into<String>, fqn: ToolFqn) -> UntypedVertexRef {
        UntypedVertexRef {
            inner: self.inner.offchain(name, fqn),
        }
    }

    /// Escape hatch: add an on-chain tool vertex with no descriptor.
    pub fn add_onchain_untyped(
        &mut self,
        name: impl Into<String>,
        fqn: ToolFqn,
    ) -> UntypedVertexRef {
        UntypedVertexRef {
            inner: self.inner.onchain(name, fqn),
        }
    }

    /// Add a normal (data-flow) edge: source and destination must carry
    /// the same payload type `T`.
    pub fn connect<V, T>(&mut self, from: OutPort<V, T>, to: InPort<T>) -> &mut Self {
        self.inner.edge(from.untyped(), to.untyped());
        self
    }

    /// Add a for-each edge: source must be `OutPort<V, Vec<T>>`,
    /// destination must be `InPort<T>`. The destination vertex runs once
    /// per item.
    pub fn connect_for_each<V, T>(&mut self, from: OutPort<V, Vec<T>>, to: InPort<T>) -> &mut Self {
        self.inner.edge_for_each(from.untyped(), to.untyped());
        self
    }

    /// Add a collect edge: source must be `OutPort<V, T>`, destination
    /// must be `InPort<Vec<T>>`. Gathers per-iteration values back into
    /// a collection.
    pub fn connect_collect<V, T>(&mut self, from: OutPort<V, T>, to: InPort<Vec<T>>) -> &mut Self {
        self.inner.edge_collect(from.untyped(), to.untyped());
        self
    }

    /// Add a do-while edge: source and destination carry the same payload
    /// type `T` (the loop's state type).
    pub fn connect_do_while<V, T>(&mut self, from: OutPort<V, T>, to: InPort<T>) -> &mut Self {
        self.inner.edge_do_while(from.untyped(), to.untyped());
        self
    }

    /// Add a break edge — exits a do-while loop; source and destination
    /// carry the same payload type `T`.
    pub fn connect_break<V, T>(&mut self, from: OutPort<V, T>, to: InPort<T>) -> &mut Self {
        self.inner.edge_break(from.untyped(), to.untyped());
        self
    }

    /// Add a static edge — supplies an external value into a loop body;
    /// source and destination carry the same payload type `T`.
    pub fn connect_static<V, T>(&mut self, from: OutPort<V, T>, to: InPort<T>) -> &mut Self {
        self.inner.edge_static(from.untyped(), to.untyped());
        self
    }

    /// Declare an entry group.
    ///
    /// Vertex names are strings — typed handles are not required here
    /// because entry groups reference vertices by name in the wire format
    /// anyway, and accepting both typed and untyped handles is ergonomic.
    pub fn entry_group<S>(
        &mut self,
        name: impl Into<String>,
        vertices: impl IntoIterator<Item = S>,
    ) -> &mut Self
    where
        S: Into<String>,
    {
        self.inner.entry_group(name, vertices);
        self
    }

    /// Declare an entry port on a typed vertex.
    pub fn entry_port<D: ToolDescriptor>(
        &mut self,
        vertex: &TypedVertexRef<D>,
        port: impl Into<String>,
    ) -> &mut Self {
        let handle = VertexRef::named(vertex.name.clone());
        self.inner.entry_port(&handle, port);
        self
    }

    /// Declare an entry port on an untyped vertex.
    pub fn entry_port_untyped(
        &mut self,
        vertex: &UntypedVertexRef,
        port: impl Into<String>,
    ) -> &mut Self {
        let handle = VertexRef::named(vertex.name().to_owned());
        self.inner.entry_port(&handle, port);
        self
    }

    /// Provide an inline-JSON default for a typed input port.
    ///
    /// The value's Rust type `T` matches `InPort<T>`'s phantom parameter,
    /// so a mismatch (e.g. passing a `String` to an `InPort<i64>`) is a
    /// compile error.
    pub fn inline_default<T>(&mut self, port: InPort<T>, value: T) -> &mut Self
    where
        T: serde::Serialize,
    {
        let handle = VertexRef::named(port.vertex);
        let value = serde_json::to_value(value).expect("port payload serializes to JSON");
        self.inner.inline_default(&handle, port.port, value);
        self
    }

    /// Mark a typed output port as an output of the DAG.
    pub fn output<V, T>(&mut self, port: OutPort<V, T>) -> &mut Self {
        self.inner.output(port.untyped());
        self
    }

    /// Borrow the underlying relaxed [`DagBuilder`] for mixed typed/untyped
    /// authoring (e.g. connecting a typed port to an untyped one without
    /// going through `connect`).
    pub fn raw(&mut self) -> &mut DagBuilder {
        &mut self.inner
    }

    /// Finalize the builder — runs the same validation as the relaxed
    /// [`DagBuilder::build`] (topology, edge-kind, reference integrity).
    pub fn build(self) -> Result<Dag, Vec<DagError>> {
        self.inner.build()
    }
}
