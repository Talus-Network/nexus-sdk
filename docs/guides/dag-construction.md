# Nexus DAG Construction Guide

This guide explains how to construct DAG (Directed Acyclic Graph) JSON files for the Nexus platform. DAGs define the workflow that an Agent will execute.

For an explanation of the terms and rules used below, refer to [the Nexus workflow documentation][nexus-next-workflow].

{% hint style="info"%}
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

**All** vertices are defined within the main `vertices` list.

```json
{
  // Defined within the "vertices" list
  "kind": {
    "variant": "off_chain", // Or "on_chain"
    "tool_fqn": "namespace.tool.name@version"
  },
  "name": "unique_vertex_name",
  "entry_ports": [
    {
      "name": "input_port_name", // Must match the tool's input schema
      "encrypted": false // Optional, default is false
    }
    // ... potentially more entry ports
  ]
}
```

- The `name` must be unique within the DAG.

{% hint style="success" %}
The input ports of a tool are specified by the tool's output schema saved in the Nexus tool registry. Each input port must have exactly one of:

1. An edge leading to it
1. A default value
1. Be part of `entry_ports`

{% endhint %}

- Add the input port as part of `entry_ports` only if it does not have an edge leading into it nor has a default value.

{% hint style="warning" %}
When beginning an execution of an [_entry group_](#5-entry-groups-optional), all `entry_ports` that belong to vertices that belong to the _entry group_ must be provided with client input data.
{% endhint %}

## 3. Edges

Edges define the flow of data between vertices, connecting an output port of a source vertex to an input port of a target vertex:

```json
{
  "from": {
    "vertex": "source_vertex_name", // Name from the "vertices" list
    "output_variant": "ok", // e.g., ok, err, gt, lt, eq
    "output_port": "output_port_name",
    "encrypted": true // Optional, default is false
  },
  "to": {
    "vertex": "target_vertex_name", // Name from the "vertices" list
    "input_port": "target_input_port_name"
  },
  "kind": "normal" // Optional, default is "normal"
}
```

- The `source_vertex_name` and `target_vertex_name` refer to the `name` field of vertices defined in the `vertices` list.
- The `target_input_port_name` must be a valid input port for the tool used by the `target_vertex_name`.
- If the `encrypted` field is set to `true`, the data will be encrypted before being sent on-chain. This is useful for sensitive data that should not be exposed in plaintext.

## 3.1 Special edge types

- **`normal`** → The default edge type. Data flows from the source vertex to the target vertex as soon as the source vertex produces output on the specified port.
- **`for_each`** → Spawns parallel executions of the downstream vertex, once per element of an array. This must be followed by a `collect` (with any number of intermediate `normal` edges) to gather the results back into a single array.
- **`collect`** → Gathers the results of a `for_each` branch back into a single ordered array.
- **`do_while`** → Repeats execution by looping back as long as a condition is satisfied.
- **`break`** → Exits a `do_while` loop when the condition is no longer met.

Read more about looping and flow controls in the [looping documentation][looping].

## 4. Default Values

Default values provide static inputs to vertices:

```json
{
  "vertex": "vertex_name", // References a name from the "vertices" list
  "input_port": "port_name",
  "value": {
    "storage": "inline",
    "data": value, // This is a JSON value
  }
}
```

**Important Constraints:**

- An _input port_ can receive data either from an _incoming edge_ or a _default value_, but **never both**. ([workflow rules][nexus-next-workflow] Rule 4)
- Entry ports **cannot** have default values (by definition). Default values are only permitted for input ports that are _not_ entry ports. ([workflow rules][nexus-next-workflow] Rule 11)

## 5. Entry Groups (Optional)

Entry groups define named starting configurations for the DAG, specifying which vertices act as entry points for a given execution (possibly multiple concurrent walks).

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

A vertex acts as an _entry point_ when its name is included in the `entry_groups` array.

{% hint style="warning" %}
Being part of an entry group does not imply that all vertices in the group get executed immediately upon DAG execution invocation. In Nexus, any vertex will only be executed as soon as all input ports on the vertex have a value.
{% endhint %}

Practically speaking, this means that you'll need to add all vertices to the entry group that either (non-exclusive):

- will immediately start execution
- need to be provided client input for entry ports

## 6. Outputs (Optional)

Outputs can be defined on vertices that have no outgoing edges. These can be thought of as the return values of a DAG execution. Only ports that are specified here will be part of the `EndStateReachedEvent` emitted by the DAG execution.

```json
{
  "vertex": "vertex_name", // Name must exist in the `vertices` list
  "output_variant": "ok", // e.g., ok, err, gt, lt, eq
  "output_port": "output_port_name", // e.g., result
  "encrypted": true // Optional, default is false
}
```

**Important Constraints:**

- DAG construction will fail if you attempt to add an output on a vertex that has outgoing edges.
- Only data from ports that are specified here or in the `edges` field will flow onchain.

### Summary

- An _entry group_ allows selecting a specific starting configuration of the workflow with selected entry points via `nexus dag execute --entry-group group_name ...`.
- The `vertex` names specified **must** refer to vertices defined in the top-level `vertices` list.
- **Input Requirement:** When executing with a specific _entry group_, input data **must** be provided via `--input-json` for each _entry port_ belonging to vertices specified in the `entry_groups` list. If a vertex in `entry_groups` has no _entry ports_, it **must** still be specified in `--input-json` with an empty object `{}`.
- **Default Value Restriction:** As stated in Section 4, _entry ports_ that belong to the chosen _entry group_ cannot have default values.
- A default entry group can be named `_default_group`. This group is used when no `--entry-group` flag is provided during execution.
- If no _entry group_ is specified, all vertices that have _entry ports_ are considered part of the `_default_group`.

## 7. Validation Rules

The [Nexus CLI][nexus-cli] (`nexus dag validate`) performs static analysis to enforce the critical rules defined in [workflow rules][nexus-next-workflow].

## 8. Best Practices

1. **Naming Conventions**:

   - Use descriptive names for vertices.

1. **Organization**:

   - Keep the DAG as simple as possible. (But no simpler! For example, branching and entry groups can make powerful composite DAG structures. )
   - Use entry groups to provide different ways of starting DAG execution.

1. **Error Handling**:

   - Consider all possible `output_variant`s (e.g., `ok`, `err`) from tools.
   - Explicitly handle error paths or ensure they lead to acceptable end states.
   - Use appropriate comparison/logic tools for branching.

1. **Documentation**:
   - Provide documentation alongside the DAG, alongside a flowchart outlining it.
   - Document the purpose of each vertex.
   - Refer to the tool documentation for the expected input/output formats for each vertex.

## 9. Example Workflow

Here's a step-by-step process to create a DAG:

1. **Define Requirements**:

   - What inputs are needed?
   - What outputs are expected?
   - What processing steps are required?

1. **Design the Flow**:

   - Map out the vertices (tools) needed
   - Determine the connections
   - Identify branching points

1. **Create Entry Points**:

   - Specify entry ports and default values
   - Set up entry groups if needed

1. **Add Processing Vertices**:

   - Define intermediate vertices (tools)
   - Set up default values

1. **Connect the Dots**:

   - Create edges between vertices
   - Handle all output variants
   - Ensure proper data flow

1. **Validate**:
   - Check for cycles
   - Verify all connections
   - Test with sample inputs

## 10. Examples

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
[example-dags]: https://github.com/Talus-Network/nexus-sdk/tree/v0.4.0/sdk/src/dag/_dags
[nexus-cli]: ../cli.md
[looping]: ../../nexus-next/flow-controls/looping.md
