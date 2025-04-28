# `xyz.taluslabs.walrus.file.upload@1`

Standard Nexus Tool that uploads a file to Walrus and returns the StorageInfo.

## Input

**`file_path`: [`String`]**

The path to the file to upload.

_opt_ **`publisher_url`: [`Option<String>`]** _default_: [`None`]

The walrus publisher URL.

_opt_ **`epochs`: [`u64`]** _default_: [`1`]

Number of epochs to store the file.

_opt_ **`send_to`: [`Option<String>`]** _default_: [`None`]

Optional address to which the created Blob object should be sent.

## Output Variants & Ports

**`ok`**

The file was uploaded successfully.

- **`ok.blob_id`: [`String`]** - The unique identifier for the uploaded blob
- **`ok.end_epoch`: [`u64`]** - The epoch at which the blob will expire
- **`ok.newly_created`: [`Option<bool>`]** - Present and `true` if a new blob was created
- **`ok.already_certified`: [`Option<bool>`]** - Present and `true` if the blob was already certified
- **`ok.tx_digest`: [`Option<String>`]** - Transaction digest (only if `already_certified` is true)
- **`ok.sui_object_id`: [`Option<String>`]** - Sui object ID (only if `newly_created` is true)

**`err`**

The file upload failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
  - Possible reasons include:
    - Invalid file data
    - Network connection errors
    - Server-side errors
    - Timeout errors
