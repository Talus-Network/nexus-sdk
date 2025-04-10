# Nexus DAG Construction Guide

This guide explains how to construct DAG (Directed Acyclic Graph) JSON files for the Nexus platform. DAGs define the workflow that an Agent will execute.

For an explanation of the terms and rules used below, refer to [the Nexus workflow documenetation][nexus-next-workflow].

## 1. Basic Structure
A DAG JSON file consists of sections defining the graph's components:
```json
{
  "vertices": [ ... ],       // All vertices in the DAG
  "edges": [ ... ],          // Connections defining data flow
  "default_values": [ ... ], // Static inputs for vertices
  "entry_groups": [ ... ]     // Named starting points (Optional)
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
  "input_ports": ["input_port_name", "another_input_port_name", ...]
}
```
- The `name` must be unique within the DAG.
- The input ports are determined by the schema of the specified tool identified by its `tool_fqn` on the one hand and the DAG design on the other hand. An input port defined in the tool definition must be populated either as a default value, through an edge or as user input as an entry port.
- A vertex acts as an *entry point* when its name is included in an `EntryGroup`. Note that when no `EntryGroup` is explicitly provided it defaults to `DefaultEntryGroup` where all vertices without any incoming edges are part of. Input ports associated with these vertices without default values attached to them are entry input ports that need an input provided to start execution.

## 3. Edges
Edges define the flow of data between vertices, connecting an output port of a source vertex to an input port of a target vertex:
```json
{
  "from": {
    "vertex": "source_vertex_name",       // Name from the "vertices" list
    "output_variant": "ok",             // e.g., ok, err, gt, lt, eq
    "output_port": "output_port_name"
  },
  "to": {
    "vertex": "target_vertex_name",       // Name from the "vertices" list
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
  "vertex": "vertex_name", // References a name from the "vertices" list
  "input_port": "port_name",
  "value": {
    "storage": "inline",
    "data": value
  }
}
```
**Important Constraints:**
- An `InputPort` can receive data either from an incoming `Edge` or a `Default Value`, but **never both**. ([workflow rules][nexus-next-workflow] Rule 4)
- Input ports designated as *entry input ports* within an activated `EntryGroup` (see Section 5) **cannot** have default values. Default values are only permitted for input ports that are *not* part of the selected entry mechanism for that execution. ([workflow rules][nexus-next-workflow] Rule 11)

## 5. Entry Groups (Optional)
Entry groups define named starting configurations for the DAG, specifying which vertices act as entry points and which of their input ports require external data (*entry input ports*) for a given execution.
```json
{
  "name": "group_name",
  "members": [
    {
      "vertex": "vertex_name_1", // Name must exist in the `vertices` list
      "input_port": "entry_port_for_v1"
    },
    {
      "vertex": "vertex_name_2", // Name must exist in the `vertices` list
      "input_port": "entry_port_for_v2"
    }
    // ... potentially more members
  ]
}
```
- An `EntryGroup` allows selecting a specific starting workflow via `nexus dag execute --entry-group group_name ...`.
- The `vertex` names specified within the `members` list **must** refer to vertices defined in the top-level `vertices` list.
- **Input Requirement:** When executing with a specific entry group, input data **must** be provided via `--input-json` for *every* designated *entry input port* specified in the `members` list of that group.
- **Default Value Restriction:** As stated in Section 4, *entry input ports* designated by the chosen `EntryGroup` cannot have default values.
- A default entry group can be named `_default_group`. This group is used when no `EntryGroup` flag is provided during execution.

{% hint style="info" %}
There is an edge case when a vertex has no entry input ports, no default values nor any incoming edges. This vertex can be executed immediately when starting the execution if it is included in the active entry group. In the `members` list of the `entry_groups` field, you simply add the vertex name without any input port key/value pair.
{% endhint %}

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
[nexus-next-workflow]: ../../nexus-next/packages/Workflow.md
[example-dags]: ../../cli/src/dag/_dags/
[nexus-cli]: ../CLI.md