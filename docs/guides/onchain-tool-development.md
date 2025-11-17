# Onchain Tool Development Guide

This comprehensive guide walks you through developing and deploying onchain tools for the Nexus framework. Onchain tools are smart contracts (modules) that execute on Sui and integrate seamlessly with Nexus workflows.

## Table of Contents

1. [Prerequisites](#prerequisites)
1. [Project Setup](#project-setup)
1. [Step-by-Step Development](#step-by-step-development)
1. [Testing](#testing)
1. [Deployment and Tool Registration](#deployment-and-tool-registration)
1. [Integration with Workflows](#integration-with-workflows)

## Prerequisites

Before starting, ensure you have:

### Required Knowledge

- Familiarity with Sui Move language
- Basic understanding of Nexus workflows and DAGs

### Setup Guide Completion

- Follow the [setup guide](setup.md) to make sure you've got the [Nexus CLI](../cli.md) and [Sui CLI](https://docs.sui.io/guides/developer/getting-started/sui-install) installed.

## Project Setup

{% hint style="info" %}
You can choose to skip the manual project setup completely by using
the following CLI command: `nexus tool new --name my_onchain_tool --template move`

This generates a ready-to-go onchain tool Move module for you to build from.
{% endhint %}

### 1. Create a New Move Package

```bash
# Initialize Move package
sui move new my_onchain_tool
```

### 2. Configure Dependencies

Edit your `Move.toml` file to include Nexus dependencies:

```toml
[package]
name = "my_onchain_tool"
edition = "2024.beta"

[dependencies]
nexus_primitives = { local = "path/to/nexus/primitives" }
nexus_workflow = { local = "path/to/nexus/workflow" }
nexus_interface = { local = "path/to/nexus/interface" }

[addresses]
my_onchain_tool = "0x0"
nexus_primitives = "0xf311c80e60f77ba6008237ed0cd619a05f25894cdac3e2318cf41c74e8e24cea"
nexus_workflow = "0x5addaf9046d4a16ec3cbbe4fa9f89b5da73e0304ed1fff26fb8301574692cf4b"
nexus_interface = "0xf66be6face6f35dea9d6aea2b6ce8930a2bf7f55ac9ec7c80ffcb8182cabf5ae"
```

**Note**: Update the paths and addresses to match your setup and target network.

## Step-by-Step Development

### Step 1: Create the Basic Module Structure

In `sources/my_onchain_tool.move`:

```move
module my_onchain_tool::my_onchain_tool;

use nexus_primitives::proof_of_uid::ProofOfUID;
use nexus_workflow::tool_output::{Self, ToolOutput};
use sui::bag::{Self, Bag};
use sui::clock::Clock;
use sui::transfer::share_object;
use std::ascii::String as AsciiString;

/// One-time witness for package initialization.
public struct MY_ONCHAIN_TOOL has drop {}

/// Witness object used to identify this tool.
public struct MyToolWitness has key, store {
    id: UID,
}

/// Your tool's state object (customize as needed).
public struct MyToolState has key {
    id: UID,
    /// Store the witness object that identifies this tool.
    witness: Bag,
    // Add your application-specific fields here
    // Example: value: u64,
    // ..
}
```

### Step 2: Define Output Variants

```move
/// Tool execution output variants.
/// This enum is used for automatic schema generation during registration.
/// It's not used during execution. Only the ToolOutput object is used.
public enum Output {
    Ok {
        result: u64,
        // Add custom fields here as needed
    },
    Err {
        reason: AsciiString,
    },
    // Add custom variants as needed
    CustomResult {
        data: vector<u8>,
        timestamp: u64,
    },
}
```

### Step 3: Implement Initialization

```move
/// Initialize your tool's state.
fun init(_otw: MY_ONCHAIN_TOOL, ctx: &mut TxContext) {
    let state = MyToolState {
        id: object::new(ctx),
        witness: {
            let mut bag = bag::new(ctx);
            bag.add(b"witness", MyToolWitness { id: object::new(ctx) });
            bag
        },
        // Initialize your fields
        // value: 0,
    };
    share_object(state);
}
```

### Step 4: Implement the Execute Function

This is the core of your tool, the function that performs the actual execution:

```move
/// Execute function with standardized Nexus signature.
///
/// CRITICAL REQUIREMENTS:
/// 1. First parameter: worksheet: &mut ProofOfUID
/// 2. Last parameter: ctx: &mut TxContext
/// 3. Return type: ToolOutput
/// 4. Must stamp worksheet with witness ID
public fun execute(
    worksheet: &mut ProofOfUID,
    state: &mut MyToolState,
    // Add your custom parameters here
    input_value: u64,
    clock: &Clock,
    ctx: &mut TxContext,
): ToolOutput {
    // Get the witness for stamping.
    let witness = state.witness();

    // REQUIRED: Stamp the worksheet to prove execution
    worksheet.stamp_with_data(&witness.id, b"my_tool_executed");

    // Implement your tool logic here
    if (input_value == 0) {
        // Return error variant
        tool_output::err(b"Input value cannot be zero")
    } else if (input_value > 1000) {
        // Return custom variant
        tool_output::variant(b"custom_result")
            .with_field(b"data", tool_output::string_value(b"large_value_processed"))
            .with_field(b"timestamp", tool_output::number_value(sui::clock::timestamp_ms(clock).to_string().into_bytes()))
    } else {
        // Return success variant
        let result = input_value * 2;
        tool_output::ok()
            .with_field(b"result", tool_output::number_value(result.to_string().into_bytes()))
    }
}
```

#### Understanding Field Value Types

When adding fields to `ToolOutput`, you must use typed constructor functions to ensure proper JSON formatting:

```move
// Numeric values (u8, u16, u32, u64, u128, u256)
.with_field(b"count", tool_output::number_value(value.to_string().into_bytes()))

// String values (will be wrapped in quotes in JSON)
.with_field(b"message", tool_output::string_value(b"Hello world"))

// Boolean values (true/false without quotes in JSON)
.with_field(b"success", tool_output::bool_value(b"true"))

// Address values (prefixed with "0x" and wrapped in quotes in JSON)
.with_field(b"sender", tool_output::address_value(address.to_string().into_bytes()))
```

This typing ensures that the Nexus framework correctly parses and processes your tool's outputs.

### Step 5: Add Helper Functions

```move
// === Getters and Helper Functions ===

/// Helper function to get the witness object.
fun witness(self: &MyToolState): &MyToolWitness {
    self.witness.borrow(b"witness")
}

/// Get the witness ID for external reference.
/// Useful for registering the tool onchain.
public fun witness_id(self: &MyToolState): ID {
    self.witness().id.to_inner()
}

// Add getters for your custom fields
// public fun value(self: &MyToolState): u64 {
//     self.value
// }

#[test_only]
public fun init_for_test(otw: MY_ONCHAIN_TOOL, ctx: &mut TxContext) {
    init(otw, ctx);
}
```

## Testing

Make sure to add tests in the `/tests` folder to test the correct functionality of your onchain tool module.

## Deployment and Tool Registration

### Step 1: Publish to Sui

```bash
# Publish to testnet (or your target network)
sui client publish

# Save the package ID from the output
export PACKAGE_ID="0x..."
# export <..> Other required IDs as described in step 2...
```

### Step 2: Get Required Information

After publishing, you'll need:

1. **Module Path**: Combination of package address and module name (e.g., "0xPACKAGE_ID::my_tool")
1. **Witness ID**: Object ID of your witness object
1. **\*ToolState**: The shared object necessary as argument for the execute function. _This ID is not required for tool registration._

You can find the witness ID in the explorer by looking up the Witness object in the dynamic field ID that is given
to you in the publish output. This object has type: `0x2::dynamic_field::Field<vector<u8>, PACKAGE_ID::my_onchain_tool::MyToolWitness>`

Alternatively, you can find the witness ID by using the CLI:

```bash
sui client object <DYNAMIC_FIELD_ID>
```

### Step 3: Register with Nexus

Use the Nexus CLI to register your tool with automatic schema generation:

```bash
nexus tool register onchain \
  --module-path "$PACKAGE_ID::my_onchain_tool" \
  --tool-fqn "xyz.mydomain.my_onchain_tool@1" \
  --description "My custom onchain tool that processes values" \
  --witness-id "0x..."
```

The CLI will:

1. **Analyze your `execute` function** to generate input schema
1. **Generate output schema** from your `Output` enum
1. **Prompt for parameter descriptions** (interactive customization)
1. **Register the tool** in the Nexus tool registry

### Step 4: Verify Registration

```bash
# List all registered tools
nexus tool list
```

## Integration with Workflows

### Using in DAG Definitions

Once registered, your onchain tool can be used in Nexus workflows the same way offchain tools are used.

An example JSON DAG using the onchain tool is as follows:

```json
{
  "default_values": [
    {
      "vertex": "just_execute_first",
      "input_port": "2",
      "value": {
        "storage": "inline",
        "data": "0x6"
      }
    },
    {
      "vertex": "just_execute_second",
      "input_port": "2",
      "value": {
        "storage": "inline",
        "data": "0x6"
      }
    }
  ],
  "vertices": [
    {
      "kind": {
        "variant": "on_chain",
        "tool_fqn": "xyz.mydomain.my_tool@1"
      },
      "name": "just_execute_first",
      "entry_ports": [
        {
          "name": "0",
          "encrypted": false
        },
        {
          "name": "1",
          "encrypted": false
        }
      ]
    },
    {
      "kind": {
        "variant": "on_chain",
        "tool_fqn": "xyz.mydomain.my_tool@1"
      },
      "name": "just_execute_second",
      "entry_ports": [
        {
          "name": "0",
          "encrypted": false
        }
      ]
    }
  ],
  "edges": [
    {
      "from": {
        "vertex": "just_execute_first",
        "output_variant": "ok",
        "output_port": "result"
      },
      "to": {
        "vertex": "just_execute_second",
        "input_port": "1"
      }
    }
  ]
}
```

This workflow only executes the onchain tool twice if the output variant is Success. Else it only executes it once.

### Useful Sources

- **Nexus Documentation**: Check the main [tool documentation](../../nexus-next/docs/tool.md)
- **Examples**: Study the [onchain tool example modules](../../nexus-next/sui/examples/) and [corresponding json dag workflows](../../sdk/src/dag/_dags/)

Remember that onchain tools are powerful building blocks in the Nexus ecosystem. Well-designed tools can be composed with others to create sophisticated autonomous agents and workflows.
