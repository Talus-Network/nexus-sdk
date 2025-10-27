//! # `xyz.taluslabs.prompt.template.new@1`
//!
//! Tool that renders prompt templates using minijinja templating engine.

use {
    minijinja::Environment,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    std::collections::HashMap,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The template string to render
    pub template: String,
    /// Template arguments - can be either a HashMap<String, String> or a single String.
    /// Can be used together with name/value parameters.
    #[serde(default, deserialize_with = "deserialize_args_option")]
    pub args: Option<Args>,
    /// Optional single value to substitute. Must be used with 'name' parameter.
    /// Can be combined with 'args' parameter.
    pub value: Option<String>,
    /// Optional name for the single variable. Must be used with 'value' parameter.
    /// Can be combined with 'args' parameter.
    pub name: Option<String>,
}

/// Enum to represent either a HashMap or a String for template arguments
#[derive(Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub(crate) enum Args {
    Map(HashMap<String, String>),
    String(String),
}

fn deserialize_args_option<'de, D>(deserializer: D) -> Result<Option<Args>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(Value::Object(map)) => {
            let mut args_map = HashMap::new();
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    args_map.insert(k, s.to_string());
                } else {
                    args_map.insert(k, v.to_string());
                }
            }
            Ok(Some(Args::Map(args_map)))
        }
        Some(Value::String(s)) => Ok(Some(Args::String(s))),
        _ => Err(serde::de::Error::custom(
            "args must be either an object or a string",
        )),
    }
}

/// Output model for the prompt template tool
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { result: String },
    Err { message: String },
}

pub(crate) struct PromptTemplate;

impl NexusTool for PromptTemplate {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.prompt.template.new@1")
    }

    fn path() -> &'static str {
        "/new"
    }

    fn description() -> &'static str {
        "Tool that news prompt templates using minijinja templating engine with flexible input options."
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let mut env = Environment::new();

        // Check if args is a String type
        let args_is_string = matches!(input.args, Some(Args::String(_)));

        // Validate: at least one of args or (name/value) must be provided
        if input.args.is_none() && input.name.is_none() && input.value.is_none() {
            return Output::Err {
                message: "Either 'args' or 'name'/'value' parameters must be provided".to_string(),
            };
        }

        // Validate: if args is an empty HashMap and no name/value provided, return error
        if let Some(Args::Map(ref args_map)) = input.args {
            if args_map.is_empty() && input.name.is_none() && input.value.is_none() {
                return Output::Err {
                    message: "args cannot be empty when name and value are not provided"
                        .to_string(),
                };
            }
        }

        // Validate: if name or value is provided (and args is NOT a String), both must be provided
        if !args_is_string {
            match (&input.name, &input.value) {
                (Some(_), None) | (None, Some(_)) => {
                    return Output::Err {
                        message: "name and value must both be provided or both be None".to_string(),
                    };
                }
                _ => {}
            }
        }

        let mut all_args = match input.args {
            Some(Args::Map(args_map)) => args_map,
            Some(Args::String(variable_name)) => match input.value {
                Some(ref value) => HashMap::from([(variable_name, value.clone())]),
                None => {
                    return Output::Err {
                        message: "When args is a String, 'value' must be provided".to_string(),
                    };
                }
            },
            None => HashMap::new(),
        };

        // If name and value are provided, add them to all_args
        if let (Some(name), Some(value)) = (input.name, input.value) {
            all_args.insert(name, value);
        }

        // Enable strict mode to catch undefined variables
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);

        env.add_template("tmpl", &input.template)
            .expect("Failed to add template");

        let tmpl = env.get_template("tmpl").expect("Failed to get template");

        match tmpl.render(all_args) {
            Ok(result) => Output::Ok { result },
            Err(e) => Output::Err {
                message: format!("Template rendering failed: {}", e),
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
            args: Some(Args::Map(HashMap::from([
                ("name".to_string(), "Alice".to_string()),
                ("city".to_string(), "Paris".to_string()),
            ]))),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello Alice from Paris!"),
            Output::Err { message } => panic!("Expected success, got error: {}", message),
        }
    }

    #[tokio::test]
    async fn test_template_with_args_string_and_value() {
        let tool = PromptTemplate::new().await;

        let input = Input {
            template: "Hello {{user}}!".to_string(),
            args: Some(Args::String("user".to_string())),
            value: Some("Bob".to_string()),
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello Bob!"),
            Output::Err { message } => panic!("Expected success, got error: {}", message),
        }
    }

    #[tokio::test]
    async fn test_template_with_name_and_value_only() {
        let tool = PromptTemplate::new().await;

        let input = Input {
            template: "Hello {{custom_var}}!".to_string(),
            args: None, // args opsiyonel artık
            value: Some("World".to_string()),
            name: Some("custom_var".to_string()),
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello World!"),
            Output::Err { message } => panic!("Expected success, got error: {}", message),
        }
    }

    #[tokio::test]
    async fn test_template_no_parameters_fails() {
        let tool = PromptTemplate::new().await;

        // Test: No args, no name, no value should fail
        let input = Input {
            template: "Simple template without variables".to_string(),
            args: None,
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for template with no parameters"),
            Output::Err { message } => {
                assert!(
                    message.contains("Either 'args' or 'name'/'value' parameters must be provided")
                )
            }
        }
    }

    #[tokio::test]
    async fn test_template_invalid_args_combination() {
        let tool = PromptTemplate::new().await;

        // name olmadan value verilmiş - bu hata vermeli
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: None,
            value: Some("World".to_string()),
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for invalid args combination"),
            Output::Err { message } => {
                assert!(message.contains("name and value must both be provided"))
            }
        }
    }

    #[tokio::test]
    async fn test_template_rendering_error() {
        let tool = PromptTemplate::new().await;

        // Test: Template with undefined variable should fail during rendering
        let input = Input {
            template: "Hello {{undefined_var}}!".to_string(),
            args: Some(Args::Map(HashMap::from([(
                "other_var".to_string(),
                "value".to_string(),
            )]))), // Non-empty args but missing the required variable
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for undefined variable"),
            Output::Err { message } => assert!(message.contains("Template rendering failed")),
        }
    }

    #[tokio::test]
    async fn test_args_and_name_value_combined() {
        let tool = PromptTemplate::new().await;

        // Test: args Map + name/value should work together
        let input = Input {
            template: "Hello {{name}} from {{city}}!".to_string(),
            args: Some(Args::Map(HashMap::from([(
                "city".to_string(),
                "Paris".to_string(),
            )]))),
            value: Some("Alice".to_string()),
            name: Some("name".to_string()),
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello Alice from Paris!"),
            Output::Err { message } => panic!("Expected success, got error: {}", message),
        }
    }

    #[tokio::test]
    async fn test_args_string_and_value_combined() {
        let tool = PromptTemplate::new().await;

        // Test: args String + value should work together
        let input = Input {
            template: "Hello {{user}}!".to_string(),
            args: Some(Args::String("user".to_string())),
            value: Some("Bob".to_string()),
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { result } => assert_eq!(result, "Hello Bob!"),
            Output::Err { message } => panic!("Expected success, got error: {}", message),
        }
    }

    #[tokio::test]
    async fn test_value_without_name_fails() {
        let tool = PromptTemplate::new().await;

        // Test: value without name should fail
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: Some(Args::Map(HashMap::from([(
                "name".to_string(),
                "Alice".to_string(),
            )]))),
            value: Some("Bob".to_string()),
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for value without name"),
            Output::Err { message } => {
                assert!(message.contains("name and value must both be provided"))
            }
        }
    }

    #[tokio::test]
    async fn test_name_without_value_fails() {
        let tool = PromptTemplate::new().await;

        // Test: name without value should fail
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: None,
            value: None,
            name: Some("name".to_string()),
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for name without value"),
            Output::Err { message } => {
                assert!(message.contains("name and value must both be provided"))
            }
        }
    }

    #[tokio::test]
    async fn test_empty_args_without_name_value_fails() {
        let tool = PromptTemplate::new().await;

        // Test: empty args without name/value should fail
        let input = Input {
            template: "Hello {{name}}!".to_string(),
            args: Some(Args::Map(HashMap::new())),
            value: None,
            name: None,
        };

        let result = tool.invoke(input).await;
        match result {
            Output::Ok { .. } => panic!("Expected error for empty args without name/value"),
            Output::Err { message } => {
                assert!(
                    message.contains("args cannot be empty when name and value are not provided")
                )
            }
        }
    }
}
