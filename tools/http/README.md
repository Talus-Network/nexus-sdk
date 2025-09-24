# `xyz.taluslabs.http.request@1`

Standard Nexus Tool that makes HTTP requests with support for various authentication methods, body types, and response validation.

## Input

**`method`: [`HttpMethod`]**

The HTTP method to use for the request.

- **`GET`** - Retrieve data from the server
- **`POST`** - Send data to the server
- **`PUT`** - Update existing data on the server
- **`DELETE`** - Remove data from the server
- **`PATCH`** - Partially update data on the server
- **`HEAD`** - Retrieve only headers from the server
- **`OPTIONS`** - Get allowed methods for a resource

**`url`: [`UrlInput`]**

The URL for the request. Can be provided in two formats:

- **Full URL**: `"https://api.example.com/users"`
- **Split URL**: `{ "base_url": "https://api.example.com", "path": "/users" }`

_opt_ **`headers`: [`Option<HashMap<String, String>>`]** _default_: [`None`]

Custom HTTP headers to include in the request.

_opt_ **`query`: [`Option<HashMap<String, String>>`]** _default_: [`None`]

Query parameters to append to the URL.

_opt_ **`auth`: [`Option<AuthConfig>`]** _default_: [`None`]

Authentication configuration for the request.

### AuthConfig Options

- **`None`** - No authentication
- **`BearerToken { token }`** - Bearer token authentication
- **`ApiKeyHeader { key, header_name }`** - API key in header (default: "X-API-Key")
- **`ApiKeyQuery { key, param_name }`** - API key in query parameter (default: "api_key")
- **`BasicAuth { username, password }`** - Basic authentication

_opt_ **`body`: [`Option<RequestBody>`]** _default_: [`None`]

Request body configuration.

### RequestBody Options

- **`Json { data }`** - JSON request body
- **`Form { data }`** - URL-encoded form data
- **`Multipart { fields }`** - Multipart form data for text fields
- **`Raw { data, content_type }`** - Raw bytes (base64 encoded)

### MultipartField Structure

- **`name`: [`String`]** - Field name
- **`value`: [`String`]** - Field value (text only)
- **`content_type`: [`Option<String>`]** - Content type for the field

_opt_ **`expect_json`: [`Option<bool>`]** _default_: [`None`]

Whether to expect a JSON response. If true, attempts to parse the response as JSON.

_opt_ **`json_schema`: [`Option<HttpJsonSchema>`]** _default_: [`None`]

Optional JSON schema to validate the response against.

### HttpJsonSchema Structure

- **`name`: [`String`]** - The name of the schema
- **`schema`: [`schemars::Schema`]** - The JSON schema for validation
- **`description`: [`Option<String>`]** - Description of the expected format
- **`strict`: [`Option<bool>`]** - Whether to enable strict schema adherence

_opt_ **`timeout_ms`: [`Option<u64>`]** _default_: [`30000`]

Request timeout in milliseconds.

_opt_ **`retries`: [`Option<u32>`]** _default_: [`0`]

Number of retries on failure.

_opt_ **`follow_redirects`: [`Option<bool>`]** _default_: [`false`]

Whether to follow HTTP redirects. Defaults to false, following curl's philosophy of not following redirects unless explicitly requested.

_opt_ **`allow_empty_json`: [`Option<bool>`]** _default_: [`false`]

If true, tolerate empty body when JSON is expected (e.g., 204 No Content).

## Output Variants & Ports

**`ok`**

The HTTP request was successful.

```json
{
  "type": "ok",
  "status": 200,
  "headers": {...},
  "raw_base64": "...",
  "text": "...",
  "json": {...},
  "schema_validation": {...}
}
```

- **`type`: [`"ok"`]** - Response type identifier
- **`status`: [`u16`]** - HTTP status code
- **`headers`: [`HashMap<String, String>`]** - Response headers
- **`raw_base64`: [`String`]** - Raw response body (base64 encoded)
- **`text`: [`Option<String>`]** - Text representation (if UTF-8 decodable)
- **`json`: [`Option<Value>`]** - JSON data (if parseable)
- **`schema_validation`: [`Option<SchemaValidationDetails>`]** - Schema validation details (if validation was performed)

### SchemaValidationDetails Structure

- **`name`: [`String`]** - Name of the schema that was used
- **`description`: [`Option<String>`]** - Description of the schema
- **`strict`: [`Option<bool>`]** - Whether strict mode was enabled
- **`valid`: [`bool`]** - Validation result
- **`errors`: [`Vec<String>`]** - Validation errors (if any)

**`err`**

An error occurred during the request.

```json
{
  "type": "err",
  "reason": "HTTP error 404: Not Found",
  "kind": "err_http",
  "status_code": 404
}
```

- **`type`: [`"err"`]** - Response type identifier
- **`reason`: [`String`]** - Detailed error message
- **`kind`: [`HttpErrorKind`]** - Type of error
- **`status_code`: [`Option<u16>`]** - HTTP status code if available

### HttpErrorKind Values

- **`err_http`** - HTTP error response (4xx, 5xx)
- **`err_json_parse`** - JSON parsing error
- **`err_schema_validation`** - Schema validation error
- **`err_network`** - Network connectivity error
- **`err_timeout`** - Request timeout error
- **`err_input`** - Input validation error
- **`err_url_parse`** - URL parsing error
- **`err_base64_decode`** - Base64 decoding error
