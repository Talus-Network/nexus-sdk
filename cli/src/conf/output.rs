use crate::prelude::*;

const REDACTED_SECRET: &str = "[REDACTED]";

/// Safe presentation view of [`CliConf`].
#[derive(Serialize)]
pub(super) struct ConfOutput<'a> {
    sui: SuiConfOutput<'a>,
    nexus: &'a Option<NexusObjects>,
    tools: &'a HashMap<ToolFqn, ToolOwnerCaps>,
    agents: &'a HashMap<String, sui::types::Address>,
    secrets: &'a SecretsConf,
    data_storage: &'a DataStorageConf,
}

#[derive(Serialize)]
struct SuiConfOutput<'a> {
    pk: Option<&'static str>,
    rpc_url: &'a Option<reqwest::Url>,
}

impl<'a> From<&'a CliConf> for ConfOutput<'a> {
    fn from(conf: &'a CliConf) -> Self {
        Self {
            sui: SuiConfOutput {
                pk: conf.sui.pk.as_ref().map(|_| REDACTED_SECRET),
                rpc_url: &conf.sui.rpc_url,
            },
            nexus: &conf.nexus,
            tools: &conf.tools,
            agents: &conf.agents,
            secrets: &conf.secrets,
            data_storage: &conf.data_storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::cli_conf::{CliConf, SuiConf},
        nexus_sdk::types::SecretValue,
    };

    #[test]
    fn conf_output_redacts_the_private_key_in_every_format() {
        let conf = CliConf {
            sui: SuiConf {
                pk: Some(SecretValue::from("actual private key")),
                rpc_url: Some(reqwest::Url::parse("https://rpc.example.com").unwrap()),
            },
            ..CliConf::default()
        };
        let output = ConfOutput::from(&conf);

        let json = serde_json::to_string_pretty(&output).expect("output should serialize as JSON");
        let toml = toml::to_string_pretty(&output).expect("output should serialize as TOML");

        for rendered in [&json, &toml] {
            assert!(!rendered.contains("actual private key"));
            assert!(rendered.contains("[REDACTED]"));
            assert!(rendered.contains("https://rpc.example.com"));
        }
    }
}
