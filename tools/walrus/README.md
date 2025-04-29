# `xyz.taluslabs.walrus.file.download@1`

Standard Nexus Tool that downloads a file from Walrus and saves it to a local path.

## Input

**`blob_id`: [`String`]**

The unique identifier of the blob to download.

_opt_ **`output_path`: [`String`]** _default_: [`"."`]

The local path where the downloaded file will be saved.

_opt_ **`aggregator_url`: [`Option<String>`]** _default_: [`None`]

The walrus aggregator URL.

## Output Variants & Ports

**`ok`**

The file was downloaded successfully.

- **`ok.blob_id`: [`String`]** - The unique identifier of the downloaded blob
- **`ok.contents`: [`String`]** - A success message indicating where the file was saved

**`err`**

The file download failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
  - Possible reasons include:
    - Invalid output path (directory does not exist or is not writable)
    - Network connection errors
    - Server-side errors
    - Blob not found
    - Timeout errors
