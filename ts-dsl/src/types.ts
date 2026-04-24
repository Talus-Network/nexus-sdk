// Wire-format types mirroring `nexus_sdk::types::Dag` and children.
//
// These JSON-shaped types are the canonical contract between the Rust SDK,
// this DSL, and any other consumer. The Rust wire types are the source of
// truth (see `sdk/src/types/json_dag.rs`) — changes there must be mirrored
// here, guarded by CI round-trip tests.

export const DEFAULT_ENTRY_GROUP = "_default_group" as const;

/** A Nexus Tool Fully-Qualified Name, e.g. `xyz.taluslabs.math.i64.add@1`. */
export type ToolFqn = string;

export type EdgeKind = "normal" | "for_each" | "collect" | "do_while" | "break" | "static";

export type StorageKind = "inline" | "walrus";

export type VertexKind =
  | { variant: "off_chain"; tool_fqn: ToolFqn }
  | { variant: "on_chain"; tool_fqn: ToolFqn };

export interface EntryPort {
  name: string;
}

export interface Vertex {
  kind: VertexKind;
  name: string;
  entry_ports?: EntryPort[];
}

export interface EntryGroup {
  name: string;
  vertices: string[];
}

export interface Data {
  storage: StorageKind;
  data: unknown;
}

export interface DefaultValue {
  vertex: string;
  input_port: string;
  value: Data;
}

export interface FromPort {
  vertex: string;
  output_variant: string;
  output_port: string;
}

export interface ToPort {
  vertex: string;
  input_port: string;
}

export interface Edge {
  from: FromPort;
  to: ToPort;
  /** Omitted on the wire when kind === "normal". */
  kind?: EdgeKind;
}

export interface Dag {
  vertices: Vertex[];
  edges: Edge[];
  default_values?: DefaultValue[];
  entry_groups?: EntryGroup[];
  outputs?: FromPort[];
}
