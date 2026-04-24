// Strict, flat TypedDagBuilder — TypeScript mirror of
// `nexus_dag_dsl::TypedDagBuilder`. Compile-time port-payload type safety
// is provided via phantom generics and TypeScript's structural type
// system.

import { DagBuilder, InPortRef, OutPortRef, VertexRef } from "./builder.js";
import type { Dag, ToolFqn } from "./types.js";
import type { DagError } from "./error.js";

/** Marker for the default `ok` output variant. */
export interface Ok {
  readonly __variant: "ok";
}
/** Marker for the default `err` output variant. */
export interface Err {
  readonly __variant: "err";
}

/**
 * Typed input port — carries the vertex name and port name at runtime and
 * the payload type `T` at compile time (phantom).
 */
export class InPort<T> {
  declare readonly __payload: T;
  constructor(
    public readonly vertex: string,
    public readonly port: string,
  ) {}
  untyped(): InPortRef {
    return new InPortRef(this.vertex, this.port);
  }
}

/**
 * Typed output port — carries vertex / variant / port at runtime and
 * variant tag `V` + payload type `T` at compile time (phantom).
 */
export class OutPort<V, T> {
  declare readonly __variant: V;
  declare readonly __payload: T;
  constructor(
    public readonly vertex: string,
    public readonly variant: string,
    public readonly port: string,
  ) {}
  untyped(): OutPortRef {
    return new OutPortRef(this.vertex, this.variant, this.port);
  }
}

/**
 * A tool's contract for the strict DSL layer. Mirrors
 * `nexus_dag_dsl::ToolDescriptor`.
 *
 * Implementors expose the tool's FQN plus two factory methods that
 * produce typed view objects bound to a given vertex name.
 */
export interface ToolDescriptor<I, O> {
  fqn: ToolFqn;
  inputsFor(vertexName: string): I;
  outputsFor(vertexName: string): O;
}

/** Handle returned when a typed vertex is added. */
export interface TypedVertexRef<I, O> {
  name: string;
  inp: I;
  out: O;
}

/** Escape-hatch handle for vertices without a descriptor. */
export class UntypedVertexRef {
  constructor(public readonly name: string) {}
  out(variant: string, port: string): OutPortRef {
    return new OutPortRef(this.name, variant, port);
  }
  inp(port: string): InPortRef {
    return new InPortRef(this.name, port);
  }
}

/**
 * Strict, flat builder — wraps `DagBuilder`. Typed operations lower to
 * the same edge primitives.
 */
export class TypedDagBuilder {
  private readonly inner = new DagBuilder();

  add<I, O>(name: string, desc: ToolDescriptor<I, O>): TypedVertexRef<I, O> {
    this.inner.offchain(name, desc.fqn);
    return { name, inp: desc.inputsFor(name), out: desc.outputsFor(name) };
  }

  addOnchain<I, O>(name: string, desc: ToolDescriptor<I, O>): TypedVertexRef<I, O> {
    this.inner.onchain(name, desc.fqn);
    return { name, inp: desc.inputsFor(name), out: desc.outputsFor(name) };
  }

  addUntyped(name: string, fqn: ToolFqn): UntypedVertexRef {
    this.inner.offchain(name, fqn);
    return new UntypedVertexRef(name);
  }

  addOnchainUntyped(name: string, fqn: ToolFqn): UntypedVertexRef {
    this.inner.onchain(name, fqn);
    return new UntypedVertexRef(name);
  }

  // --- connect family: kind-specific arity encoded in the signatures ---

  connect<V, T>(from: OutPort<V, T>, to: InPort<T>): this {
    this.inner.edge(from.untyped(), to.untyped());
    return this;
  }

  connectForEach<V, T>(from: OutPort<V, T[]>, to: InPort<T>): this {
    this.inner.edgeForEach(from.untyped(), to.untyped());
    return this;
  }

  connectCollect<V, T>(from: OutPort<V, T>, to: InPort<T[]>): this {
    this.inner.edgeCollect(from.untyped(), to.untyped());
    return this;
  }

  connectDoWhile<V, T>(from: OutPort<V, T>, to: InPort<T>): this {
    this.inner.edgeDoWhile(from.untyped(), to.untyped());
    return this;
  }

  connectBreak<V, T>(from: OutPort<V, T>, to: InPort<T>): this {
    this.inner.edgeBreak(from.untyped(), to.untyped());
    return this;
  }

  connectStatic<V, T>(from: OutPort<V, T>, to: InPort<T>): this {
    this.inner.edgeStatic(from.untyped(), to.untyped());
    return this;
  }

  // --- misc passthroughs ---

  entryGroup(name: string, vertexNames: string[]): this {
    this.inner.entryGroup(name, vertexNames);
    return this;
  }

  entryPort<I, O>(vertex: TypedVertexRef<I, O>, port: string): this {
    this.inner.entryPort(VertexRef.named(vertex.name), port);
    return this;
  }

  entryPortUntyped(vertex: UntypedVertexRef, port: string): this {
    this.inner.entryPort(VertexRef.named(vertex.name), port);
    return this;
  }

  inlineDefault<T>(port: InPort<T>, value: T): this {
    this.inner.inlineDefault(VertexRef.named(port.vertex), port.port, value);
    return this;
  }

  output<V, T>(port: OutPort<V, T>): this {
    this.inner.output(port.untyped());
    return this;
  }

  /** Escape to the underlying relaxed builder for mixed typed/untyped edges. */
  raw(): DagBuilder {
    return this.inner;
  }

  build(): { ok: true; dag: Dag } | { ok: false; errors: DagError[] } {
    return this.inner.build();
  }
}

/**
 * Helper used by codegen: build a `ToolDescriptor` from a declarative
 * input/output spec. Intended for the codegen-emitted descriptors. Hand-
 * written descriptors can implement `ToolDescriptor` directly.
 *
 * The `inputs` and `outputs` arguments are the port-name maps; payload
 * types are carried as phantom generics.
 */
export function defineTool<I extends Record<string, unknown>, O extends Record<string, Record<string, unknown>>>(
  spec: {
    fqn: ToolFqn;
    inputs: I;
    outputs: O;
  },
): ToolDescriptor<
  { [K in keyof I]: InPort<I[K]> },
  { [V in keyof O]: { [P in keyof O[V]]: OutPort<V, O[V][P]> } }
> {
  return {
    fqn: spec.fqn,
    inputsFor(vertexName) {
      const out: Record<string, InPort<unknown>> = {};
      for (const key of Object.keys(spec.inputs)) {
        out[key] = new InPort(vertexName, key);
      }
      return out as { [K in keyof I]: InPort<I[K]> };
    },
    outputsFor(vertexName) {
      const out: Record<string, Record<string, OutPort<unknown, unknown>>> = {};
      for (const variant of Object.keys(spec.outputs)) {
        const ports: Record<string, OutPort<unknown, unknown>> = {};
        for (const portName of Object.keys(spec.outputs[variant] as object)) {
          ports[portName] = new OutPort(vertexName, variant, portName);
        }
        out[variant] = ports;
      }
      return out as { [V in keyof O]: { [P in keyof O[V]]: OutPort<V, O[V][P]> } };
    },
  };
}
