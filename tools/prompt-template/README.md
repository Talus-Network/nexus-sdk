# `xyz.taluslabs.prompt-template@1`

Standard Nexus Tool that parses prompt templates using Jinja2 templating engine with flexible input options.

## Input

**`template`: [`String`]**

The template string to render. Supports template syntax with variable substitution using double curly braces (e.g., `{{variable_name}}`). The tool operates in strict mode, meaning all template variables must be defined.

_opt_ **`args`: [`Option<HashMap<String, String>>`]** _default_: [`None`]

Template arguments as a HashMap mapping variable names to their values. This parameter can be combined with `name`/`value` parameters to provide additional variables. At least one of `args` or the `name`/`value` pair must be provided.

_opt_ **`value`: [`Option<String>`]** _default_: [`None`]

Optional single value to substitute in the template. Must be used together with the `name` parameter. Can be combined with the `args` parameter to add an additional variable.

_opt_ **`name`: [`Option<String>`]** _default_: [`None`]

Optional name for the single variable. Must be used together with the `value` parameter. This allows you to specify an additional variable beyond those provided in `args`.

## Output Variants & Ports

**`ok`**

The template was rendered successfully.

- **`ok.result`: [`String`]** - The rendered template with all variables substituted

**`err`**

The template rendering failed due to an error.

- **`err.reason`: [`String`]** - A detailed error message describing what went wrong. This could be:
  - `"Either 'args' or 'name'/'value' parameters must be provided"` - No parameters were provided or args is empty
  - `"name and value must both be provided or both be None"` - Only one of name/value was provided
  - `"Template rendering failed: [error details]"` - Template syntax error or undefined variable (e.g., `"Template rendering failed: undefined value (in tmpl:1)"`)

---

## Examples

### Example 1: Using args HashMap

```json
{
  "template": "Hello {{name}} from {{city}}!",
  "args": {
    "name": "Alice",
    "city": "Paris"
  }
}
```

**Output:**

```json
{
  "type": "ok",
  "result": "Hello Alice from Paris!"
}
```

### Example 2: Using name and value

```json
{
  "template": "Welcome, {{username}}!",
  "name": "username",
  "value": "Bob"
}
```

**Output:**

```json
{
  "type": "ok",
  "result": "Welcome, Bob!"
}
```

### Example 3: Combining args and name/value

```json
{
  "template": "{{greeting}} {{name}} from {{city}}!",
  "args": {
    "greeting": "Hello",
    "city": "Tokyo"
  },
  "name": "name",
  "value": "Dave"
}
```

**Output:**

```json
{
  "type": "ok",
  "result": "Hello Dave from Tokyo!"
}
```

### Example 4: Using empty args with name/value

```json
{
  "template": "{{message}}",
  "args": {},
  "name": "message",
  "value": "Test message"
}
```

**Output:**

```json
{
  "type": "ok",
  "result": "Test message"
}
```

### Example 5: Error - No parameters provided

```json
{
  "template": "Hello {{name}}!"
}
```

**Output:**

```json
{
  "type": "err",
  "reason": "Either 'args' or 'name'/'value' parameters must be provided"
}
```

### Example 6: Error - Only name provided

```json
{
  "template": "Hello {{name}}!",
  "name": "name"
}
```

**Output:**

```json
{
  "type": "err",
  "reason": "name and value must both be provided or both be None"
}
```

### Example 7: Error - Undefined variable

```json
{
  "template": "Hello {{undefined_var}}!",
  "args": {
    "other_var": "value"
  }
}
```

**Output:**

```json
{
  "type": "err",
  "reason": "Template rendering failed: undefined value (in tmpl:1)"
}
```
