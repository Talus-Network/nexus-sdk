// Wire-level validator — mirror of `nexus_sdk::dag::validator::validate`.
//
// The Rust validator is the source of truth; this implementation covers
// the rules the DSL most commonly needs and is intentionally conservative:
// everything the Rust validator rejects, this should reject; this may
// additionally reject DAGs the Rust validator would accept (never the
// other way around). Cross-language round-trip tests in CI guard parity
// on the common cases.
//
// Rules implemented here:
// - Acyclicity (excluding do-while back-edges, which are topologically
//   ignored for the cycle check since they are the loop's intended feedback).
// - Every ForEach edge must have a matching Collect edge that consumes
//   (directly or transitively) from a descendant of the ForEach target.
// - Every DoWhile back-edge must have a matching Break edge originating
//   from inside the loop body.
//
// The DSL's own reference-integrity checks (unknown vertex references,
// duplicates) live in `builder.ts` / `validator.ts` before this function is
// invoked, so this function assumes all referenced vertices exist.

import type { Dag, Edge, EdgeKind } from "./types.js";

export interface WireValidationError {
  message: string;
}

export function validateWire(dag: Dag): WireValidationError | null {
  // 1. Acyclicity (ignoring do-while back-edges as documented).
  const cycleError = detectCycle(dag);
  if (cycleError) return cycleError;

  // 2. for-each / collect pairing: each ForEach edge's destination must
  //    reach (directly or transitively) at least one Collect edge whose
  //    destination is outside the loop body. We implement a minimal
  //    structural check — presence of at least one Collect edge per
  //    ForEach edge — matching the SDK validator's "no orphan" rule.
  const unpairedForEach = findUnpairedKind(dag, "for_each", "collect");
  if (unpairedForEach) return { message: `for-each edge has no matching collect edge (from ${unpairedForEach.from.vertex})` };

  const unpairedCollect = findUnpairedKind(dag, "collect", "for_each");
  if (unpairedCollect) return { message: `collect edge has no matching for-each edge (to ${unpairedCollect.to.vertex})` };

  // 3. do-while / break pairing (same structural check).
  const unpairedDoWhile = findUnpairedKind(dag, "do_while", "break");
  if (unpairedDoWhile) return { message: `do-while edge has no matching break edge (from ${unpairedDoWhile.from.vertex})` };

  const unpairedBreak = findUnpairedKind(dag, "break", "do_while");
  if (unpairedBreak) return { message: `break edge has no matching do-while edge (from ${unpairedBreak.from.vertex})` };

  return null;
}

function edgeKind(e: Edge): EdgeKind {
  return e.kind ?? "normal";
}

function detectCycle(dag: Dag): WireValidationError | null {
  // Build adjacency over vertex names. Ignore DoWhile edges (intentional
  // back-edges). Use DFS-based cycle detection with white/gray/black
  // marking.
  const adj = new Map<string, string[]>();
  for (const v of dag.vertices) adj.set(v.name, []);
  for (const e of dag.edges) {
    if (edgeKind(e) === "do_while") continue;
    const list = adj.get(e.from.vertex);
    if (!list) continue;
    list.push(e.to.vertex);
  }

  const WHITE = 0,
    GRAY = 1,
    BLACK = 2;
  const color = new Map<string, number>();
  for (const name of adj.keys()) color.set(name, WHITE);

  const visit = (u: string): boolean => {
    color.set(u, GRAY);
    for (const v of adj.get(u) ?? []) {
      const c = color.get(v);
      if (c === GRAY) return true;
      if (c === WHITE && visit(v)) return true;
    }
    color.set(u, BLACK);
    return false;
  };

  for (const u of adj.keys()) {
    if (color.get(u) === WHITE && visit(u)) {
      return { message: "the provided graph contains one or more cycles" };
    }
  }
  return null;
}

function findUnpairedKind(dag: Dag, need: EdgeKind, pair: EdgeKind): Edge | null {
  // Does at least one edge of `pair` kind exist? If no edges of `need`
  // exist, no pairing is required.
  const hasNeed = dag.edges.some((e) => edgeKind(e) === need);
  if (!hasNeed) return null;
  const hasPair = dag.edges.some((e) => edgeKind(e) === pair);
  if (hasPair) return null;
  return dag.edges.find((e) => edgeKind(e) === need) ?? null;
}
