// Typed DagBuilder tests — parallel the Rust
// `dag-dsl/tests/typed_builder.rs`. Exercise defineTool (codegen's
// descriptor factory) + TypedDagBuilder end-to-end.

import { describe, expect, it } from "vitest";
import { TypedDagBuilder, defineTool, type OutPort, type InPort, type Ok } from "../src/index.js";

// Descriptor for a simple add tool.
const AddTool = defineTool({
  fqn: "xyz.taluslabs.math.i64.add@1",
  inputs: {
    a: 0 as unknown as number,
    b: 0 as unknown as number,
  },
  outputs: {
    ok: {
      result: 0 as unknown as number,
    },
  },
});

describe("typed DagBuilder", () => {
  // Happy path: add two typed vertices, connect via typed ports, build.
  it("builds minimal typed dag", () => {
    const dag = new TypedDagBuilder();
    const a = dag.add("a", AddTool);
    const b = dag.add("b", AddTool);
    dag.entryPort(a, "a");
    dag.inlineDefault(a.inp.b, 1);
    dag.inlineDefault(b.inp.b, 2);
    dag.connect(a.out.ok.result, b.inp.a);
    dag.output(b.out.ok.result);

    const result = dag.build();
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.dag.vertices.length).toBe(2);
    expect(result.dag.edges.length).toBe(1);
  });

  // Compile-time: `connect` expects matching payload types. This test
  // only asserts runtime correctness — compile-time rejection is
  // verified at `tsc --noEmit` time as an invariant of the public API.
  it("types check at compile time (sanity)", () => {
    // The following lines must compile — they exercise the typed
    // connect signatures in the expected-good case.
    const _checkTypes = (
      out: OutPort<Ok, number>,
      inp: InPort<number>,
    ): void => {
      const dag = new TypedDagBuilder();
      dag.connect(out, inp);
      dag.connectDoWhile(out, inp);
      dag.connectBreak(out, inp);
      dag.connectStatic(out, inp);
    };
    expect(typeof _checkTypes).toBe("function");
  });
});
