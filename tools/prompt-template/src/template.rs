//! # `xyz.taluslabs.prompt.template.new@1`
//!
//! Tool that renders prompt templates using minijinja templating engine.

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

/// Output model for the prompt template tool
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { result: String },
    Err { reason: String },
}

pub(crate) struct PromptTemplate;

impl NexusTool for PromptTemplate {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.prompt-template@1")
    }

    fn path() -> &'static str {
        "/prompt-template"
    }

    fn description() -> &'static str {
        "Tool that parses prompt templates using Jinja2 templating engine with flexible input options."
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

        let tmpl = env
            .get_template("tmpl")
            .expect("Template must exist because it was added.");

        match tmpl.render(all_args.clone()) {
            Ok(_rendered) => {
                let mut result = input.template.clone();
                // Manually replace Jinja2-style placeholders ({{variable}}) with their values.
                // This allows undefined variables to be preserved as placeholders for potential
                // chaining with other tools, rather than being rendered as empty strings.
                for (var, value) in &all_args {
                    let placeholder = format!("{{{{{}}}}}", var);
                    result = result.replace(&placeholder, value);
                }

                Output::Ok { result }
            }
            Err(e) => Output::Err {
                reason: format!("Template rendering failed: {}", e),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_template_with_args_map() {
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
        let tool = PromptTemplate::new().await;

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
}
