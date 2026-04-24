//! Scoped authoring surface.
//!
//! The scoped API introduces lexical loop bodies via closures. Instead of
//! enumerating `EdgeKind::ForEach` / `Collect` / `DoWhile` / `Break` /
//! `Static` edges manually (as the flat layer does), the user writes the
//! loop structure and the scope emits the correct edge kinds automatically:
//!
//! ```no_run
//! use nexus_dag_dsl::{DagBuilder, ToolFqn};
//! use std::str::FromStr;
//!
//! let fqn = |s: &str| ToolFqn::from_str(s).unwrap();
//! let mut dag = DagBuilder::new();
//! let splitter = dag.offchain("splitter", fqn("xyz.taluslabs.util.make_vec@1"));
//! let aggregator = dag.offchain("aggregator", fqn("xyz.taluslabs.math.i64.sum@1"));
//! dag.entry_port(&splitter, "n");
//!
//! dag.for_each_named("per_item", splitter.out("ok", "items"), |scope, item| {
//!     let add = scope.offchain("add", fqn("xyz.taluslabs.math.i64.add@1"));
//!     scope.inline_default(&add, "b", 1);
//!     scope.consume(item, add.inp("a"));
//!     scope.collect(add.out("ok", "result"), aggregator.inp("vec"));
//! });
//! dag.output(aggregator.out("ok", "result"));
//! ```
//!
//! Scope names join with `.` to form the wire-format vertex name. Anonymous
//! scopes are auto-named `foreach_N` / `dowhile_N` where `N` is a
//! monotonic counter per builder. Users can override via the `*_named`
//! variants.

use {
    crate::builder::{DagBuilder, InPortRef, OutPortRef, VertexRef},
    nexus_sdk::ToolFqn,
};

// ---------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------

/// Per-iteration item handle, produced by [`DagBuilder::for_each`].
///
/// `ItemHandle` wraps the scope's source output port. Consume it via
/// [`ForEachScope::consume`] to emit a `ForEach` edge from the source to
/// a destination input port. `Clone` — an item can be consumed by multiple
/// destinations.
#[derive(Clone, Debug)]
pub struct ItemHandle {
    source: OutPortRef,
}

/// Per-iteration state handle, produced by [`DagBuilder::do_while`].
///
/// Wraps the seed source port. Feed it into the loop body's state input via
/// [`DoWhileScope::feed_state`]. `Clone` for multi-consumer feeds.
#[derive(Clone, Debug)]
pub struct StateHandle {
    source: OutPortRef,
}

// ---------------------------------------------------------------------------
// ForEachScope
// ---------------------------------------------------------------------------

/// A for-each scope — passed to the closure given to [`DagBuilder::for_each`].
///
/// Vertices added here are auto-prefixed with the scope's name (`.`-joined)
/// to avoid name collisions. Edges from the per-iteration [`ItemHandle`]
/// are `ForEach`-tagged; outbound edges from [`ForEachScope::collect`] are
/// `Collect`-tagged.
pub struct ForEachScope<'a> {
    builder: &'a mut DagBuilder,
    prefix: String,
}

impl ForEachScope<'_> {
    /// Add an off-chain tool vertex, with its name prefixed by the scope's
    /// path.
    pub fn offchain(&mut self, name: impl Into<String>, fqn: ToolFqn) -> VertexRef {
        let name = prefix_join(&self.prefix, name.into());
        self.builder.offchain(name, fqn)
    }

    /// Add an on-chain tool vertex, with its name prefixed by the scope's
    /// path.
    pub fn onchain(&mut self, name: impl Into<String>, fqn: ToolFqn) -> VertexRef {
        let name = prefix_join(&self.prefix, name.into());
        self.builder.onchain(name, fqn)
    }

    /// Consume the per-iteration item — emits a `ForEach` edge from the
    /// scope's source to `to`.
    pub fn consume(&mut self, item: ItemHandle, to: InPortRef) -> &mut Self {
        self.builder.edge_for_each(item.source, to);
        self
    }

    /// Add a normal (data-flow) edge inside the loop body.
    pub fn edge(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.builder.edge(from, to);
        self
    }

    /// Emit a `Collect` edge from a loop-body output to a destination input
    /// port outside the loop — gathers one value per iteration into a
    /// collection at `to`.
    pub fn collect(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.builder.edge_collect(from, to);
        self
    }

    /// Declare an entry port on a scope-local vertex.
    pub fn entry_port(&mut self, vertex: &VertexRef, port: impl Into<String>) -> &mut Self {
        self.builder.entry_port(vertex, port);
        self
    }

    /// Provide an inline-JSON default value for a scope-local vertex.
    pub fn inline_default(
        &mut self,
        vertex: &VertexRef,
        input_port: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> &mut Self {
        self.builder.inline_default(vertex, input_port, value);
        self
    }

    /// Enter a nested for-each scope. Names compose — the inner scope's
    /// prefix is `<outer>.<inner>`.
    pub fn for_each<F>(&mut self, source: OutPortRef, body: F) -> &mut Self
    where
        F: FnOnce(&mut ForEachScope<'_>, ItemHandle),
    {
        let inner_name = format!("foreach_{}", self.builder.next_scope_index());
        self.for_each_named_impl(inner_name, source, body)
    }

    /// Enter a nested for-each scope with an explicit name.
    pub fn for_each_named<F>(
        &mut self,
        name: impl Into<String>,
        source: OutPortRef,
        body: F,
    ) -> &mut Self
    where
        F: FnOnce(&mut ForEachScope<'_>, ItemHandle),
    {
        self.for_each_named_impl(name.into(), source, body)
    }

    fn for_each_named_impl<F>(&mut self, name: String, source: OutPortRef, body: F) -> &mut Self
    where
        F: FnOnce(&mut ForEachScope<'_>, ItemHandle),
    {
        let prefix = prefix_join(&self.prefix, name);
        let item = ItemHandle {
            source: source.clone(),
        };
        let mut inner = ForEachScope {
            builder: self.builder,
            prefix,
        };
        body(&mut inner, item);
        self
    }
}

// ---------------------------------------------------------------------------
// DoWhileScope
// ---------------------------------------------------------------------------

/// A do-while scope — passed to the closure given to [`DagBuilder::do_while`].
///
/// Vertices added here are auto-prefixed with the scope's name. Back-edges
/// from [`DoWhileScope::continue_with`] are `DoWhile`-tagged;
/// loop-exit edges from [`DoWhileScope::break_to`] are `Break`-tagged;
/// external inputs from [`DoWhileScope::static_input`] are `Static`-tagged.
pub struct DoWhileScope<'a> {
    builder: &'a mut DagBuilder,
    prefix: String,
}

impl DoWhileScope<'_> {
    /// Add an off-chain tool vertex, prefixed by the scope's path.
    pub fn offchain(&mut self, name: impl Into<String>, fqn: ToolFqn) -> VertexRef {
        let name = prefix_join(&self.prefix, name.into());
        self.builder.offchain(name, fqn)
    }

    /// Add an on-chain tool vertex, prefixed by the scope's path.
    pub fn onchain(&mut self, name: impl Into<String>, fqn: ToolFqn) -> VertexRef {
        let name = prefix_join(&self.prefix, name.into());
        self.builder.onchain(name, fqn)
    }

    /// Seed the loop body's state input from the scope's state handle —
    /// emits a normal edge from the seed source to `to`.
    pub fn feed_state(&mut self, state: StateHandle, to: InPortRef) -> &mut Self {
        self.builder.edge(state.source, to);
        self
    }

    /// Emit a `DoWhile` back-edge — the loop's "continue" condition.
    pub fn continue_with(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.builder.edge_do_while(from, to);
        self
    }

    /// Emit a `Break` edge — exits the loop body.
    pub fn break_to(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.builder.edge_break(from, to);
        self
    }

    /// Emit a `Static` edge — provides a fixed value from outside the loop
    /// into the loop body.
    pub fn static_input(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.builder.edge_static(from, to);
        self
    }

    /// Add a normal (data-flow) edge inside the loop body.
    pub fn edge(&mut self, from: OutPortRef, to: InPortRef) -> &mut Self {
        self.builder.edge(from, to);
        self
    }

    /// Declare an entry port on a scope-local vertex.
    pub fn entry_port(&mut self, vertex: &VertexRef, port: impl Into<String>) -> &mut Self {
        self.builder.entry_port(vertex, port);
        self
    }

    /// Provide an inline-JSON default for a scope-local vertex.
    pub fn inline_default(
        &mut self,
        vertex: &VertexRef,
        input_port: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> &mut Self {
        self.builder.inline_default(vertex, input_port, value);
        self
    }
}

// ---------------------------------------------------------------------------
// Builder extension
// ---------------------------------------------------------------------------

impl DagBuilder {
    /// Enter a for-each scope. The source port's per-iteration item is
    /// passed to the closure as an [`ItemHandle`]; consume it via
    /// [`ForEachScope::consume`] to emit `ForEach` edges.
    pub fn for_each<F>(&mut self, source: OutPortRef, body: F) -> &mut Self
    where
        F: FnOnce(&mut ForEachScope<'_>, ItemHandle),
    {
        let name = format!("foreach_{}", self.next_scope_index());
        self.for_each_named_impl(name, source, body)
    }

    /// Enter a for-each scope with an explicit name. The name is used as
    /// the prefix of every vertex created in the scope
    /// (`<name>.<vertex_name>`).
    pub fn for_each_named<F>(
        &mut self,
        name: impl Into<String>,
        source: OutPortRef,
        body: F,
    ) -> &mut Self
    where
        F: FnOnce(&mut ForEachScope<'_>, ItemHandle),
    {
        self.for_each_named_impl(name.into(), source, body)
    }

    fn for_each_named_impl<F>(&mut self, name: String, source: OutPortRef, body: F) -> &mut Self
    where
        F: FnOnce(&mut ForEachScope<'_>, ItemHandle),
    {
        let item = ItemHandle {
            source: source.clone(),
        };
        let mut scope = ForEachScope {
            builder: self,
            prefix: name,
        };
        body(&mut scope, item);
        self
    }

    /// Enter a do-while scope. The seed source is passed to the closure as
    /// a [`StateHandle`]; wire it into the loop body via
    /// [`DoWhileScope::feed_state`].
    pub fn do_while<F>(&mut self, seed: OutPortRef, body: F) -> &mut Self
    where
        F: FnOnce(&mut DoWhileScope<'_>, StateHandle),
    {
        let name = format!("dowhile_{}", self.next_scope_index());
        self.do_while_named_impl(name, seed, body)
    }

    /// Enter a do-while scope with an explicit name.
    pub fn do_while_named<F>(
        &mut self,
        name: impl Into<String>,
        seed: OutPortRef,
        body: F,
    ) -> &mut Self
    where
        F: FnOnce(&mut DoWhileScope<'_>, StateHandle),
    {
        self.do_while_named_impl(name.into(), seed, body)
    }

    fn do_while_named_impl<F>(&mut self, name: String, seed: OutPortRef, body: F) -> &mut Self
    where
        F: FnOnce(&mut DoWhileScope<'_>, StateHandle),
    {
        let state = StateHandle {
            source: seed.clone(),
        };
        let mut scope = DoWhileScope {
            builder: self,
            prefix: name,
        };
        body(&mut scope, state);
        self
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn prefix_join(prefix: &str, name: String) -> String {
    if prefix.is_empty() {
        name
    } else {
        format!("{}.{}", prefix, name)
    }
}
