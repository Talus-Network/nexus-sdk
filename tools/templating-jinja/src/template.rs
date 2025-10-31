//! # `xyz.taluslabs.templating.jinja@1`
//!
//! Tool that renders templates using minijinja templating engine.

use {
    minijinja::Environment,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The template string to render
    template: String,
    /// Template arguments as a HashMap<String, String>.
    /// Can be used together with name/value parameters.
    #[serde(default)]
    args: HashMap<String, String>,
    /// Optional single value to substitute. Must be used with 'name' parameter.
    /// Can be combined with 'args' parameter.
    value: Option<String>,
    /// Optional name for the single variable. Must be used with 'value' parameter.
    /// Can be combined with 'args' parameter.
    name: Option<String>,
}

/// Output model for the templating tool
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { result: String },
    Err { reason: String },
}

/// Helper function to render a variable expression using MiniJinja
fn render_variable_expression(expr: &str, var_name: &str, value: &str) -> String {
    let mut temp_env = Environment::new();

    // Try to render with MiniJinja
    if temp_env.add_template("temp", expr).is_ok() {
        if let Ok(tmpl) = temp_env.get_template("temp") {
            let mut ctx = HashMap::new();
            ctx.insert(var_name.to_string(), value.to_string());

            if let Ok(rendered) = tmpl.render(ctx) {
                return rendered;
            }
        }
    }

    // If rendering fails, return original expression
    expr.to_string()
}

pub(crate) struct TemplatingJinja;

impl NexusTool for TemplatingJinja {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.templating.jinja@1")
    }

    fn path() -> &'static str {
        "/templating-jinja"
    }

    fn description() -> &'static str {
        "Tool that parses templates using Jinja2 templating engine with flexible input options."
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let mut env = Environment::new();

        let mut all_args = input.args;

        // Validate: if name or value is provided, both must be provided
        match (&input.name, &input.value) {
            (None, None) => (),
            (Some(name), Some(value)) => {
                all_args.insert(name.clone(), value.clone());
            }
            _ => {
                return Output::Err {
                    reason: "name and value must both be provided or both be None".to_string(),
                };
            }
        }

        // Validate: at least one parameter must be provided
        if all_args.is_empty() {
            return Output::Err {
                reason: "Either 'args' or 'name'/'value' parameters must be provided".to_string(),
            };
        }

        env.set_undefined_behavior(minijinja::UndefinedBehavior::Chainable);

        // First, validate template syntax by attempting to add it
        match env.add_template("tmpl", &input.template) {
            Ok(_) => {}
            Err(e) => {
                return Output::Err {
                    reason: format!("Template syntax error: {}", e),
                };
            }
        }

        // Parse template and handle variables with optional whitespace
        let mut result = input.template.clone();

        for (var_name, value) in &all_args {
            let mut new_result = String::new();
            let mut remaining = result.as_str();

            while let Some(start_pos) = remaining.find("{{") {
                // Add everything before {{
                new_result.push_str(&remaining[..start_pos]);

                // Look for closing }}
                let after_open = &remaining[start_pos + 2..];
                if let Some(end_pos) = after_open.find("}}") {
                    let content = &after_open[..end_pos];
                    let trimmed = content.trim();

                    // Check if this variable matches (with optional filters/spaces)
                    let matches = trimmed == var_name
                        || trimmed.starts_with(&format!("{} ", var_name))
                        || trimmed.starts_with(&format!("{}|", var_name))
                        || trimmed.starts_with(&format!("{} |", var_name));

                    if matches {
                        // Extract filter part if any
                        let filter_part = if trimmed.len() > var_name.len() {
                            &trimmed[var_name.len()..]
                        } else {
                            ""
                        };

                        let expr = format!("{{{{{}{}}}}}", var_name, filter_part);
                        let rendered = render_variable_expression(&expr, var_name, value);
                        new_result.push_str(&rendered);
                    } else {
                        // Not our variable, keep original
                        new_result.push_str("{{");
                        new_result.push_str(content);
                        new_result.push_str("}}");
                    }

                    remaining = &after_open[end_pos + 2..];
                } else {
                    // No closing }}, keep original
                    new_result.push_str("{{");
                    remaining = after_open;
                }
            }

            // Add remaining text
            new_result.push_str(remaining);
            result = new_result;
        }

        Output::Ok { result }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_template_with_args_map() {
        let tool = TemplatingJinja::new().await;

        let input = Input {
            template: "Hello {{name}} from {{city}}!".to_string(),
            args: HashMap::from([
                ("name".to_string(), "Alice".to_string()),
                ("city".to_string(), "Paris".to_string()),
            ]),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello Alice from Paris!"),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }

    #[tokio::test]
    async fn test_template_with_name_and_value() {
        let tool = TemplatingJinja::new().await;

        let input = Input {
            template: "Hello {{user}}!".to_string(),
            args: HashMap::new(),
            value: Some("Bob".to_string()),
            name: Some("user".to_string()),
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello Bob!"),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }

    #[tokio::test]
    async fn test_template_with_name_and_value_only() {
        let tool = TemplatingJinja::new().await;

        let input = Input {
            template: "Hello {{custom_var}}!".to_string(),
            args: HashMap::new(),
            value: Some("World".to_string()),
            name: Some("custom_var".to_string()),
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello World!"),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }

    #[tokio::test]
    async fn test_template_no_parameters_fails() {
        let tool = TemplatingJinja::new().await;

        // Test: No args, no name, no value should fail
        let input = Input {
            template: "Simple template without variables".to_string(),
            args: HashMap::new(),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for template with no parameters"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Either 'args' or 'name'/'value' parameters must be provided")
                )
            }
        }
    }

    #[tokio::test]
    async fn test_template_invalid_args_combination() {
        let tool = TemplatingJinja::new().await;

        // Test: value without name should fail
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: HashMap::new(),
            value: Some("World".to_string()),
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for invalid args combination"),
            Output::Err { reason } => {
                assert!(reason.contains("name and value must both be provided"))
            }
        }
    }

    #[tokio::test]
    async fn test_template_with_undefined_variable_preserves_placeholder() {
        let tool = TemplatingJinja::new().await;

        // Test: Template with undefined variable should preserve placeholder for chaining
        let input = Input {
            template: "Hi, this is {{name}}, from {{city}}".to_string(),
            args: HashMap::from([("name".to_string(), "Pavel".to_string())]),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hi, this is Pavel, from {{city}}"),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }

    #[tokio::test]
    async fn test_args_and_name_value_combined() {
        let tool = TemplatingJinja::new().await;

        // Test: args Map + name/value should work together
        let input = Input {
            template: "Hello {{name}} from {{city}}!".to_string(),
            args: HashMap::from([("city".to_string(), "Paris".to_string())]),
            value: Some("Alice".to_string()),
            name: Some("name".to_string()),
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello Alice from Paris!"),
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }

    #[tokio::test]
    async fn test_value_without_name_fails() {
        let tool = TemplatingJinja::new().await;

        // Test: value without name should fail
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: HashMap::from([("name".to_string(), "Alice".to_string())]),
            value: Some("Bob".to_string()),
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for value without name"),
            Output::Err { reason } => {
                assert!(reason.contains("name and value must both be provided"))
            }
        }
    }

    #[tokio::test]
    async fn test_name_without_value_fails() {
        let tool = TemplatingJinja::new().await;

        // Test: name without value should fail
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: HashMap::new(),
            value: None,
            name: Some("name".to_string()),
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for name without value"),
            Output::Err { reason } => {
                assert!(reason.contains("name and value must both be provided"))
            }
        }
    }

    #[tokio::test]
    async fn test_empty_args_without_name_value_fails() {
        let tool = TemplatingJinja::new().await;

        // Test: empty args without name/value should fail
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: HashMap::new(),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for empty args without name/value"),
            Output::Err { reason } => {
                assert!(
                    reason.contains("Either 'args' or 'name'/'value' parameters must be provided")
                )
            }
        }
    }

    #[tokio::test]
    async fn test_partial_variables_preserved_for_chaining() {
        let tool = TemplatingJinja::new().await;

        // Test: Multiple undefined variables should all be preserved
        let input = Input {
            template: "Hello {{name}} from {{city}}! Your age is {{age}}.".to_string(),
            args: HashMap::from([("name".to_string(), "Alice".to_string())]),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => {
                assert_eq!(result, "Hello Alice from {{city}}! Your age is {{age}}.")
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }

    #[tokio::test]
    async fn test_template_with_filters_on_defined_and_undefined_variables() {
        let tool = TemplatingJinja::new().await;

        // Test: Template with filters on both defined and undefined variables
        let input = Input {
            template: "Hello {{name | upper}}, Im from {{ city | upper }}!".to_string(),
            args: HashMap::from([("name".to_string(), "Alice".to_string())]),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => {
                assert_eq!(result, "Hello ALICE, Im from {{ city | upper }}!")
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }

    #[tokio::test]
    async fn test_template_with_filter_and_simple_undefined_variable() {
        let tool = TemplatingJinja::new().await;

        // Test: Template with filter on defined variable and simple undefined variable
        let input = Input {
            template: "Hello {{name | upper}}, Im from {{city}}!".to_string(),
            args: HashMap::from([("name".to_string(), "Alice".to_string())]),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => {
                assert_eq!(result, "Hello ALICE, Im from {{city}}!")
            }
            Output::Err { reason } => panic!("Expected success, got error: {}", reason),
        }
    }
}
