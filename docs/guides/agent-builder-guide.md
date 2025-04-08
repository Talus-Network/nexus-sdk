# Agent Builder Guide: Creating Simple Math + LLM DAGs

This guide walks through the process of creating two simple Directed Acyclic Graphs (DAGs) for the Nexus platform. These DAGs demonstrate a basic linear workflow involving a mathematical operation followed by a call to an OpenAI LLM. We will leverage the principles outlined in the main [Nexus DAG Construction Guide](../dag-construction.md).

The goal is to build two DAGs:
1.  **DAG 1:** Takes a number as input, adds 420 to it, and sends the result to an LLM to guess the original number and make a joke.
2.  **DAG 2:** Takes a number as input, multiplies it by 69, and sends the result to an LLM with a similar guessing and joking task.

## Core DAG Components

Recall that Nexus DAGs are defined in JSON and primarily consist of:
*   `entry_vertices`: Define the starting points of the DAG and their initial inputs.
*   `vertices`: Define the processing steps (Tools) within the DAG.
*   `edges`: Define the data flow connections between vertices.
*   `default_values`: Provide static or pre-configured inputs to vertices.
*   `entry_groups` (Optional): Group entry vertices, though not used in these simple examples.

## DAG 1: Addition + LLM (`math_add_llm_simple.json`)

This DAG performs addition and then calls the LLM.

### 1. Entry Vertex (`add_step`)

We start with an entry vertex that uses the standard math addition tool.
```json
{
  "entry_vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.math.i64.add@1" // Standard i64 addition tool
      },
      "name": "add_step",          // Descriptive name for the step
      "input_ports": ["a"]         // Expects one input, named 'a'
    }
  ],
  // ... rest of DAG ...
}
```
This vertex, named `add_step`, is the entry point. It uses the `xyz.taluslabs.math.i64.add@1` tool and expects a single input value provided to its `a` port when the DAG is executed.

### 2. Regular Vertex (`guess_step`)

Next, we define the LLM step as a regular vertex.
```json
{
  // ... entry_vertices ...
  "vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.llm.openai.chat-completion@1" // Standard OpenAI tool
      },
      "name": "guess_step"         // Descriptive name
    }
  ],
  // ... rest of DAG ...
}
```
This vertex, `guess_step`, uses the `xyz.taluslabs.llm.openai.chat-completion@1` tool. It doesn't define input ports here because its inputs will come either from other vertices (via edges) or from default values.

### 3. Edge (Connecting `add_step` to `guess_step`)

We need to connect the output of the addition step to the input of the LLM step.
```json
{
  // ... vertices ...
  "edges": [
    {
      "from": {
        "vertex": "add_step",           // Source vertex
        "output_variant": "ok",       // Use the 'ok' outcome of the addition
        "output_port": "result"       // Use the 'result' port of the 'ok' variant
      },
      "to": {
        "vertex": "guess_step",          // Target vertex
        "input_port": "prompt"          // Connect to the 'prompt' input of the LLM
      }
    }
  ],
  // ... rest of DAG ...
}
```
This edge specifies that if `add_step` completes successfully (`ok` variant), its `result` output should be fed into the `prompt` input port of `guess_step`.

### 4. Default Values

We provide the constant value `420` for the addition and configure the LLM step.
```json
{
  // ... edges ...
  "default_values": [
    {
      "vertex": "add_step",
      "input_port": "b",              // Provide the second number for addition
      "value": { "storage": "inline", "data": 420 } // The constant value
    },
    {
      "vertex": "guess_step",
      "input_port": "api_key",        // Provide the OpenAI API key
      "value": { "storage": "inline", "data": "YOUR_API_KEY_HERE" } // Placeholder!
    },
    {
      "vertex": "guess_step",
      "input_port": "context",        // Provide the system prompt/instructions
      "value": {
        "storage": "inline",
        "data": [                     // MessageBag format
          {
            "role": "system",
            "content": "You'll get a number provided to you which is the result of adding an unknown number to 420. By inference, guess what number the input was. Make a joke about using a technological construct like the Nexus framework to deliver a rather simple result."
          }
        ]
      }
    }
  ],
  "entry_groups": [] // No entry groups needed for this simple case
}
```
- The first default value sets the `b` input of `add_step` to `420`.
- The second provides the `api_key` for `guess_step`. **Crucially, `"YOUR_API_KEY_HERE"` is a placeholder and must be replaced with a valid OpenAI API key for the DAG to function.** Using `Secret<String>` is recommended for production.
- The third provides the system message (`context`) to the LLM, instructing it on its task. The input from the `add_step` (connected via the edge) will be treated as the user message by the LLM tool.

## DAG 2: Multiplication + LLM (`math_mul_llm_simple.json`)

This DAG is structurally identical to DAG 1, with two key differences:
1.  It uses the multiplication tool (`xyz.taluslabs.math.i64.mul@1`) instead of addition.
2.  The default value for the math operation is `69` instead of `420`.
3.  The LLM context prompt is slightly adjusted to reflect the multiplication operation.

```json
// cli/src/dag/_dags/math_mul_llm_simple.json
{
  "entry_vertices": [
    {
      "kind": { "variant": "off_chain", "tool_fqn": "xyz.taluslabs.math.i64.mul@1" }, // Multiplication tool
      "name": "mul_step",
      "input_ports": ["a"]
    }
  ],
  "vertices": [
    {
      "kind": { "variant": "off_chain", "tool_fqn": "xyz.taluslabs.llm.openai.chat-completion@1" },
      "name": "guess_step"
    }
  ],
  "edges": [
    {
      "from": { "vertex": "mul_step", "output_variant": "ok", "output_port": "result" },
      "to": { "vertex": "guess_step", "input_port": "prompt" }
    }
  ],
  "default_values": [
    {
      "vertex": "mul_step",
      "input_port": "b",
      "value": { "storage": "inline", "data": 69 } // Default value is 69
    },
    {
      "vertex": "guess_step",
      "input_port": "api_key",
      "value": { "storage": "inline", "data": "YOUR_API_KEY_HERE" } // Placeholder!
    },
    {
      "vertex": "guess_step",
      "input_port": "context",
      "value": {
        "storage": "inline",
        "data": [
          {
            "role": "system",
            "content": "You'll get a number provided to you which is the result of multiplying an unknown number by 69. By inference, guess what number the input was. Make a joke about using a technological construct like the Nexus framework to deliver a rather simple result." // Adjusted prompt
          }
        ]
      }
    }
  ],
  "entry_groups": []
}

```

## Alternative: Combined DAG with Entry Groups (`math_add_or_mul_llm_grouped.json`)

While creating separate DAGs for each workflow is perfectly valid, maintaining multiple similar DAGs can lead to redundancy. Nexus offers a more elegant solution for such cases using **Entry Groups**. Instead of two separate files, we can combine the addition and multiplication workflows into a single DAG.

This approach involves:
*   Defining *both* the `add_step` and `mul_step` as `entry_vertices`.
*   Keeping the single `guess_step` as a regular `vertex`.
*   Having *two* edges, one from `add_step` and one from `mul_step`, both targeting the same `guess_step.prompt` input.
*   Crucially, defining specific `entry_groups` that allow the user to choose which path to execute.

### 1. Multiple Entry Vertices

Both math operations are now defined as potential starting points:
```json
// From math_add_or_mul_llm_grouped.json
{
  "entry_vertices": [
    {
      "kind": { "variant": "off_chain", "tool_fqn": "xyz.taluslabs.math.i64.add@1" },
      "name": "add_step",
      "input_ports": ["a"]
    },
    {
      "kind": { "variant": "off_chain", "tool_fqn": "xyz.taluslabs.math.i64.mul@1" },
      "name": "mul_step",
      "input_ports": ["a"]
    }
  ],
  // ...
}
```

### 2. Converging Edges

The output of *either* math step is routed to the *same* LLM prompt input:
```json
// From math_add_or_mul_llm_grouped.json
{
  // ...
  "edges": [
    {
      "from": { "vertex": "add_step", "output_variant": "ok", "output_port": "result" },
      "to": { "vertex": "guess_step", "input_port": "prompt" }
    },
    {
      "from": { "vertex": "mul_step", "output_variant": "ok", "output_port": "result" },
      "to": { "vertex": "guess_step", "input_port": "prompt" }
    }
  ],
  // ...
}
```

### 3. Generic LLM Context

Since `guess_step` can receive input from either `add_step` or `mul_step`, its `context` (system prompt) must be generic enough to handle both possibilities:
```json
// From math_add_or_mul_llm_grouped.json
{
  // ...
  "default_values": [
    // ... math defaults ...
    { "vertex": "guess_step", "input_port": "api_key", "value": { ... } },
    {
      "vertex": "guess_step",
      "input_port": "context",
      "value": {
        "storage": "inline",
        "data": [
          {
            "role": "system",
            "content": "You'll get a number provided to you. This number is the result of either (1) adding an unknown number to 420, OR (2) multiplying an unknown number by 69. Based on the number you receive, infer which operation was performed and guess the original unknown number. Make a joke about using a technological construct like the Nexus framework to deliver a rather simple result."
          }
        ]
      }
    }
  ],
 // ...
}
```
This prompt explicitly tells the LLM about the two possible origins of the input number.

### 4. Entry Groups for Selection

Finally, `entry_groups` allow selecting the desired workflow at runtime:
```json
// From math_add_or_mul_llm_grouped.json
{
  // ...
  "entry_groups": [
    {
      "name": "add_path",        // Name for the addition workflow
      "vertices": ["add_step"]   // Specifies the starting vertex for this path
    },
    {
      "name": "mul_path",        // Name for the multiplication workflow
      "vertices": ["mul_step"]   // Specifies the starting vertex for this path
    }
  ]
}
```
When executing this DAG using `nexus dag execute`, you would specify `--entry-group add_path` or `--entry-group mul_path` to trigger the corresponding workflow.

### Advantages of the Combined Approach

*   **Reduced Redundancy:** Avoids duplicating the `guess_step` definition and its associated default values (like the API key).
*   **Single Artifact:** Easier to manage, version, and deploy one DAG file instead of two.
*   **Flexibility:** Leverages the entry group mechanism for selecting execution paths within a single DAG structure.

This pattern is generally preferred when workflows share significant downstream logic but differ only in their initial steps or branches.

## Managing and Running the DAGs with the CLI

Once you have created your DAG JSON files, you'll use the `nexus` CLI to validate, publish, and execute them. Here's a breakdown based on the commands documented in `docs/CLI.md`.

**(Note:** Commands like `publish` and `execute` typically require a configured wallet connected to the CLI.)

### 1. Validation

Before publishing, always validate your DAG structure:

```bash
# Validate the simple addition DAG
nexus dag validate --path cli/src/dag/_dags/math_add_llm_simple.json

# Validate the simple multiplication DAG
nexus dag validate --path cli/src/dag/_dags/math_mul_llm_simple.json

# Validate the combined DAG
nexus dag validate --path cli/src/dag/_dags/math_add_or_mul_llm_grouped.json
```
This step catches structural errors, ensuring your DAG adheres to Nexus rules.

### 2. Publishing

Once validated, publish the DAG to make it executable. Note down the DAG ID returned by each command.

```bash
# Publish the simple addition DAG
nexus dag publish --path cli/src/dag/_dags/math_add_llm_simple.json
# Example output might include: Published DAG with ID: <add_dag_id>

# Publish the simple multiplication DAG
nexus dag publish --path cli/src/dag/_dags/math_mul_llm_simple.json
# Example output might include: Published DAG with ID: <mul_dag_id>

# Publish the combined DAG
nexus dag publish --path cli/src/dag/_dags/math_add_or_mul_llm_grouped.json
# Example output might include: Published DAG with ID: <combined_dag_id>
```

### 3. Execution

Execute the published DAG using its ID. You need to provide input data via `--input-json`. For the combined DAG, you **must** specify the `--entry-group`.

**Input JSON Structure:**
The JSON string maps entry vertex names to objects, which map input port names to their values.

```json
// Example Input for add_step vertex, providing value 10 to port 'a'
'{"add_step": {"a": 10}}'

// Example Input for mul_step vertex, providing value 5 to port 'a'
'{"mul_step": {"a": 5}}'
```

**Execution Commands:**

```bash
# Execute the simple addition DAG (replace <add_dag_id> and provide input)
nexus dag execute --dag-id <add_dag_id> --input-json '{"add_step": {"a": 10}}' --inspect

# Execute the simple multiplication DAG (replace <mul_dag_id> and provide input)
nexus dag execute --dag-id <mul_dag_id> --input-json '{"mul_step": {"a": 5}}' --inspect

# Execute the combined DAG via the 'add_path' (replace <combined_dag_id>)
nexus dag execute --dag-id <combined_dag_id> --input-json '{"add_step": {"a": 10}}' --entry-group add_path --inspect

# Execute the combined DAG via the 'mul_path' (replace <combined_dag_id>)
nexus dag execute --dag-id <combined_dag_id> --input-json '{"mul_step": {"a": 5}}' --entry-group mul_path --inspect
```
*   The `--inspect` flag automatically calls the inspection command after execution submission.
*   For the combined DAG, notice how the `--input-json` still targets the specific entry vertex being activated (`add_step` or `mul_step`), even though they belong to the same DAG file. The `--entry-group` flag is essential for selecting the correct starting point in the combined DAG.

### 4. Inspecting Execution (Manual)

If you don't use `--inspect` or need to check later, use the execution ID and transaction digest (returned by the `execute` command):

```bash
# Inspect a specific execution (replace <exec_id> and <tx_digest>)
nexus dag inspect-execution --dag-execution-id <dag_exec_id> --execution-digest <tx_digest>
```

This provides details on the execution status and results.

## Rationale and Summary

These examples demonstrate:
*   **Linear Flow:** A simple sequence where the output of one tool becomes the input for the next.
*   **Tool Usage:** Integrating standard Nexus tools (`math.i64.add`, `math.i64.mul`, `llm.openai.chat-completion`).
*   **Entry Points:** Defining how external data enters the DAG (`entry_vertices`).
*   **Default Values:** Configuring tools with static data (constants, API keys, prompts).
*   **Placeholders:** Highlighting the need to replace placeholder values like API keys before execution.

These basic DAGs serve as foundational examples for building more complex agent workflows within the Nexus framework. They follow the structure and rules defined in the `dag-construction.md` document. Remember to replace the placeholder API key before attempting to execute these DAGs using the Nexus CLI (`nexus dag execute ...`). 