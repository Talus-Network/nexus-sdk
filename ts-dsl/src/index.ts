// Public API for `@nexus/dag-dsl`. Keep in sync with the Rust mirror.

export type {
  Dag,
  Data,
  DefaultValue,
  Edge,
  EdgeKind,
  EntryGroup,
  EntryPort,
  FromPort,
  StorageKind,
  ToPort,
  ToolFqn,
  Vertex,
  VertexKind,
} from "./types.js";
export { DEFAULT_ENTRY_GROUP } from "./types.js";

export type { DagError } from "./error.js";
export { formatDagError } from "./error.js";

export { DagBuilder, InPortRef, OutPortRef, VertexRef, entryPort } from "./builder.js";

export {
  TypedDagBuilder,
  UntypedVertexRef,
  InPort,
  OutPort,
  defineTool,
} from "./typed.js";
export type { Ok, Err, ToolDescriptor, TypedVertexRef } from "./typed.js";

export {
  DoWhileScope,
  ForEachScope,
  ItemHandle,
  StateHandle,
  doWhile,
  doWhileNamed,
  forEach,
  forEachNamed,
} from "./scoped.js";

export { validateWire } from "./validator.js";
export type { WireValidationError } from "./validator.js";
