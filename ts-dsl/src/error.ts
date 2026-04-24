// Errors surfaced by `DagBuilder.build()`. Mirrors `nexus-dag-dsl`'s
// `DagError` — every variant carries a locator (vertex name, port name,
// edge endpoints) so the user can fix the offending construct without
// re-scanning the whole builder input.

export type DagError =
  | { kind: "DuplicateVertex"; name: string }
  | { kind: "EdgeFromUnknownVertex"; vertex: string; variant: string; port: string }
  | { kind: "EdgeToUnknownVertex"; vertex: string; port: string }
  | { kind: "EntryGroupUnknownVertex"; group: string; vertex: string }
  | { kind: "DefaultValueUnknownVertex"; vertex: string; port: string }
  | { kind: "OutputUnknownVertex"; vertex: string; variant: string; port: string }
  | { kind: "WireValidation"; message: string };

export function formatDagError(e: DagError): string {
  switch (e.kind) {
    case "DuplicateVertex":
      return `duplicate vertex name \`${e.name}\``;
    case "EdgeFromUnknownVertex":
      return `edge source references unknown vertex \`${e.vertex}\` (variant \`${e.variant}\`, port \`${e.port}\`)`;
    case "EdgeToUnknownVertex":
      return `edge destination references unknown vertex \`${e.vertex}\` (port \`${e.port}\`)`;
    case "EntryGroupUnknownVertex":
      return `entry group \`${e.group}\` references unknown vertex \`${e.vertex}\``;
    case "DefaultValueUnknownVertex":
      return `default value references unknown vertex \`${e.vertex}\` (input port \`${e.port}\`)`;
    case "OutputUnknownVertex":
      return `output references unknown vertex \`${e.vertex}\` (variant \`${e.variant}\`, port \`${e.port}\`)`;
    case "WireValidation":
      return `wire-level validation failed: ${e.message}`;
  }
}
