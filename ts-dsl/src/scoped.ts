// Scoped authoring surface — mirror of `nexus_dag_dsl::scoped`.
//
// `forEach` and `doWhile` closures introduce lexical loop bodies that
// auto-tag the correct `EdgeKind` and auto-prefix scope-local vertex
// names with `.` (per the plan's D12).

import { DagBuilder, InPortRef, OutPortRef, VertexRef } from "./builder.js";
import type { ToolFqn } from "./types.js";

export class ItemHandle {
  constructor(public readonly source: OutPortRef) {}
}

export class StateHandle {
  constructor(public readonly source: OutPortRef) {}
}

export class ForEachScope {
  constructor(
    private readonly builder: DagBuilder,
    private readonly prefix: string,
  ) {}

  offchain(name: string, fqn: ToolFqn): VertexRef {
    return this.builder.offchain(joinPrefix(this.prefix, name), fqn);
  }

  onchain(name: string, fqn: ToolFqn): VertexRef {
    return this.builder.onchain(joinPrefix(this.prefix, name), fqn);
  }

  consume(item: ItemHandle, to: InPortRef): this {
    this.builder.edgeForEach(item.source, to);
    return this;
  }

  edge(from: OutPortRef, to: InPortRef): this {
    this.builder.edge(from, to);
    return this;
  }

  collect(from: OutPortRef, to: InPortRef): this {
    this.builder.edgeCollect(from, to);
    return this;
  }

  entryPort(vertex: VertexRef, port: string): this {
    this.builder.entryPort(vertex, port);
    return this;
  }

  inlineDefault(vertex: VertexRef, inputPort: string, value: unknown): this {
    this.builder.inlineDefault(vertex, inputPort, value);
    return this;
  }

  forEach(source: OutPortRef, body: (scope: ForEachScope, item: ItemHandle) => void): this {
    const inner = `foreach_${this.builder.nextScopeIndex()}`;
    return this.forEachNamedImpl(inner, source, body);
  }

  forEachNamed(
    name: string,
    source: OutPortRef,
    body: (scope: ForEachScope, item: ItemHandle) => void,
  ): this {
    return this.forEachNamedImpl(name, source, body);
  }

  private forEachNamedImpl(
    name: string,
    source: OutPortRef,
    body: (scope: ForEachScope, item: ItemHandle) => void,
  ): this {
    const item = new ItemHandle(source);
    const inner = new ForEachScope(this.builder, joinPrefix(this.prefix, name));
    body(inner, item);
    return this;
  }
}

export class DoWhileScope {
  constructor(
    private readonly builder: DagBuilder,
    private readonly prefix: string,
  ) {}

  offchain(name: string, fqn: ToolFqn): VertexRef {
    return this.builder.offchain(joinPrefix(this.prefix, name), fqn);
  }

  onchain(name: string, fqn: ToolFqn): VertexRef {
    return this.builder.onchain(joinPrefix(this.prefix, name), fqn);
  }

  feedState(state: StateHandle, to: InPortRef): this {
    this.builder.edge(state.source, to);
    return this;
  }

  continueWith(from: OutPortRef, to: InPortRef): this {
    this.builder.edgeDoWhile(from, to);
    return this;
  }

  breakTo(from: OutPortRef, to: InPortRef): this {
    this.builder.edgeBreak(from, to);
    return this;
  }

  staticInput(from: OutPortRef, to: InPortRef): this {
    this.builder.edgeStatic(from, to);
    return this;
  }

  edge(from: OutPortRef, to: InPortRef): this {
    this.builder.edge(from, to);
    return this;
  }

  entryPort(vertex: VertexRef, port: string): this {
    this.builder.entryPort(vertex, port);
    return this;
  }

  inlineDefault(vertex: VertexRef, inputPort: string, value: unknown): this {
    this.builder.inlineDefault(vertex, inputPort, value);
    return this;
  }
}

// Extension methods on DagBuilder provided as free functions (TypeScript
// class-extension syntax is awkward; keeping these as functions keeps
// DagBuilder's class definition minimal).

export function forEach(
  builder: DagBuilder,
  source: OutPortRef,
  body: (scope: ForEachScope, item: ItemHandle) => void,
): void {
  const name = `foreach_${builder.nextScopeIndex()}`;
  forEachNamedImpl(builder, name, source, body);
}

export function forEachNamed(
  builder: DagBuilder,
  name: string,
  source: OutPortRef,
  body: (scope: ForEachScope, item: ItemHandle) => void,
): void {
  forEachNamedImpl(builder, name, source, body);
}

function forEachNamedImpl(
  builder: DagBuilder,
  name: string,
  source: OutPortRef,
  body: (scope: ForEachScope, item: ItemHandle) => void,
): void {
  const scope = new ForEachScope(builder, name);
  body(scope, new ItemHandle(source));
}

export function doWhile(
  builder: DagBuilder,
  seed: OutPortRef,
  body: (scope: DoWhileScope, state: StateHandle) => void,
): void {
  const name = `dowhile_${builder.nextScopeIndex()}`;
  doWhileNamedImpl(builder, name, seed, body);
}

export function doWhileNamed(
  builder: DagBuilder,
  name: string,
  seed: OutPortRef,
  body: (scope: DoWhileScope, state: StateHandle) => void,
): void {
  doWhileNamedImpl(builder, name, seed, body);
}

function doWhileNamedImpl(
  builder: DagBuilder,
  name: string,
  seed: OutPortRef,
  body: (scope: DoWhileScope, state: StateHandle) => void,
): void {
  const scope = new DoWhileScope(builder, name);
  body(scope, new StateHandle(seed));
}

function joinPrefix(prefix: string, name: string): string {
  return prefix.length === 0 ? name : `${prefix}.${name}`;
}
