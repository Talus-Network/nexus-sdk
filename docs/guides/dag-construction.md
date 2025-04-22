# Nexus DAG Construction Guide

This guide explains how to construct DAG (Directed Acyclic Graph) JSON files for the Nexus platform. DAGs define the workflow that an Agent will execute.

For an explanation of the terms and rules used below, refer to [the Nexus workflow documenetation][nexus-next-workflow].

{% hint sytle="info"%}
Note that for all DAG related terms in the configuration JSON file, snake casing is applied.
{% endhint %}

## 1. Basic Structure

A DAG JSON file consists of sections defining the graph's components:

```json
{
  "vertices": [ ... ],       // All vertices in the DAG
  "edges": [ ... ],          // Connections defining data flow
  "default_values": [ ... ], // Static inputs for vertices
  "entry_groups": [ ... ]    // Named starting points (Optional)
}
```

## 2. Vertex Definitions (`vertices` list)

All vertices, whether they serve as entry points or internal steps, are defined within the main `vertices` list.

```json
{
  // Defined within the "vertices" list
  "kind": {
    "variant": "off_chain", // Or "on_chain"
    "tool_fqn": "namespace.tool.name@version"
  },
  "name": "unique_vertex_name",
  "entry_ports": ["entry_port_name", "another_entry_port_name", ...]
}
```

- The `name` must be unique within the DAG.
- The input ports of a tool are specified by the tool's output schema saved in the Nexus tool registry. Each input port must have exactly one of:
  1. An edge leading to it
  2. A default value
  3. Be part of `entry_ports`
- A vertex acts as an _entry point_ when its name is included in the `entry_groups` array. Note that when no _entry group_ is explicitly provided, all vertices with `entry_ports` are considered a part of the _default entry group_. When beginning an execution of an _entry group_, all `entry_ports` that belong to vertices that belong to the _entry group_ must be provided with input data.

## 3. Edges

Edges define the flow of data between vertices, connecting an output port of a source vertex to an input port of a target vertex:

```json
{
  "from": {
    "vertex": "source_vertex_name", // Name from the "vertices" list
    "output_variant": "ok", // e.g., ok, err, gt, lt, eq
    "output_port": "output_port_name"
  },
  "to": {
    "vertex": "target_vertex_name", // Name from the "vertices" list
    "input_port": "target_input_port_name"
  }
}
```

- The `source_vertex_name` and `target_vertex_name` refer to the `name` field of vertices defined in the `vertices` list.
- The `target_input_port_name` must be a valid input port for the tool used by the `target_vertex_name`.

## 4. Default Values

Default values provide static inputs to vertices:

```json
{
  "vertex": "vertex_name",  // References a name from the "vertices" list
  "input_port": "port_name",
  "value": {
    "storage": "inline",
    "data": value           // This is a JSON value
  }
}
```

**Important Constraints:**

- An _input port_ can receive data either from an _incoming edge_ or a _default value_, but **never both**. ([workflow rules][nexus-next-workflow] Rule 4)
- Entry ports **cannot** have default values. Default values are only permitted for input ports that are _not_ entry ports. ([workflow rules][nexus-next-workflow] Rule 11)

## 5. Entry Groups (Optional)

Entry groups define named starting configurations for the DAG, specifying which vertices act as entry points for a given execution.

<!-- TODO: <https://github.com/Talus-Network/nexus-sdk/pull/128> -->

```json
{
  "name": "group_name",
  "vertices": [
    "vertex_name_1", // Name must exist in the `vertices` list
    "vertex_name_2" // Name must exist in the `vertices` list
    // ... potentially more members
  ]
}
```

- An _entry group_ allows selecting a specific starting workflow via `nexus dag execute --entry-group group_name ...`.
- The `vertex` names specified **must** refer to vertices defined in the top-level `vertices` list.
- **Input Requirement:** When executing with a specific _entry group_, input data **must** be provided via `--input-json` for each _entry port_ belonging to vertices specified in the `entry_groups` list. If a vertex in `entry_groups` has no _entry ports_, it **must** still be specified in `--input-json` with an empty object `{}`.
- **Default Value Restriction:** As stated in Section 4, _entry ports_ that belong to the chosen _entry group_ cannot have default values.
- A default entry group can be named `_default_group`. This group is used when no `--entry-group` flag is provided during execution.
- If no _entry group_ is specified, all vertices that have _entry ports_ are considered part of the `_default_group`.

## 6. Validation Rules

The [Nexus CLI][nexus-cli] (`nexus dag validate`) performs static analysis to enforce the critical rules defined in [workflow rules][nexus-next-workflow].

## 7. Best Practices

1. **Naming Conventions**:

   - Use descriptive names for vertices.

2. **Organization**:

   - Keep the DAG as simple as possible. (But no simpler! For example, branching and entry groups can make powerful composite DAG structures. )
   - Use entry groups to provide different ways of starting DAG execution.

3. **Error Handling**:

   - Consider all possible `output_variant`s (e.g., `ok`, `err`) from tools.
   - Explicitly handle error paths or ensure they lead to acceptable end states.
   - Use appropriate comparison/logic tools for branching.

4. **Documentation**:
   - Provide documentation alongside the DAG, alongside a flowchart outlining it.
   - Document the purpose of each vertex.
   - Refer to the tool documentation for the expected input/output formats for each vertex.

## 8. Example Workflow

Here's a step-by-step process to create a DAG:

1. **Define Requirements**:

   - What inputs are needed?
   - What outputs are expected?
   - What processing steps are required?

2. **Design the Flow**:

   - Map out the vertices (tools) needed
   - Determine the connections
   - Identify branching points

3. **Create Entry Points**:

   - Specify entry ports and default values
   - Set up entry groups if needed

4. **Add Processing Vertices**:

   - Define intermediate vertices (tools)
   - Set up default values

5. **Connect the Dots**:

   - Create edges between vertices
   - Handle all output variants
   - Ensure proper data flow

6. **Validate**:
   - Check for cycles
   - Verify all connections
   - Test with sample inputs

## 9. Examples

For working examples, see the following files in the `cli/src/dag/_dags` directory:

- `math_branching.json`: Example of branching logic
- `multiple_entry_multiple_goal_valid.json`: Example of multiple entry points
- `multiple_output_ports_valid.json`: Example of multiple outputs
- `trip_planner.json`: Example of a real-world workflow
- `ig_story_planner_valid.json`: Example of a complex workflow
- `entry_groups_valid.json`: Example of using entry groups.

For examples of invalid DAGs and common mistakes to avoid (especially regarding Rule 5 - Race Conditions), see the diagrams in [workflow documentation][nexus-next-workflow] and the `*_invalid.json` files in the [testing DAG directory][example-dags].

<!-- List of references -->

[nexus-next-workflow]: ../../nexus-next/packages/workflow.md
[example-dags]: https://github.com/Talus-Network/nexus-sdk/tree/v0.1.0/cli/src/dag/_dags
[nexus-cli]: ../cli.md
