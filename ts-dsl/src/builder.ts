// Relaxed, flat DagBuilder — TypeScript mirror of `nexus_dag_dsl::DagBuilder`.
//
// Same API shape, same validation rules, same wire format. Compile-time
// port-payload type safety lives in `typed.ts`.

import type { Dag, Data, DefaultValue, Edge, EdgeKind, EntryGroup, EntryPort, FromPort, ToolFqn, Vertex } from "./types.js";
import type { DagError } from "./error.js";
import { validateWire } from "./validator.js";

export class VertexRef {
  constructor(public readonly name: string) {}

  out(variant: string, port: string): OutPortRef {
    return new OutPortRef(this.name, variant, port);
  }

  inp(port: string): InPortRef {
    return new InPortRef(this.name, port);
  }

  /** Construct a handle for a vertex by name (no guarantee it's been added). */
  static named(name: string): VertexRef {
    return new VertexRef(name);
  }
}

export class OutPortRef {
  constructor(
    public readonly vertex: string,
    public readonly variant: string,
    public readonly port: string,
  ) {}

  toFromPort(): FromPort {
    return { vertex: this.vertex, output_variant: this.variant, output_port: this.port };
  }
}

export class InPortRef {
  constructor(public readonly vertex: string, public readonly port: string) {}

  toToPort(): { vertex: string; input_port: string } {
    return { vertex: this.vertex, input_port: this.port };
  }
}

export class DagBuilder {
  private vertices: Vertex[] = [];
  private edges: Edge[] = [];
  private defaultValues: DefaultValue[] = [];
  private entryGroups: EntryGroup[] = [];
  private outputs: FromPort[] = [];
  private vertexNames: Set<string> = new Set();
  private scopeCounter = 0;

  offchain(name: string, fqn: ToolFqn): VertexRef {
    this.vertexNames.add(name);
    this.vertices.push({ name, kind: { variant: "off_chain", tool_fqn: fqn } });
    return new VertexRef(name);
  }

  onchain(name: string, fqn: ToolFqn): VertexRef {
    this.vertexNames.add(name);
    this.vertices.push({ name, kind: { variant: "on_chain", tool_fqn: fqn } });
    return new VertexRef(name);
  }

  entryPort(vertex: VertexRef, port: string): this {
    const v = this.vertices.find((v) => v.name === vertex.name);
    if (v) {
      (v.entry_ports ??= []).push({ name: port });
    }
    return this;
  }

  edge(from: OutPortRef, to: InPortRef): this {
    return this.pushEdge(from, to, "normal");
  }

  edgeForEach(from: OutPortRef, to: InPortRef): this {
    return this.pushEdge(from, to, "for_each");
  }

  edgeCollect(from: OutPortRef, to: InPortRef): this {
    return this.pushEdge(from, to, "collect");
  }

  edgeDoWhile(from: OutPortRef, to: InPortRef): this {
    return this.pushEdge(from, to, "do_while");
  }

  edgeBreak(from: OutPortRef, to: InPortRef): this {
    return this.pushEdge(from, to, "break");
  }

  edgeStatic(from: OutPortRef, to: InPortRef): this {
    return this.pushEdge(from, to, "static");
  }

  private pushEdge(from: OutPortRef, to: InPortRef, kind: EdgeKind): this {
    const edge: Edge = { from: from.toFromPort(), to: to.toToPort() };
    if (kind !== "normal") edge.kind = kind;
    this.edges.push(edge);
    return this;
  }

  entryGroup(name: string, vertices: string[]): this {
    this.entryGroups.push({ name, vertices: [...vertices] });
    return this;
  }

  defaultValue(vertex: VertexRef, inputPort: string, value: Data): this {
    this.defaultValues.push({ vertex: vertex.name, input_port: inputPort, value });
    return this;
  }

  inlineDefault(vertex: VertexRef, inputPort: string, value: unknown): this {
    return this.defaultValue(vertex, inputPort, { storage: "inline", data: value });
  }

  walrusDefault(vertex: VertexRef, inputPort: string, value: unknown): this {
    return this.defaultValue(vertex, inputPort, { storage: "walrus", data: value });
  }

  output(port: OutPortRef): this {
    this.outputs.push(port.toFromPort());
    return this;
  }

  /** Accessor for the accumulated vertices (useful for tests / inspection). */
  getVertices(): readonly Vertex[] {
    return this.vertices;
  }

  getEdges(): readonly Edge[] {
    return this.edges;
  }

  /**
   * Allocate the next anonymous scope index. Used by the scoped layer
   * (`scoped.ts`).
   */
  nextScopeIndex(): number {
    return this.scopeCounter++;
  }

  /**
   * Finalize the builder — run local consistency checks, then wire-level
   * validation. All locally-detectable errors are aggregated; wire-level
   * validation is a single pass that bails on first failure (mirroring
   * the Rust validator).
   */
  build(): { ok: true; dag: Dag } | { ok: false; errors: DagError[] } {
    const errors: DagError[] = [];

    const seen = new Set<string>();
    for (const v of this.vertices) {
      if (!seen.add(v.name)) {
        errors.push({ kind: "DuplicateVertex", name: v.name });
      }
    }

    for (const e of this.edges) {
      if (!this.vertexNames.has(e.from.vertex)) {
        errors.push({
          kind: "EdgeFromUnknownVertex",
          vertex: e.from.vertex,
          variant: e.from.output_variant,
          port: e.from.output_port,
        });
      }
      if (!this.vertexNames.has(e.to.vertex)) {
        errors.push({ kind: "EdgeToUnknownVertex", vertex: e.to.vertex, port: e.to.input_port });
      }
    }

    for (const g of this.entryGroups) {
      for (const v of g.vertices) {
        if (!this.vertexNames.has(v)) {
          errors.push({ kind: "EntryGroupUnknownVertex", group: g.name, vertex: v });
        }
      }
    }

    for (const d of this.defaultValues) {
      if (!this.vertexNames.has(d.vertex)) {
        errors.push({ kind: "DefaultValueUnknownVertex", vertex: d.vertex, port: d.input_port });
      }
    }

    for (const o of this.outputs) {
      if (!this.vertexNames.has(o.vertex)) {
        errors.push({
          kind: "OutputUnknownVertex",
          vertex: o.vertex,
          variant: o.output_variant,
          port: o.output_port,
        });
      }
    }

    if (errors.length > 0) return { ok: false, errors };

    const dag: Dag = {
      vertices: this.vertices,
      edges: this.edges,
      ...(this.defaultValues.length > 0 ? { default_values: this.defaultValues } : {}),
      ...(this.entryGroups.length > 0 ? { entry_groups: this.entryGroups } : {}),
      ...(this.outputs.length > 0 ? { outputs: this.outputs } : {}),
    };

    const wireError = validateWire(dag);
    if (wireError) {
      return { ok: false, errors: [{ kind: "WireValidation", message: wireError.message }] };
    }

    return { ok: true, dag };
  }
}

// Helper so users don't need to construct EntryPort objects directly.
export const entryPort = (name: string): EntryPort => ({ name });
