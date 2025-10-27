# `xyz.taluslabs.prompt.template.new@1`

Standard Nexus Tool that renders prompt templates using minijinja templating engine with flexible input options.

## Input

**`template`: [`String`]**

The template string to render. Supports template syntax with variable substitution using double curly braces (e.g., `{{variable_name}}`). The tool operates in strict mode, meaning all template variables must be defined.

_opt_ **`args`: [`Option<Args>`]** _default_: [`None`]

Template arguments - can be either a HashMap<String, String> or a single String.

- When provided as an object (HashMap): Maps variable names to their values
- When provided as a String: Specifies the variable name to use with the `value` parameter

This parameter can be combined with `name`/`value` parameters to provide additional variables. At least one of `args` or the `name`/`value` pair must be provided. If `args` is an empty HashMap, then `name` and `value` must also be provided.

_opt_ **`value`: [`Option<String>`]** _default_: [`None`]

Optional single value to substitute in the template. Must be used together with the `name` parameter (except when `args` is a String, in which case `value` can be provided alone and the `args` string will be used as the variable name).

_opt_ **`name`: [`Option<String>`]** _default_: [`None`]

Optional name for the single variable. Must be used together with the `value` parameter. This allows you to specify an additional variable beyond those provided in `args`.

## Output Variants & Ports

**`ok`**

The template was rendered successfully.

- **`ok.result`: [`String`]** - The rendered template with all variables substituted

**`err`**

The template rendering failed due to an error.

- **`err.message`: [`String`]** - A detailed error message describing what went wrong. This could be:
  - `"Either 'args' or 'name'/'value' parameters must be provided"` - No parameters were provided
  - `"args cannot be empty when name and value are not provided"` - Empty args object with no name/value
  - `"name and value must both be provided or both be None"` - Only one of name/value was provided
  - `"When args is a String, 'value' must be provided"` - args is a String but value is missing
  - `"Template rendering failed: [error details]"` - Template syntax error or undefined variable (e.g., `"Template rendering failed: undefined value (in tmpl:1)"`)

---

## Examples

### Example 1: Using args as a HashMap

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

### Example 3: Using args as a String with value

```json
{
  "template": "Hello {{user}}!",
  "args": "user",
  "value": "Charlie"
}
```

**Output:**

```json
{
  "type": "ok",
  "result": "Hello Charlie!"
}
```

### Example 4: Combining args and name/value

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
  "message": "Either 'args' or 'name'/'value' parameters must be provided"
}
```

### Example 6: Error - Undefined variable

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
  "message": "Template rendering failed: undefined value (in tmpl:1)"
}
```
