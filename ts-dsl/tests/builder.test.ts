// Relaxed DagBuilder tests — parallel the Rust
// `dag-dsl/tests/relaxed_builder.rs`. Each test's comment states what
// breaks if it's deleted.

import { describe, expect, it } from "vitest";
import { DagBuilder, VertexRef, type DagError } from "../src/index.js";

const ADD_FQN = "xyz.taluslabs.math.i64.add@1";

describe("relaxed DagBuilder", () => {
  // Happy path: a two-vertex DAG with one normal edge and one output
  // builds cleanly. Guards the whole happy-path authoring surface.
  it("builds minimal two-vertex dag", () => {
    const dag = new DagBuilder();
    const a = dag.offchain("a", ADD_FQN);
    const b = dag.offchain("b", ADD_FQN);
    dag
      .entryPort(a, "a")
      .inlineDefault(a, "b", 1)
      .edge(a.out("ok", "result"), b.inp("a"))
      .inlineDefault(b, "b", 2)
      .output(b.out("ok", "result"));

    const result = dag.build();
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.dag.vertices.length).toBe(2);
    expect(result.dag.edges.length).toBe(1);
    expect(result.dag.edges[0].kind).toBeUndefined(); // Normal edges omit kind.
    expect(result.dag.outputs?.length).toBe(1);
  });

  // Edge references to an unknown vertex surface
  // EdgeFromUnknownVertex / EdgeToUnknownVertex with a locator.
  it("detects edge references to unknown vertices", () => {
    const dag = new DagBuilder();
    const b = dag.offchain("b", ADD_FQN);
    dag.edge(VertexRef.named("ghost").out("ok", "result"), b.inp("a"));

    const result = dag.build();
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(
      result.errors.some((e: DagError) => e.kind === "EdgeFromUnknownVertex" && e.vertex === "ghost"),
    ).toBe(true);
  });

  // Entry groups listing unknown vertices surface
  // EntryGroupUnknownVertex with group + vertex.
  it("detects entry-group references to unknown vertices", () => {
    const dag = new DagBuilder();
    dag.offchain("a", ADD_FQN);
    dag.entryGroup("main", ["a", "never_added"]);

    const result = dag.build();
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(
      result.errors.some(
        (e: DagError) =>
          e.kind === "EntryGroupUnknownVertex" && e.group === "main" && e.vertex === "never_added",
      ),
    ).toBe(true);
  });

  // Multiple local errors aggregate in one pass — mirror of the Rust
  // behaviour (R16, P7).
  it("aggregates multiple local errors", () => {
    const dag = new DagBuilder();
    dag.offchain("a", ADD_FQN);
    dag.entryGroup("main", ["ghost"]);
    dag.output(VertexRef.named("nope").out("ok", "result"));
    dag.inlineDefault(VertexRef.named("missing"), "b", 1);

    const result = dag.build();
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.errors.length).toBeGreaterThanOrEqual(3);
  });

  // Cycles surface WireValidation — the local validator defers
  // structural checks to the wire-level validator.
  it("detects cycles", () => {
    const dag = new DagBuilder();
    const a = dag.offchain("a", ADD_FQN);
    const b = dag.offchain("b", ADD_FQN);
    dag.entryPort(a, "a");
    dag.inlineDefault(a, "b", 1);
    dag.inlineDefault(b, "b", 1);
    dag.edge(a.out("ok", "result"), b.inp("a"));
    dag.edge(b.out("ok", "result"), a.inp("a"));
    dag.output(a.out("ok", "result"));

    const result = dag.build();
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.errors[0].kind).toBe("WireValidation");
  });
});
