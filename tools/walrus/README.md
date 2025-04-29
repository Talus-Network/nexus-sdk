# `xyz.taluslabs.walrus.file.download@1`

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
  - Possible reasons include:
    - Directory does not exist
    - Directory exists but is not writable
    - File already exists and is not writable
    - Network connection errors
    - Server-side errors (e.g., 500 Internal Server Error)
    - Blob not found (404 error)
    - Timeout errors
