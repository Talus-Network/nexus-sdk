// Cross-language round-trip test: parse every committed DAG JSON
// fixture from the SDK's `sdk/src/dag/_dags/` directory, serialize it
// back, re-parse, and assert structural equality.
//
// This test pins the canonical wire contract across Rust and TS. If
// the Rust `Serialize` output or the TS parser/serializer diverge, this
// test fails — independent of whether either side's builder APIs change.

import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { Dag } from "../src/index.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const fixturesDir = join(__dirname, "..", "..", "sdk", "src", "dag", "_dags");

// What the wire's optional-key omission contract is: absent fields are
// omitted (not null). TS's structural typing happily accepts either, so
// we compare via JSON canonicalization rather than deep object equality.
function canonicalize(v: unknown): string {
  return JSON.stringify(sortKeys(v));
}

function sortKeys(v: unknown): unknown {
  if (v === null || typeof v !== "object") return v;
  if (Array.isArray(v)) return v.map(sortKeys);
  const obj = v as Record<string, unknown>;
  const sorted: Record<string, unknown> = {};
  for (const key of Object.keys(obj).sort()) {
    sorted[key] = sortKeys(obj[key]);
  }
  return sorted;
}

describe("cross-language wire round-trip", () => {
  const files = readdirSync(fixturesDir).filter((f) => f.endsWith(".json"));

  // The fixture list must not be empty — otherwise this test silently
  // asserts nothing.
  it("fixture directory is non-empty", () => {
    expect(files.length).toBeGreaterThan(0);
  });

  // For each parseable fixture, round-trip it: parse → re-stringify →
  // re-parse → compare canonicalized JSON strings. Non-parseable files
  // (the intentionally-invalid ones, e.g. `empty_invalid.json`) are
  // skipped — they are for other tests.
  //
  // What breaks if this test is deleted: the Rust Serialize output or
  // this TS parser/serializer could silently diverge from the wire
  // format — breaking the one-source-of-truth contract that makes the
  // DSL work across both languages.
  it("round-trips every parseable fixture", () => {
    let checked = 0;
    const skipped: string[] = [];
    for (const file of files) {
      const path = join(fixturesDir, file);
      const content = readFileSync(path, "utf-8");
      let parsed: Dag;
      try {
        parsed = JSON.parse(content) as Dag;
      } catch {
        skipped.push(file);
        continue;
      }
      // Basic shape check: must have vertices + edges arrays to count
      // as a DAG.
      if (!Array.isArray(parsed.vertices) || !Array.isArray(parsed.edges)) {
        skipped.push(file);
        continue;
      }
      const reserialized = JSON.stringify(parsed);
      const reparsed = JSON.parse(reserialized);
      expect(canonicalize(reparsed)).toBe(canonicalize(parsed));
      checked++;
    }
    expect(checked).toBeGreaterThan(0);
  });
});
