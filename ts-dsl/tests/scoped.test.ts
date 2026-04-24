// Scoped layer tests — parallel the Rust `dag-dsl/tests/scoped_builder.rs`.

import { describe, expect, it } from "vitest";
import { DagBuilder, forEach, forEachNamed } from "../src/index.js";

const ADD_FQN = "xyz.taluslabs.math.i64.add@1";
const MAKE_VEC_FQN = "xyz.taluslabs.util.make_vec@1";
const SUM_FQN = "xyz.taluslabs.math.i64.sum@1";

describe("scoped DagBuilder", () => {
  // `consume` emits ForEach; `collect` emits Collect. Both are the
  // whole point of the scoped surface.
  it("emits ForEach + Collect edges", () => {
    const dag = new DagBuilder();
    const splitter = dag.offchain("splitter", MAKE_VEC_FQN);
    const aggregator = dag.offchain("aggregator", SUM_FQN);
    dag.entryPort(splitter, "n");

    forEachNamed(dag, "per_item", splitter.out("ok", "items"), (scope, item) => {
      const add = scope.offchain("add", ADD_FQN);
      scope.inlineDefault(add, "b", 1);
      scope.consume(item, add.inp("a"));
      scope.collect(add.out("ok", "result"), aggregator.inp("vec"));
    });
    dag.output(aggregator.out("ok", "result"));

    const result = dag.build();
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    const kinds = result.dag.edges.map((e) => e.kind ?? "normal");
    expect(kinds).toEqual(["for_each", "collect"]);

    // Scope-local vertex name was prefixed with `.`.
    const names = result.dag.vertices.map((v) => v.name);
    expect(names).toContain("per_item.add");
  });

  // Anonymous scopes get distinct auto-names (`foreach_0`, `foreach_1`).
  // Checked before `.build()` since two for-each loops into a shared
  // sink would be rejected by the wire validator (race condition).
  it("auto-names sibling anonymous scopes distinctly", () => {
    const dag = new DagBuilder();
    const a_src = dag.offchain("src_a", MAKE_VEC_FQN);
    const b_src = dag.offchain("src_b", MAKE_VEC_FQN);
    dag.offchain("sink_a", SUM_FQN);
    dag.offchain("sink_b", SUM_FQN);
    dag.entryPort(a_src, "n");
    dag.entryPort(b_src, "n");

    forEach(dag, a_src.out("ok", "items"), (scope, item) => {
      const t = scope.offchain("t", ADD_FQN);
      scope.inlineDefault(t, "b", 1);
      scope.consume(item, t.inp("a"));
    });
    forEach(dag, b_src.out("ok", "items"), (scope, item) => {
      const t = scope.offchain("t", ADD_FQN);
      scope.inlineDefault(t, "b", 2);
      scope.consume(item, t.inp("a"));
    });

    const names = dag.getVertices().map((v) => v.name);
    expect(names).toContain("foreach_0.t");
    expect(names).toContain("foreach_1.t");
  });
});
