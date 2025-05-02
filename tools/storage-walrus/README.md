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

# `xyz.taluslabs.storage.walrus.download-file@1`

Standard Nexus Tool that downloads a file from Walrus and saves it to a local path.

## Input

**`blob_id`: [`String`]**

The unique identifier of the blob to download.

_opt_ **`output_path`: [`String`]** _default_: [`"$HOME/Downloads"`]

The local directory path where the downloaded file will be saved. The actual file will be saved as `downloaded_file.{extension}` in this directory. If a file with the same name already exists, it will automatically append a number in parentheses (e.g., `downloaded_file(1).{extension}`).

_opt_ **`file_extension`: [`FileExtension`]** _default_: [`"txt"`]

The file extension to use when saving the downloaded file. Supported extensions:

- `txt` - Text file
- `json` - JSON file
- `bin` - Binary file
- `png` - PNG image
- `jpg` - JPG image
- `jpeg` - JPEG image

_opt_ **`aggregator_url`: [`Option<String>`]** _default_: [`None`]

The walrus aggregator URL. If not provided, the default Walrus configuration will be used.

## Output Variants & Ports

**`ok`**

The file was downloaded successfully.

- **`ok.blob_id`: [`String`]** - The unique identifier of the downloaded blob
- **`ok.contents`: [`String`]** - A success message indicating where the file was saved

**`err`**

The file download failed.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong
- **`err.kind`: [`DownloadErrorKind`]** - Type of error that occurred
  - Possible kinds:
    - `network` - Error during HTTP requests or network connectivity issues
    - `validation` - Path validation errors (e.g., directory does not exist)
    - `file_system` - File system errors (e.g., directory exists but is not writable)
- **`err.status_code`: [`Option<u16>`]** - HTTP status code if available (for network errors)
