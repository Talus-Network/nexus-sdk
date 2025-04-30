# `xyz.taluslabs.storage.walrus.upload-json`

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

**`ok`**

The JSON data was uploaded successfully.

- **`ok.blob_id`: [`String`]** - The unique identifier for the uploaded blob
- **`ok.end_epoch`: [`u64`]** - The epoch at which the blob will expire
- **`ok.newly_created`: [`Option<bool>`]** - Present and `true` if a new blob was created
- **`ok.already_certified`: [`Option<bool>`]** - Present and `true` if the blob was already certified
- **`ok.tx_digest`: [`Option<String>`]** - Transaction digest (only if `already_certified` is true)
- **`ok.sui_object_id`: [`Option<String>`]** - Sui object ID (only if `newly_created` is true)

**`err`**

The blob upload failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
  - Possible reasons include:
    - Invalid JSON input
    - Network connection errors
    - Server-side errors
    - Timeout errors

---
