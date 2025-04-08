# Nexus DAG Construction Guide

This guide explains how to construct DAG (Directed Acyclic Graph) JSON files for the Nexus platform. DAGs define the workflow of operations that an Agent will execute.

## 1. Basic Structure
A DAG JSON file consists of four main sections:
```json
{
  "entry_vertices": [...],
  "vertices": [...],
  "edges": [...],
  "default_values": [...],
  "entry_groups": [...]  // Optional
}
```

## 2. Vertex Types

### 2.1 Entry Vertices
Entry vertices are the starting points of your DAG that accept initial input:
```json
{
  "kind": {
    "variant": "off_chain",
    "tool_fqn": "namespace.tool.name@version"
  },
  "name": "vertex_name",
  "input_ports": ["port1", "port2"]
}
```

### 2.2 Regular Vertices
Regular vertices are intermediate nodes in your DAG:
```json
{
  "kind": {
    "variant": "off_chain",
    "tool_fqn": "namespace.tool.name@version"
  },
  "name": "vertex_name"
}
```

## 3. Edges
Edges define the flow of data between vertices:
```json
{
  "from": {
    "vertex": "source_vertex_name",
    "output_variant": "ok",  // or "lt", "gt", "eq" for comparison results
    "output_port": "port_name"
  },
  "to": {
    "vertex": "target_vertex_name",
    "input_port": "port_name"
  }
}
```

## 4. Default Values
Default values provide static inputs to vertices:
```json
{
  "vertex": "vertex_name",
  "input_port": "port_name",
  "value": {
    "storage": "inline",
    "data": value  // Can be any JSON value
  }
}
```

## 5. Entry Groups (Optional)
Entry groups organize entry vertices into logical groups:
```json
{
  "name": "group_name",
  "vertices": ["vertex1", "vertex2"]
}
```

## 6. Common Patterns

### 6.1 Branching Logic
For conditional branching:
1. Use a comparison tool (`cmp`) as the branching point
2. Connect different output variants (`lt`, `gt`, `eq`) to different operations
3. Each branch should have its own default values

### 6.2 Multiple Entry Points
For multiple entry points:
1. Define multiple entry vertices
2. Each entry vertex can have its own input ports
3. Use entry groups to organize them if needed

### 6.3 Multiple Outputs
For multiple outputs:
1. Define vertices with multiple output ports
2. Connect each output port to different downstream vertices
3. Use appropriate output variants for different result types

## 7. Validation Rules
Here are important rules to follow:

1. **No Cycles**: The graph must be acyclic (no circular dependencies)
2. **No Dead Ends**: All paths should lead to a valid output
3. **No Undefined Connections**: All vertex references in edges must exist
4. **No Duplicate Entry Inputs**: Entry vertices can't have the same input port multiple times
5. **No Default Values on Entry Inputs**: Entry vertices can't have default values
6. **Proper Entry Groups**: Entry groups must contain valid vertex names
7. **No Normal Vertices in Groups**: Only entry vertices can be in entry groups

## 8. Best Practices

1. **Naming Conventions**:
   - Use descriptive names for vertices
   - Use consistent naming for input/output ports
   - Prefix tool names with namespace (e.g., `xyz.taluslabs.math`)

2. **Organization**:
   - Group related vertices together
   - Use entry groups for complex workflows
   - Keep the DAG as simple as possible

3. **Error Handling**:
   - Consider all possible output variants
   - Handle error cases explicitly
   - Use appropriate comparison tools for branching

4. **Documentation**:
   - Add comments for complex logic
   - Document the purpose of each vertex
   - Explain the expected input/output formats

## 9. Example Workflow

Here's a step-by-step process to create a DAG:

1. **Define Requirements**:
   - What inputs are needed?
   - What outputs are expected?
   - What processing steps are required?

2. **Design the Flow**:
   - Map out the vertices needed
   - Determine the connections
   - Identify branching points

3. **Create Entry Points**:
   - Define entry vertices
   - Specify input ports
   - Set up entry groups if needed

4. **Add Processing Vertices**:
   - Define intermediate vertices
   - Specify their tools
   - Set up default values

5. **Connect the Dots**:
   - Create edges between vertices
   - Handle all output variants
   - Ensure proper data flow

6. **Validate**:
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

For examples of invalid DAGs and common mistakes to avoid, see the `*_invalid.json` files in the same directory. 