# Nexus CLI WASM

WebAssembly bindings for Nexus CLI functionality, enabling DAG validation and other operations in the browser and web environments.

## Features

- **DAG Validation**: Validate Nexus DAG structures from JSON in the browser
- **DAG Execute**: Build DAG execution transactions (CLI-compatible)
- **Walrus Storage**: Upload data to Walrus for remote port storage (CLI `--remote` parity)
- **Zero Dependencies**: All functionality bundled in WASM for web usage
- **Multiple Targets**: Supports web, Node.js, and bundler environments

## Building

### Prerequisites

Install `wasm-pack`:

```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### Build Commands

```bash
# Build for web (default)
npm run build

# Build for Node.js
npm run build:node

# Build for bundlers (webpack, etc.)
npm run build:bundler

# Build all targets
npm run build:all
```

## Usage

### Web

```javascript
import init, { validate_dag_from_json } from "./pkg/nexus_cli_wasm.js";

async function validateDag() {
  await init();

  const dagJson = `{
        "vertices": [...],
        "edges": [...],
        ...
    }`;

  const result = validate_dag_from_json(dagJson);

  if (result.is_valid) {
    console.log("DAG is valid!");
  } else {
    console.error("DAG validation failed:", result.error_message);
  }
}
```

### Node.js

```javascript
const { validate_dag_from_json } = require("./pkg-node/nexus_cli_wasm.js");

const dagJson = `{...}`;
const result = validate_dag_from_json(dagJson);

console.log(result.is_valid ? "Valid!" : result.error_message);
```

## API

### `validate_dag_from_json(dag_json: string): ValidationResult`

Validates a DAG from a JSON string.

**Parameters:**

- `dag_json`: JSON string representation of the DAG

**Returns:**

- `ValidationResult` object with:
  - `is_valid`: boolean indicating if the DAG is valid
  - `error_message`: optional string with error details if validation fails

### Walrus (Remote Storage)

To store port data in Walrus instead of inline (CLI `--remote` parity):

1. **Upload data** for each remote port:

```javascript
const blobId = await upload_json_to_walrus(
  "https://publisher.walrus-testnet.walrus.space",
  JSON.stringify(portValue),
  2  // save_for_epochs (1-53)
);
```

2. **Build the transaction** with remote ports:

```javascript
const result = build_dag_execution_transaction(
  dagId,
  "default",
  inputJson,
  "{}",
  "10000000",
  "0",
  '["vertex.port"]',           // remote_ports_json
  `{"vertex.port": "${blobId}"}`,  // remote_ports_blob_ids_json
  "2"                         // walrus_save_for_epochs
);
```

**Parameters** (Walrus, optional):

- `remote_ports_json`: JSON array of `"vertex.port"` strings
- `remote_ports_blob_ids_json`: JSON object mapping `"vertex.port"` → blob ID
- `walrus_save_for_epochs`: Number of epochs (1-53). Required when using remote ports.

## Development

The WASM bindings are built from Rust source code using `wasm-bindgen`. The main validator logic is adapted from the Nexus CLI to work in WASM environments.

## License

Apache-2.0
