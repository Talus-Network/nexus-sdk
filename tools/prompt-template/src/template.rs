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
    /// Template arguments - can be either a HashMap<String, String> or a single String
    #[serde(deserialize_with = "deserialize_args")]
    pub args: Args,
    /// Optional single value to substitute (if this is Some, args must be a String)
    pub value: Option<String>,
    /// Optional name for the single variable (if this is Some, value must also be Some)
    pub name: Option<String>,
}

/// Enum to represent either a HashMap or a String for template arguments
#[derive(Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub(crate) enum Args {
    Map(HashMap<String, String>),
    String(String),
}

fn deserialize_args<'de, D>(deserializer: D) -> Result<Args, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Object(map) => {
            let mut args_map = HashMap::new();
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    args_map.insert(k, s.to_string());
                } else {
                    args_map.insert(k, v.to_string());
                }
            }
            Ok(Args::Map(args_map))
        }
        Value::String(s) => Ok(Args::String(s)),
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

        let all_args = match input.args {
            Args::Map(mut args_map) => match (input.name, input.value) {
                (Some(name), Some(value)) => {
                    args_map.insert(name, value);
                    args_map
                }
                (None, None) => args_map,
                _ => {
                    return Output::Err {
                        message: "name and value must both be provided or both be None".to_string(),
                    };
                }
            },
            Args::String(variable_name) => match (input.name, input.value) {
                (Some(name), Some(value)) => HashMap::from([(name, value)]),
                (None, Some(value)) => HashMap::from([(variable_name, value)]),
                _ => {
                    return Output::Err {
                        message: "When args is a String, 'value' must be provided".to_string(),
                    };
                }
            },
        };

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
