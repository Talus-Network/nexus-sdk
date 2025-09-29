# Onchain Tool Development Guide

This comprehensive guide walks you through developing and deploying onchain tools for the Nexus framework. Onchain tools are smart contracts (modules) that execute on Sui and integrate seamlessly with Nexus workflows.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Project Setup](#project-setup)
3. [Step-by-Step Development](#step-by-step-development)
4. [Testing](#testing)
5. [Deployment and Tool Registration](#deployment-and-tool-registration)
6. [Integration with Workflows](#integration-with-workflows)

## Prerequisites

Before starting, ensure you have:

### Required Knowledge

- Familiarity with Sui Move language
- Basic understanding of Nexus workflows and DAGs

### Setup Guide Completion

- Follow the [setup guide](setup.md) to make sure you've got the [Nexus CLI](../cli.md) and [Sui CLI](https://docs.sui.io/guides/developer/getting-started/sui-install) installed.

## Project Setup

todo: add local setup guide

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

**Note**: Update the paths and addresses to match your local setup and target network.

## Step-by-Step Development

### Step 1: Create the Basic Module Structure

Create `sources/my_tool.move`:

```move
module my_onchain_tool::my_tool;

use nexus_primitives::proof_of_uid::ProofOfUID;
use nexus_workflow::tool_output::{Self, ToolOutput};
use sui::bag::{Self, Bag};
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
    Success {
        result: AsciiString,
        // Add custom fields here as needed
    },
    Error {
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
    ctx: &mut TxContext,
): ToolOutput {
    // Get the witness for stamping.
    let witness = state.witness();

    // REQUIRED: Stamp the worksheet to prove execution
    worksheet.stamp_with_data(&witness.id, b"my_tool_executed");

    // Implement your tool logic here
    if (input_value == 0) {
        // Return error variant
        tool_output::error(b"Input value cannot be zero")
    } else if (input_value > 1000) {
        // Return custom variant
        tool_output::variant(b"custom_result")
            .with_field(b"data", b"large_value_processed")
            .with_field(b"timestamp", sui::clock::timestamp_ms(ctx).to_string().into_bytes())
    } else {
        // Return success variant
        tool_output::success()
            .with_field(b"result", input_value.to_string().into_bytes())
    }
}
```

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
# Build first to check for errors
sui move build

# Publish to testnet (or your target network)
sui client publish --gas-budget 100000000

# Save the package ID from the output
export PACKAGE_ID="0x..."
```

### Step 2: Get Required Information

After publishing, you'll need:

1. **Package Address**: From publish output
2. **Module Name**: Your module name (e.g., "my_tool")
3. **Witness ID**: Object ID of your witness object

You can find the witness ID in the explorer by looking up the Witness object in the dynamic field ID that is given
to you in the publish output.

### Step 3: Register with Nexus

Use the Nexus CLI to register your tool with automatic schema generation:

```bash
nexus tool register-onchain \
  --package-address $PACKAGE_ID \
  --module-name my_tool \
  --tool-fqn "mydomain.my_tool@1" \
  --description "My custom onchain tool that processes values" \
  --witness-id "0x..."
```

The CLI will:

1. **Analyze your `execute` function** to generate input schema
2. **Generate output schema** from your `Output` enum
3. **Prompt for parameter descriptions** (interactive customization)
4. **Register the tool** in the Nexus tool registry

### Step 4: Verify Registration

```bash
# List all registered tools
nexus tool list
```

## Integration with Workflows

### Using in DAG Definitions

Once registered, your onchain tool can be used in Nexus workflows the same way offchain tools are used.

### Useful Sources

todo: link docs here
- **Nexus Documentation**: Check the main [tool documentation]()
- **Examples**: Study the [onchain tool examples]()

Remember that onchain tools are powerful building blocks in the Nexus ecosystem. Well-designed tools can be composed with others to create sophisticated autonomous agents and workflows.
