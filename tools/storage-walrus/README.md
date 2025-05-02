# `xyz.taluslabs.storage.walrus.upload-json@1`

Standard Nexus Tool that uploads a JSON file to Walrus and returns the blob ID.

## Input

**`json`: [`String`]**

The JSON data to upload.

_opt_ **`publisher_url`: [`Option<String>`]** _default_: [`None`]

The walrus publisher URL.

_opt_ **`aggregator_url`: [`Option<String>`]** _default_: [`None`]

The URL of the Walrus aggregator to upload the JSON to.

_opt_ **`epochs`: [`u64`]** _default_: [`1`]

Number of epochs to store the data.

_opt_ **`send_to_address`: [`Option<String>`]** _default_: [`None`]

Optional address to which the created Blob object should be sent.

## Output Variants & Ports

**`newly_created`**

A new blob was created and uploaded successfully.

- **`newly_created.blob_id`: [`String`]** - The unique identifier for the uploaded blob
- **`newly_created.end_epoch`: [`u64`]** - The epoch at which the blob will expire
- **`newly_created.sui_object_id`: [`String`]** - Sui object ID of the newly created blob

**`already_certified`**

The blob was already certified in the blockchain.

- **`already_certified.blob_id`: [`String`]** - The unique identifier for the blob
- **`already_certified.end_epoch`: [`u64`]** - The epoch at which the blob will expire
- **`already_certified.tx_digest`: [`String`]** - Transaction digest of the certified blob

**`err`**

The blob upload failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`UploadErrorKind`]** - Type of error that occurred
  - Possible kinds:
    - `network` - Error during HTTP requests or network connectivity issues
    - `validation` - Invalid JSON input or data validation failures
- **`err.status_code`: [`Option<u16>`]** - HTTP status code if available (for network errors)

---

# `xyz.taluslabs.storage.walrus.read-json@1`

Standard Nexus Tool that reads a JSON file from Walrus and returns the JSON data.

## Input

**`blob_id`: [`String`]**

The blob ID of the JSON file to read.

_opt_ **`aggregator_url`: [`Option<String>`]** _default_: [`None`]

The URL of the Walrus aggregator to read the JSON from.

## Output Variants & Ports

**`ok`**

The JSON data was read successfully.

- **`ok.json`: [`Value`]** - The JSON data as a structured value
- **`ok.text`: [`String`]** - The JSON data as a raw string

**`err`**

The JSON read operation failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`ReadErrorKind`]** - Type of error that occurred
  - Possible kinds:
    - `network` - Error during HTTP requests or network connectivity issues
    - `validation` - Invalid JSON data format or parsing failures
- **`err.status_code`: [`Option<u16>`]** - HTTP status code if available (for network errors)

---

=======
