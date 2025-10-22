const { describe, test, expect, beforeAll } = require("@jest/globals");

// Import WASM module - we'll use the Node.js build
let wasmModule;

beforeAll(() => {
  // Load the WASM module for Node.js using require
  wasmModule = require("../pkg-node/nexus_cli_wasm.js");
}, 1000); // 1 second timeout for WASM loading

describe("DAG Validation Tests", () => {
  test("should validate a simple valid DAG", () => {
    const validDag = {
      default_values: [
        {
          vertex: "add_input_and_default",
          input_port: "b",
          value: {
            storage: "inline",
            data: -3,
          },
        },
        {
          vertex: "mul_by_neg_3",
          input_port: "b",
          value: {
            storage: "inline",
            data: -3,
          },
        },
        {
          vertex: "mul_by_7",
          input_port: "b",
          value: {
            storage: "inline",
            data: 7,
          },
        },
        {
          vertex: "is_negative",
          input_port: "b",
          value: {
            storage: "inline",
            data: 0,
          },
        },
        {
          vertex: "add_1",
          input_port: "b",
          value: {
            storage: "inline",
            data: 1,
          },
        },
      ],
      vertices: [
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "xyz.taluslabs.math.i64.add@1",
          },
          name: "add_input_and_default",
          entry_ports: [
            {
              name: "a",
              encrypted: false,
            },
          ],
        },
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "xyz.taluslabs.math.i64.cmp@1",
          },
          name: "is_negative",
        },
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "xyz.taluslabs.math.i64.mul@1",
          },
          name: "mul_by_neg_3",
        },
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "xyz.taluslabs.math.i64.mul@1",
          },
          name: "mul_by_7",
        },
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "xyz.taluslabs.math.i64.add@1",
          },
          name: "add_1",
        },
      ],
      edges: [
        {
          from: {
            vertex: "add_input_and_default",
            output_variant: "ok",
            output_port: "result",
          },
          to: {
            vertex: "is_negative",
            input_port: "a",
          },
        },
        {
          from: {
            vertex: "is_negative",
            output_variant: "lt",
            output_port: "a",
          },
          to: {
            vertex: "mul_by_neg_3",
            input_port: "a",
          },
        },
        {
          from: {
            vertex: "is_negative",
            output_variant: "gt",
            output_port: "a",
          },
          to: {
            vertex: "mul_by_7",
            input_port: "a",
          },
        },
        {
          from: {
            vertex: "is_negative",
            output_variant: "eq",
            output_port: "a",
          },
          to: {
            vertex: "add_1",
            input_port: "a",
          },
        },
      ],
    };

    const result = wasmModule.validate_dag_from_json(JSON.stringify(validDag));

    expect(result.is_valid).toBe(true);
    expect(result.error_message).toBeUndefined();
  });

  test("should validate a simple invalid DAG", () => {
    const invalidDag = {
      vertices: [
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "com.test.test@1",
          },
          name: "a",
          entry_ports: [{ name: "input", encrypted: false }],
        },
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "com.test.test@1",
          },
          name: "b",
        },
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "com.test.test@1",
          },
          name: "c",
        },
        {
          kind: {
            variant: "off_chain",
            tool_fqn: "com.test.test@1",
          },
          name: "d",
        },
      ],
      edges: [
        {
          from: {
            vertex: "a",
            output_variant: "1",
            output_port: "1.0",
          },
          to: {
            vertex: "b",
            input_port: "1",
          },
        },
        {
          from: {
            vertex: "a",
            output_variant: "2",
            output_port: "2.0",
          },
          to: {
            vertex: "b",
            input_port: "1",
          },
        },
        {
          from: {
            vertex: "a",
            output_variant: "2",
            output_port: "2.1",
          },
          to: {
            vertex: "c",
            input_port: "1",
          },
        },
        {
          from: {
            vertex: "b",
            output_variant: "1",
            output_port: "1.0",
          },
          to: {
            vertex: "d",
            input_port: "1",
          },
        },
        {
          from: {
            vertex: "c",
            output_variant: "1",
            output_port: "1.0",
          },
          to: {
            vertex: "d",
            input_port: "1",
          },
        },
      ],
    };

    const result = wasmModule.validate_dag_from_json(
      JSON.stringify(invalidDag)
    );
    console.log(result.error_message);
    expect(result.is_valid).toBe(false);
    expect(result.error_message).toBe(
      "'Input port: d.1' has a race condition on it when invoking group '_default_group'"
    );
  });
});
